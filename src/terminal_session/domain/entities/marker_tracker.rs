use crate::history::domain::entities::history_entry::{HistoryEntry, HistoryEntryPrimitives};
use crate::shared::domain::value_objects::shell::Shell;
use crate::terminal_session::domain::value_objects::cell_tag::CellTag;
use crate::terminal_session::domain::value_objects::deferred_marker::DeferredMarker;
use crate::terminal_session::domain::value_objects::edit_buffer::EditBuffer;
use crate::terminal_session::domain::value_objects::edit_buffer_context::{
    EditBufferContext, EditBufferContextParts,
};
use crate::terminal_session::domain::value_objects::marker_effects::MarkerEffects;
use crate::terminal_session::domain::value_objects::marker_reaction::MarkerReaction;
use crate::terminal_session::domain::value_objects::pwsh_prediction_color::PwshPredictionColor;
use crate::terminal_session::domain::value_objects::screen_command::ScreenCommand;
use crate::terminal_session::domain::value_objects::session_id::SessionId;
use crate::terminal_session::domain::value_objects::shell_event::ShellEvent;
use crate::terminal_session::domain::value_objects::shell_marker::ShellMarker;
use crate::terminal_session::domain::value_objects::shell_suggestion_color::ShellSuggestionColor;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};

/// Shell-integration state of one wrapper session (SPEC §5.1, §5.1.2, §5.1.4): the prompt/command cycle the OSC 697
/// markers describe. It never touches the screen: it says which cells to tag (`ScreenCommand`) and what happened
/// (`ShellEvent`), and takes in what only the screen knows (the cursor column, the edit buffer).
pub(crate) struct MarkerTracker {
    session_id: SessionId,
    /// Windows ConPTY deferral of EndPrompt/NewCmd.
    defer_markers: bool,
    osc_lock: bool,
    preexec: bool,
    /// Current command cycle `n` (None before the first NewCmd).
    cycle: Option<u64>,
    /// The last NewCmd came at column 0 (a prompt ending in a newline): its line has no prompt cell.
    line_col0: bool,
    cwd: Option<PathBuf>,
    shell: Option<Shell>,
    shell_name: Option<String>,
    pid: Option<i32>,
    zsh_color: Option<ShellSuggestionColor>,
    fish_color: Option<ShellSuggestionColor>,
    /// Buffer the user submitted with Enter (set by the wrapper); preferred over a scrape that has shell output
    /// (`!!` echo, zsh correction prompt) after it. Cleared on NewCmd.
    submitted: Option<String>,
    /// The shell keeps the next command out of its history (`NoHistory` marker): tabsmith does too. Reset on NewCmd.
    no_history: bool,
    /// PSReadLine ListView predictions own the rows below the prompt (`PwshListView=1`/`0`, every pwsh prompt).
    /// Cleared when a command starts, not on NewCmd: the marker may precede NewCmd in the same prompt.
    pwsh_list_view: bool,
    /// PSReadLine `InlinePredictionColor`; None = PSReadLine's default dim/italic test.
    pwsh_color: Option<PwshPredictionColor>,
    /// PSReadLine ContinuationPrompt and PromptText(s): drawn after EndPrompt, so stripped from the buffer.
    pwsh_continuation: String,
    pwsh_prompt_text: Vec<String>,
    /// Running commands, innermost last: nested integrated shells share the session id (finished by PID).
    pending: Vec<HistoryEntry>,
    exit_code: Option<i32>,
    deferred: VecDeque<DeferredMarker>,
    batch_markers: usize,
}

impl MarkerTracker {
    pub(crate) fn new(session_id: SessionId, shell: Option<Shell>, defer_markers: bool) -> Self {
        MarkerTracker {
            session_id,
            defer_markers,
            osc_lock: false,
            preexec: false,
            cycle: None,
            line_col0: false,
            cwd: None,
            shell,
            shell_name: shell.map(|s| s.name().to_string()),
            pid: None,
            zsh_color: None,
            fish_color: None,
            submitted: None,
            no_history: false,
            pwsh_list_view: false,
            pwsh_color: None,
            pwsh_continuation: ">> ".into(), // PSReadLine's default, until the prompt reports it
            pwsh_prompt_text: Vec::new(),
            pending: Vec::new(),
            exit_code: None,
            deferred: VecDeque::new(),
            batch_markers: 0,
        }
    }

    /// One marker, at the byte offset it came in, with the cursor column the screen has there and the time now.
    pub(crate) fn on_marker(
        &mut self,
        marker: ShellMarker,
        cursor_column: usize,
        now_secs: u64,
    ) -> MarkerReaction {
        self.batch_markers += 1;
        let mut effects = MarkerEffects::default();
        if self.osc_lock && !marker.passes_integration_lock() {
            return MarkerReaction::Handled(effects);
        }
        match marker {
            ShellMarker::StartPrompt => {
                effects.push_screen_command(ScreenCommand::Tag(CellTag::Prompt))
            }
            ShellMarker::EndPrompt if self.defer_markers => {
                self.deferred.push_back(DeferredMarker::EndPrompt)
            }
            ShellMarker::EndPrompt => self.end_prompt(&mut effects),
            // A nested session's, or a bare one (any program output could forge it; our scripts always send the sid).
            ShellMarker::NewCmd(session_id)
                if session_id.as_deref() != Some(self.session_id.as_str()) => {}
            ShellMarker::NewCmd(_) if self.defer_markers => {
                self.deferred.push_back(DeferredMarker::NewCmd)
            }
            ShellMarker::NewCmd(_) => self.new_cmd(cursor_column, now_secs, &mut effects),
            ShellMarker::PreExec if !self.preexec => {
                self.flush_deferred(cursor_column, now_secs, &mut effects);
                return MarkerReaction::EditBufferNeeded(effects);
            }
            ShellMarker::NoHistory if !self.preexec => {
                self.flush_deferred(cursor_column, now_secs, &mut effects); // a queued NewCmd (which resets the flag) belongs before it
                self.no_history = true;
            }
            ShellMarker::OscLock(id) if id == self.session_id.as_str() => self.osc_lock = true,
            ShellMarker::OscUnlock(id) if id == self.session_id.as_str() => self.osc_lock = false,
            ShellMarker::Dir(path) => self.cwd = Some(path),
            ShellMarker::ExitCode(code) => self.exit_code = code,
            ShellMarker::Shell(name) => {
                self.shell = Shell::from_name(&name);
                self.shell_name = Some(name);
            }
            ShellMarker::Pid(pid) => self.pid = pid,
            ShellMarker::ZshAutosuggestionColor(color) => self.zsh_color = color,
            ShellMarker::FishSuggestionColor(color) => self.fish_color = color,
            ShellMarker::PwshPredictionColor(color) => self.pwsh_color = color,
            ShellMarker::PwshListView(on) => self.pwsh_list_view = on,
            ShellMarker::PwshPrompts {
                continuation,
                prompt_texts,
            } => {
                self.pwsh_continuation = continuation;
                self.pwsh_prompt_text = prompt_texts;
            }
            ShellMarker::PreExec
            | ShellMarker::NoHistory
            | ShellMarker::OscLock(_)
            | ShellMarker::OscUnlock(_)
            | ShellMarker::Ignored => {}
        }
        MarkerReaction::Handled(effects)
    }

    /// The second half of an accepted `PreExec`: `scraped` is the edit buffer read from the screen once the effects
    /// of `on_marker` were applied (None while no cycle is open).
    pub(crate) fn complete_pre_exec(
        &mut self,
        scraped: Option<&EditBuffer>,
        now_secs: u64,
    ) -> MarkerEffects {
        let scraped = scraped
            .map(|e| e.buffer().trim().to_string())
            .unwrap_or_default();
        // Lines the shell printed after Enter (`!!` echo, correction prompt) are not part of the command, nor is a
        // transient prompt redrawn without markers before it; a line the shell rewrote (abbreviation, PS2 loop) is
        // taken from the screen.
        let then_output = |s: &str| {
            scraped.match_indices(s).any(|(i, _)| {
                let rest = scraped[i + s.len()..].trim_start_matches(' ');
                (i == 0 || scraped[..i].ends_with(' '))
                    && (rest.is_empty() || rest.starts_with('\n'))
            })
        };
        let command = match self.submitted.take().map(|s| s.trim().to_string()) {
            Some(s) if !s.is_empty() && then_output(&s) => s,
            _ => scraped,
        };
        // Still pending (its prompt finishes it, SPEC §3.5), but blank: not recorded.
        let command = if std::mem::take(&mut self.no_history) {
            String::new()
        } else {
            command
        };
        self.pending.push(HistoryEntry::new(HistoryEntryPrimitives {
            command,
            shell: self.shell_name.clone().unwrap_or_default(),
            pid: self.pid,
            session_id: self.session_id.as_str().to_string(),
            cwd: self.cwd.as_ref().map(|p| p.display().to_string()),
            start_time: now_secs,
            ..Default::default()
        }));
        self.preexec = true;
        self.pwsh_list_view = false; // a nested shell does not inherit it; the next pwsh prompt re-reports it
        let mut effects = MarkerEffects::default();
        effects.push_screen_command(ScreenCommand::ClearTag);
        effects.push_event(ShellEvent::PreExec);
        effects
    }

    /// The end of a PTY batch (SPEC §5.1.4): the deferred markers are applied (FIFO) once a batch brings no new ones,
    /// at the cursor column the batch ends on.
    pub(crate) fn end_batch(&mut self, cursor_column: usize, now_secs: u64) -> MarkerEffects {
        let mut effects = MarkerEffects::default();
        if self.batch_markers == 0 {
            self.flush_deferred(cursor_column, now_secs, &mut effects);
        }
        self.batch_markers = 0;
        effects
    }

    /// At a prompt with a known command cycle (an edit buffer exists).
    pub(crate) fn at_prompt(&self) -> bool {
        self.cycle.is_some() && !self.preexec
    }

    /// What the screen needs to read the edit buffer; None while no cycle is open.
    pub(crate) fn edit_buffer_context(&self) -> Option<EditBufferContext<'_>> {
        let cycle = self.cycle.filter(|_| !self.preexec)?;
        Some(EditBufferContext::new(EditBufferContextParts {
            cycle,
            shell: self.shell,
            line_col0: self.line_col0,
            pwsh_list_view: self.pwsh_list_view,
            pwsh_continuation: &self.pwsh_continuation,
            pwsh_prompt_text: &self.pwsh_prompt_text,
            zsh_color: self.zsh_color.as_ref(),
            fish_color: self.fish_color.as_ref(),
            pwsh_color: self.pwsh_color.as_ref(),
        }))
    }

    pub(crate) fn preexec(&self) -> bool {
        self.preexec
    }

    pub(crate) fn cwd(&self) -> Option<&Path> {
        self.cwd.as_deref()
    }

    pub(crate) fn shell(&self) -> Option<Shell> {
        self.shell
    }

    pub(crate) fn pwsh_list_view(&self) -> bool {
        self.pwsh_list_view
    }

    pub(crate) fn submitted(&self) -> Option<&str> {
        self.submitted.as_deref()
    }

    pub(crate) fn set_submitted(&mut self, line: Option<String>) {
        self.submitted = line;
    }

    fn end_prompt(&mut self, effects: &mut MarkerEffects) {
        let open = self.cycle.filter(|_| !self.preexec);
        effects.push_screen_command(match open {
            Some(n) => ScreenCommand::Tag(CellTag::Cycle(n)),
            None => ScreenCommand::ClearTag,
        });
    }

    fn new_cmd(&mut self, cursor_column: usize, now_secs: u64, effects: &mut MarkerEffects) {
        let redraw = self.cycle.filter(|_| !self.preexec);
        let n = self.cycle.map_or(1, |n| n + 1);
        self.cycle = Some(n);
        self.preexec = false;
        self.no_history = false;
        effects.push_screen_command(ScreenCommand::Tag(CellTag::Cycle(n)));
        effects.push_screen_command(ScreenCommand::MarkLineStart {
            redrawn_cycle: redraw,
        });
        self.line_col0 = cursor_column == 0;
        // The prompt finishes this shell's command (and those of nested shells that exited under it); a nested
        // shell's first prompt leaves the outer command running.
        let exit = self.exit_code.take();
        let Some(k) = self.pending.iter().rposition(|p| p.pid() == self.pid) else {
            return effects.push_event(ShellEvent::NewCommand(None));
        };
        self.submitted = None; // not on a redraw: a transient prompt redraws between Enter and PreExec
        for mut h in self.pending.drain(k..).rev() {
            h.finish(exit, now_secs);
            // ponytail: an empty command (NoHistory, or nothing scraped) is the "do not record" mark.
            effects.push_event(if h.command().is_empty() {
                ShellEvent::CommandEnded
            } else {
                ShellEvent::NewCommand(Some(h))
            });
        }
    }

    fn flush_deferred(&mut self, cursor_column: usize, now_secs: u64, effects: &mut MarkerEffects) {
        while let Some(d) = self.deferred.pop_front() {
            match d {
                DeferredMarker::EndPrompt => self.end_prompt(effects),
                DeferredMarker::NewCmd => self.new_cmd(cursor_column, now_secs, effects),
            }
        }
    }
}

#[cfg(test)]
impl MarkerTracker {
    pub(crate) fn cycle(&self) -> Option<u64> {
        self.cycle
    }

    pub(crate) fn osc_lock(&self) -> bool {
        self.osc_lock
    }

    pub(crate) fn no_history(&self) -> bool {
        self.no_history
    }

    pub(crate) fn pending_is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    pub(crate) fn deferred(&self) -> &VecDeque<DeferredMarker> {
        &self.deferred
    }

    pub(crate) fn set_pwsh_list_view(&mut self, on: bool) {
        self.pwsh_list_view = on;
    }
}

#[cfg(test)]
mod tests;
