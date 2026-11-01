//! OSC 697 splitter, shell-integration marker tracker (cell tagging via injected OSC 8), edit-buffer and cursor
//! extraction from the model (SPEC §5.1, §5.6), and the filter for bytes sent to the real terminal (SPEC §7.5.4).
//! OWNER: wrapper agent.
use crate::shared::domain::value_objects::shell::Shell;
use crate::terminal_session::domain::entities::marker_splitter::MarkerSplitter;
use crate::terminal_session::domain::entities::marker_tracker::MarkerTracker;
use crate::terminal_session::domain::value_objects::cell_tag::CellTag;
use crate::terminal_session::domain::value_objects::edit_buffer::EditBuffer;
use crate::terminal_session::domain::value_objects::edit_buffer_context::EditBufferContext;
use crate::terminal_session::domain::value_objects::marker_effects::MarkerEffects;
use crate::terminal_session::domain::value_objects::marker_reaction::MarkerReaction;
use crate::terminal_session::domain::value_objects::output_piece::OutputPiece;
use crate::terminal_session::domain::value_objects::session_id::SessionId;
use crate::terminal_session::domain::value_objects::shell_event::ShellEvent;
use crate::terminal_session::domain::value_objects::shell_marker::ShellMarker;
use crate::terminal_session::domain::value_objects::shell_suggestion_color::{
    rgb_to_256, ShellSuggestionColor,
};
use crate::terminal_session::domain::value_objects::text_emphasis::TextEmphasis;
use crate::terminal_session::infrastructure::adapters::cell_tagger;
use crate::terminal_session::infrastructure::adapters::output_filter::OutputFilter;
use crate::types::{now_secs, Model};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::{Cell, Flags};
use alacritty_terminal::vte::ansi::{Color, Processor};

// ---------------------------------------------------------------------------------------------------------------
// Suggestion colours (SPEC §5.1.3)

fn cell_col(c: Color) -> Option<u8> {
    match c {
        Color::Indexed(i) => Some(i),
        Color::Spec(rgb) => Some(rgb_to_256(rgb.r, rgb.g, rgb.b)),
        Color::Named(n) => (n as usize).try_into().ok().filter(|i: &u8| *i < 16),
    }
}

fn matches_cell(z: &ShellSuggestionColor, cell: &Cell) -> bool {
    z.matches(cell_col(cell.fg), cell_col(cell.bg))
}

fn cell_emphasis(flags: Flags) -> TextEmphasis {
    TextEmphasis::new(flags.contains(Flags::DIM), flags.contains(Flags::ITALIC))
}

// ---------------------------------------------------------------------------------------------------------------
// Tracker (SPEC §5.1, §5.1.2, §5.1.4)

/// What `extract` collects while it scans the cells; the `EditBuffer` is built from it once, at the end.
#[derive(Default)]
struct Scraped {
    buffer: String,
    /// Byte offset into `buffer`.
    cursor: usize,
    /// The shell already draws its own suggestion after the cursor (SPEC §8.1).
    shell_ghost: bool,
    /// The cursor is on the command line (see `extract`): false on the rows an inline TUI draws below it (fzf
    /// `--height`, fish's pager search field), whose cells carry the open cycle's tag too.
    at_prompt: bool,
}

/// Column the model's cursor is on (raw: a pending wrap counts as the last column).
fn cursor_column(model: &Model) -> usize {
    model.grid().cursor.point.column.0
}

/// Shell-integration state of one wrapper session; owns the PTY output path (splitter → model → filter). The marker
/// state lives in the domain's `MarkerTracker`; this composite applies its effects to the model (interim, until the
/// screen port replaces it).
pub struct Tracker {
    pub marker: MarkerTracker,
    /// Drained by the wrapper after every batch.
    pub events: Vec<ShellEvent>,
    pub filter: OutputFilter,
    splitter: MarkerSplitter,
    proc: Processor,
}

impl Tracker {
    /// `cfg!(windows)` decides the ConPTY deferral of EndPrompt/NewCmd (SPEC §5.1.4).
    pub fn new(sid: String, shell: Option<Shell>, rows: usize) -> Tracker {
        Tracker::with_defer(sid, shell, rows, cfg!(windows))
    }

    pub fn with_defer(sid: String, shell: Option<Shell>, rows: usize, defer: bool) -> Tracker {
        let session_id = SessionId::new(sid).expect("generated ids are valid");
        Tracker {
            marker: MarkerTracker::new(session_id, shell, defer),
            events: Vec::new(),
            filter: OutputFilter::new(rows),
            splitter: MarkerSplitter::default(),
            proc: Processor::new(),
        }
    }

    /// One PTY batch (SPEC §4.5): markers handled at their byte offset, window names appended (filtered) to `out`
    /// only, everything else fed to the model and appended (filtered) to `out` for the real terminal.
    pub fn feed(&mut self, model: &mut Model, batch: &[u8], out: &mut Vec<u8>) {
        for piece in self.splitter.feed(batch) {
            match piece {
                OutputPiece::Bytes(b) => {
                    self.advance(model, &b);
                    self.filter.feed(&b, out);
                }
                OutputPiece::WindowName(b) => self.filter.feed(&b, out),
                OutputPiece::Marker(p) => self.handle_marker(&p, model),
            }
        }
        // Windows: apply deferred markers once a batch brings no new ones (FIFO).
        let effects = self.marker.end_batch(cursor_column(model), now_secs());
        self.apply(model, effects);
    }

    fn handle_marker(&mut self, payload: &str, model: &mut Model) {
        let marker = ShellMarker::parse(payload);
        match self
            .marker
            .on_marker(marker, cursor_column(model), now_secs())
        {
            MarkerReaction::Handled(effects) => self.apply(model, effects),
            MarkerReaction::EditBufferNeeded(effects) => {
                self.apply(model, effects);
                let scraped = self.extract(model);
                let effects = self.marker.complete_pre_exec(scraped.as_ref(), now_secs());
                self.apply(model, effects);
            }
        }
    }

    /// The screen commands reach the model right away and in order; the events wait for the wrapper.
    fn apply(&mut self, model: &mut Model, effects: MarkerEffects) {
        for &command in effects.screen_commands() {
            cell_tagger::apply(model, command);
        }
        self.events.extend(effects.into_events());
    }

    /// Feed bytes to the model only. Synchronized updates are applied at once: the model mirrors terminal state.
    pub fn advance(&mut self, model: &mut Model, bytes: &[u8]) {
        self.proc.advance(model, bytes);
        if self.proc.sync_timeout().sync_timeout().is_some() {
            self.proc.stop_sync(model);
        }
    }

    /// At a prompt with a known command cycle (an edit buffer exists).
    pub fn at_prompt(&self) -> bool {
        self.marker.at_prompt()
    }

    /// Suggestion-coloured cell (SPEC §5.1.3; pwsh: the reported prediction colour, else dim or italic).
    fn suggestion(context: &EditBufferContext, cell: &Cell) -> bool {
        match context.shell() {
            Some(Shell::Zsh) => context.zsh_color().is_some_and(|c| matches_cell(c, cell)),
            Some(Shell::Fish) => context.fish_color().is_some_and(|c| matches_cell(c, cell)),
            Some(Shell::Pwsh) => match context.pwsh_color() {
                Some(c) => c.matches(cell_col(cell.fg), cell_col(cell.bg), cell_emphasis(cell.flags)),
                None => cell_emphasis(cell.flags).is_dim_or_italic(),
            },
            _ => false,
        }
    }

    /// Edit buffer + cursor from the tagged cells around the cursor (SPEC §5.6). None while no cycle is open.
    pub fn extract(&self, model: &Model) -> Option<EditBuffer> {
        self.marker
            .edit_buffer_context()
            .and_then(|context| Tracker::extract_with(&context, model))
    }

    fn extract_with(context: &EditBufferContext, model: &Model) -> Option<EditBuffer> {
        let n = context.cycle();
        let tag = CellTag::Cycle(n).uri();
        let grid = model.grid();
        let cols = grid.columns();
        let cur = &grid.cursor;
        // Pending wrap = the cursor sits past the last column.
        let (cl, cc) = (
            cur.point.line.0,
            if cur.input_needs_wrap {
                cols
            } else {
                cur.point.column.0
            },
        );
        let row = |r: i32| &grid[Line(r)][..Column(cols)];
        let has = |c: &Cell, t: &str| c.hyperlink().is_some_and(|h| h.uri() == t);
        // Row r continues on r + 1: auto-wrap, or this cycle's text fills it to the last column and goes on at the
        // next row's first column after the shell's own CRLF, which sets no WRAPLINE (fish rewriting a wrapped line
        // after an edit or re-highlight on an earlier row). Not a prompt cell there: a right prompt at the margin
        // (zsh ZLE_RPROMPT_INDENT=0, fish) would join an inline TUI's rows below (fzf --layout=reverse) to the line.
        // ponytail: a real newline right after a line exactly as wide as the terminal reads as a soft wrap.
        let bottom = grid.bottommost_line().0;
        let wraps = |r: i32| {
            let last = &row(r)[cols - 1];
            last.flags.contains(Flags::WRAPLINE)
                || (r < bottom && has(last, &tag) && has(&row(r + 1)[0], &tag))
        };
        let tagged = |r: i32| row(r).iter().any(|c| has(c, &tag));

        let mut start = cl;
        while start > grid.topmost_line().0
            && cl - start < 100
            && (wraps(start - 1) || tagged(start - 1))
        {
            start -= 1;
        }
        // Below the cursor only wrap continuations and PS2 rows count (not zsh completion listings, not PSReadLine's
        // ListView, whose padded rows can look like wrap continuations).
        let pwsh = context.shell() == Some(Shell::Pwsh);
        let mut end = cl;
        while !(pwsh && context.pwsh_list_view())
            && end < grid.bottommost_line().0
            && (wraps(end) || (has(&row(end + 1)[0], CellTag::PROMPT_URI) && tagged(end + 1)))
        {
            end += 1;
        }

        // At the prompt = the cursor's line (rows joined by auto-wrap) starts on a prompt: a prompt cell (PS1's last
        // line, PS2, a prompt drawn without markers: NewCmd tags it) or PSReadLine's ContinuationPrompt. A NewCmd at
        // column 0 (PS1 ending in a newline) leaves its line no prompt cell: there, a prompt row right above and
        // nothing of this cycle drawn below (an inline TUI's query row can sit on that very row: fzf Ctrl-T, reverse
        // layout, from an empty line). An inline TUI's rows below the line (fzf --height, fish's pager search field)
        // are none of these; the buffer walk above still joins them, for pre_exec's scrape (a transient prompt is
        // redrawn without markers).
        // ponytail: continuation lines without a prompt (zle Alt-Enter, fish), an empty PS1, a markerless prompt
        // ending in a newline, and a newline-ended PS1's line while zsh lists completions below it read as not at
        // the prompt; recording the NewCmd row with scroll tracking is the upgrade.
        let mut top = cl;
        while top > grid.topmost_line().0 && cl - top < 100 && wraps(top - 1) {
            top -= 1;
        }
        let prompt_row = |r: i32| row(r).iter().any(|c| has(c, CellTag::PROMPT_URI));
        // A prompt filling its row to the margin (zsh then moves on with its own CRLF): like a PS1 ending in a newline.
        let full_prompt =
            |r: i32| r >= grid.topmost_line().0 && row(r).iter().all(|c| has(c, CellTag::PROMPT_URI));
        let cont = context.pwsh_continuation().chars().count();
        let below = end < grid.bottommost_line().0 && tagged(end + 1);
        let at_prompt = (top..=cl).any(prompt_row)
            || (pwsh
                && cont > 0
                && row(top)
                    .iter()
                    .take(cont)
                    .map(|c| c.c)
                    .eq(context.pwsh_continuation().chars()))
            || ((context.line_starts_at_column_zero() || full_prompt(top - 1))
                && top > grid.topmost_line().0
                && prompt_row(top - 1)
                && !below);
        let mut e = Scraped {
            at_prompt,
            ..Scraped::default()
        };
        let mut cursor = None;
        let (mut pad, mut line_has_cmd) = (0, false);
        // Blanks between command cells and the cursor are buffer text (zsh skips them with CUF on a redraw).
        let mut at_cursor = |e: &mut Scraped, pad: &mut usize, line_has_cmd: bool| {
            if line_has_cmd {
                e.buffer.extend(std::iter::repeat_n(' ', *pad));
                *pad = 0;
            }
            cursor = Some(e.buffer.len());
        };
        for r in start..=end {
            if r == start || !wraps(r - 1) {
                // ponytail: untagged cells before the first command cell of a line (prompt) are not padding.
                pad = 0;
                line_has_cmd = false;
            }
            let mut row_has_cmd = false;
            for (c, cell) in row(r).iter().enumerate() {
                if (r, c) == (cl, cc) {
                    at_cursor(&mut e, &mut pad, line_has_cmd);
                }
                if cell
                    .flags
                    .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
                {
                    continue;
                }
                let after = (r, c) >= (cl, cc);
                let cmd = has(cell, &tag);
                if cmd && !(after && Tracker::suggestion(context, cell)) {
                    if line_has_cmd {
                        e.buffer.extend(std::iter::repeat_n(' ', pad));
                    }
                    pad = 0;
                    line_has_cmd = true;
                    row_has_cmd = true;
                    e.buffer.push(cell.c);
                    e.buffer.extend(cell.zerowidth().into_iter().flatten());
                } else {
                    pad += 1;
                    e.shell_ghost |= cmd;
                }
            }
            if r == cl && cc >= cols {
                at_cursor(&mut e, &mut pad, line_has_cmd);
            }
            if r < end && !wraps(r) && (row_has_cmd || !e.buffer.is_empty()) {
                e.buffer.push('\n');
            }
        }
        e.cursor = cursor.unwrap_or(e.buffer.len());
        if pwsh {
            Tracker::strip_pwsh_prompts(context, &mut e);
        }
        // Blank command cells after the cursor are drawing leftovers.
        let keep = e.buffer.trim_end().len().max(e.cursor);
        e.buffer.truncate(keep);
        Some(EditBuffer::new(e.buffer, e.cursor, e.shell_ghost, e.at_prompt))
    }

    /// PSReadLine draws its ContinuationPrompt (each line after a real `\n`) and a re-rendered PromptText (first
    /// line) after EndPrompt, so they carry the command tag: drop them, keeping the cursor on the same character.
    /// ponytail: a first line the user typed starting with the PromptText loses it too.
    fn strip_pwsh_prompts(context: &EditBufferContext, e: &mut Scraped) {
        let (mut out, mut off, mut shift) = (String::with_capacity(e.buffer.len()), 0, 0);
        for (i, line) in e.buffer.split('\n').enumerate() {
            let p = if i == 0 {
                context
                    .pwsh_prompt_text()
                    .iter()
                    .find(|p| line.starts_with(p.as_str()))
                    .map_or(0, |p| p.len())
            } else if !context.pwsh_continuation().is_empty()
                && line.starts_with(context.pwsh_continuation())
            {
                context.pwsh_continuation().len()
            } else {
                0
            };
            if i > 0 {
                out.push('\n');
            }
            out.push_str(&line[p..]);
            shift += p.min(e.cursor.saturating_sub(off));
            off += line.len() + 1;
        }
        e.buffer = out;
        e.cursor -= shift;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal_session::domain::value_objects::deferred_marker::DeferredMarker;
    use std::path::Path;
    use alacritty_terminal::event::VoidListener;
    use alacritty_terminal::term::test::TermSize;
    use alacritty_terminal::term::{Config, Term};
    use crate::shared::infrastructure::testing::fixture_log::assert_matches_fixture;

    const SID: &str = "0123456789abcdef0123456789abcdef";
    const SP: &str = "\x1b]697;StartPrompt\x07";
    const EP: &str = "\x1b]697;EndPrompt\x07";

    fn nc() -> String {
        format!("\x1b]697;NewCmd={SID}\x07")
    }

    fn osc(p: &str) -> String {
        format!("\x1b]697;{p}\x1b\\")
    }

    fn setup(cols: usize, rows: usize, shell: Shell) -> (Tracker, Model) {
        (
            Tracker::with_defer(SID.into(), Some(shell), rows, false),
            Term::new(Config::default(), &TermSize::new(cols, rows), VoidListener),
        )
    }

    fn feed(t: &mut Tracker, m: &mut Model, s: &str) -> Vec<u8> {
        let mut out = Vec::new();
        t.feed(m, s.as_bytes(), &mut out);
        out
    }

    fn buf(t: &Tracker, m: &Model) -> (String, usize) {
        let e = t.extract(m).expect("at prompt");
        (e.buffer().to_string(), e.cursor())
    }

    fn prompt(p: &str) -> String {
        format!("{SP}{p}{EP}{}", nc())
    }

    /// A precmd's markers: unlock, then `extra` markers, then the prompt.
    fn precmd(extra: &[&str], p: &str) -> String {
        let markers: String = extra.iter().map(|m| osc(m)).collect();
        format!("{}{markers}{}", osc(&format!("OSCUnlock={SID}")), prompt(p))
    }

    #[test]
    fn markers_stripped_from_output() {
        let (mut t, mut m) = setup(40, 5, Shell::Zsh);
        let out = feed(
            &mut t,
            &mut m,
            &format!("{}\x1b]0;t\x07{}x", osc("Dir=/tmp"), prompt("% ")),
        );
        assert_eq!(out, b"\x1b]0;t\x07% x");
        assert_eq!(t.marker.cwd(), Some(Path::new("/tmp")));
        assert_eq!(buf(&t, &m), ("x".into(), 1));
    }

    #[test]
    fn window_name_reaches_the_terminal_but_is_not_printed() {
        // screen's `ESC k name ST` (oh-my-zsh titles inside tmux): tmux names the window, the model would print `vim`,
        // drift right and, once PROMPT_SP pads the line to the margin, wrap a row below the real cursor.
        let (mut t, mut m) = setup(40, 5, Shell::Zsh);
        let shell_output = "ab\x1bkvim\x1b\\cd";

        let out = feed(&mut t, &mut m, shell_output);

        assert_eq!(out, shell_output.as_bytes());
        assert_eq!(m.grid().cursor.point.column.0, 4);
    }

    #[test]
    fn lock_and_foreign_session_ids() {
        let (mut t, mut m) = setup(40, 5, Shell::Zsh);
        feed(
            &mut t,
            &mut m,
            &format!("{}{}", osc("OSCLock=other"), osc("Dir=/a")),
        );
        assert!(!t.marker.osc_lock());
        assert_eq!(t.marker.cwd(), Some(Path::new("/a")));
        feed(
            &mut t,
            &mut m,
            &format!(
                "{}{}{}",
                osc(&format!("OSCLock={SID}")),
                osc("Dir=/b"),
                osc("OSCUnlock=other")
            ),
        );
        assert!(t.marker.osc_lock());
        assert_eq!(t.marker.cwd(), Some(Path::new("/a")));
        feed(
            &mut t,
            &mut m,
            &format!("{}{}", osc(&format!("OSCUnlock={SID}")), osc("Dir=/c")),
        );
        assert!(!t.marker.osc_lock());
        assert_eq!(t.marker.cwd(), Some(Path::new("/c")));
        // Empty values are ignored.
        feed(&mut t, &mut m, &osc("Dir="));
        assert_eq!(t.marker.cwd(), Some(Path::new("/c")));
        // NewCmd from a nested session, or without a sid, is ignored; only ours opens a cycle.
        feed(&mut t, &mut m, &osc("NewCmd=ffff"));
        assert_eq!(t.marker.cycle(), None);
        feed(&mut t, &mut m, &osc("NewCmd"));
        assert_eq!(t.marker.cycle(), None);
        feed(&mut t, &mut m, &nc());
        assert_eq!(t.marker.cycle(), Some(1));
    }

    #[test]
    fn bare_newcmd_cannot_fake_a_prompt_while_locked() {
        let (mut t, mut m) = setup(40, 5, Shell::Bash);
        feed(&mut t, &mut m, &prompt("$ "));
        feed(
            &mut t,
            &mut m,
            &format!(
                "cat x\r\n{}{}",
                osc(&format!("OSCLock={SID}")),
                osc("PreExec")
            ),
        );
        assert!(t.marker.preexec());
        // `cat` of untrusted output prints a bare NewCmd: the command keeps running.
        feed(&mut t, &mut m, "\x1b]697;NewCmd\x07");
        assert!(t.marker.preexec() && !t.at_prompt());
        assert_eq!(t.marker.cycle(), Some(1));
    }

    #[test]
    fn command_cycle_produces_history() {
        let (mut t, mut m) = setup(40, 6, Shell::Zsh);
        feed(
            &mut t,
            &mut m,
            &format!("{}{}{}", osc("Shell=zsh"), osc("PID=42"), prompt("% ")),
        );
        assert_eq!(t.events, [ShellEvent::NewCommand(None)]);
        t.events.clear();
        feed(&mut t, &mut m, "ls -l");
        feed(
            &mut t,
            &mut m,
            &format!(
                "\r\n{}{}file\r\n",
                osc(&format!("OSCLock={SID}")),
                osc("PreExec")
            ),
        );
        assert_eq!(t.events, [ShellEvent::PreExec]);
        assert!(t.extract(&m).is_none());
        t.events.clear();
        feed(
            &mut t,
            &mut m,
            &format!(
                "{}{}{}",
                osc(&format!("OSCUnlock={SID}")),
                osc("ExitCode=2"),
                prompt("% ")
            ),
        );
        let [ShellEvent::NewCommand(Some(h))] = &t.events[..] else {
            panic!("{:?}", t.events)
        };
        let h = h.to_primitives();
        assert_eq!(
            (h.command.as_str(), h.exit_code, h.pid, h.shell.as_str()),
            ("ls -l", Some(2), Some(42), "zsh")
        );
        assert_eq!(h.session_id, SID);
        assert_eq!(buf(&t, &m), (String::new(), 0));
    }

    #[test]
    fn extract_zsh_prompt_and_command() {
        let (mut t, mut m) = setup(40, 5, Shell::Zsh);
        feed(&mut t, &mut m, &format!("{}git st", prompt("~/src % ")));
        assert_eq!(buf(&t, &m), ("git st".into(), 6));
        // Cursor moved left: byte offset of the cursor.
        feed(&mut t, &mut m, "\x1b[2D");
        assert_eq!(buf(&t, &m), ("git st".into(), 4));
    }

    #[test]
    fn extract_ignores_rprompt() {
        let (mut t, mut m) = setup(40, 5, Shell::Zsh);
        feed(
            &mut t,
            &mut m,
            &format!("{}\x1b[35G{SP}[rp]{EP}\x1b[3Gls", prompt("% ")),
        );
        assert_eq!(buf(&t, &m), ("ls".into(), 2));
    }

    #[test]
    fn extract_ps2_multiline() {
        let (mut t, mut m) = setup(40, 5, Shell::Zsh);
        feed(
            &mut t,
            &mut m,
            &format!("{}echo 'a\r\n{SP}> {EP}b", prompt("% ")),
        );
        assert_eq!(buf(&t, &m), ("echo 'a\nb".into(), 9));
        // Cursor back on the first line: the PS2 row below still counts.
        feed(&mut t, &mut m, "\x1b[A\x1b[4G");
        assert_eq!(buf(&t, &m), ("echo 'a\nb".into(), 1));
    }

    #[test]
    fn extract_wrapped_long_command() {
        let (mut t, mut m) = setup(20, 5, Shell::Bash);
        let cmd = "abcdefghijklmnopqrstuvwxyz0123456789";
        feed(&mut t, &mut m, &format!("{}{cmd}", prompt("$ ")));
        assert_eq!(buf(&t, &m), (cmd.into(), cmd.len()));
        // Exactly at the right margin (pending wrap) the cursor is at the end.
        let (mut t, mut m) = setup(20, 5, Shell::Bash);
        feed(&mut t, &mut m, &format!("{}{}", prompt("$ "), &cmd[..18]));
        assert_eq!(buf(&t, &m), (cmd[..18].into(), 18));
        // Reflow on resize keeps the buffer.
        let (mut t, mut m) = setup(20, 5, Shell::Bash);
        feed(&mut t, &mut m, &format!("{}{cmd}", prompt("$ ")));
        m.resize(TermSize::new(15, 5));
        assert_eq!(buf(&t, &m), (cmd.into(), cmd.len()));
    }

    #[test]
    fn extract_excludes_completion_listing_below() {
        let (mut t, mut m) = setup(40, 6, Shell::Zsh);
        feed(
            &mut t,
            &mut m,
            &format!(
                "{}git ch\r\ncheckout  cherry-pick\r\ncherry\x1b[2A\r\x1b[8C",
                prompt("% ")
            ),
        );
        assert_eq!(buf(&t, &m), ("git ch".into(), 6));
    }

    #[test]
    fn extract_ghost_after_cursor_vs_typed_grey() {
        let (mut t, mut m) = setup(40, 5, Shell::Zsh);
        feed(
            &mut t,
            &mut m,
            &format!(
                "{}{}git st\x1b[90matus\x1b[0m\x1b[4D",
                osc("ZshAutosuggestionColor=fg=8"),
                prompt("% ")
            ),
        );
        let e = t.extract(&m).unwrap();
        assert_eq!(
            (e.buffer(), e.cursor(), e.shell_ghost()),
            ("git st", 6, true)
        );

        let (mut t, mut m) = setup(40, 5, Shell::Zsh);
        feed(
            &mut t,
            &mut m,
            &format!(
                "{}{}\x1b[38;5;8mgit\x1b[0m st",
                osc("ZshAutosuggestionColor=fg=8"),
                prompt("% ")
            ),
        );
        let e = t.extract(&m).unwrap();
        assert_eq!(
            (e.buffer(), e.cursor(), e.shell_ghost()),
            ("git st", 6, false)
        );

        // pwsh: PSReadLine predictions are dim/italic.
        let (mut t, mut m) = setup(40, 5, Shell::Pwsh);
        feed(
            &mut t,
            &mut m,
            &format!("{}ls\x1b[3m -Force\x1b[0m\x1b[7D", prompt("PS> ")),
        );
        let e = t.extract(&m).unwrap();
        assert_eq!(
            (e.buffer(), e.cursor(), e.shell_ghost()),
            ("ls", 2, true)
        );
    }

    #[test]
    fn extract_keeps_blank_skipped_before_cursor() {
        // zsh redraw (Ctrl-L, reset-prompt) moves over the trailing blank with CUF instead of writing it.
        let (mut t, mut m) = setup(40, 5, Shell::Zsh);
        feed(
            &mut t,
            &mut m,
            &format!("{}git \r{}git\x1b[K\x1b[1C", prompt("% "), prompt("% ")),
        );
        assert_eq!(buf(&t, &m), ("git ".into(), 4));
        // Blanks after the prompt alone are not buffer text; blanks after the cursor are still trimmed.
        feed(&mut t, &mut m, &format!("\r{}\x1b[K\x1b[2C", prompt("% ")));
        assert_eq!(buf(&t, &m), (String::new(), 0));
        feed(&mut t, &mut m, &format!("\r{}ls -l\x1b[2D", prompt("% ")));
        assert_eq!(buf(&t, &m), ("ls -l".into(), 3));
    }

    #[test]
    fn pwsh_reported_prediction_colour() {
        // A custom InlinePredictionColor (plain grey, neither dim nor italic) is excluded after the cursor.
        let (mut t, mut m) = setup(40, 5, Shell::Pwsh);
        let grey = "git ch\x1b[90meckout main\x1b[0m\x1b[11D";
        feed(
            &mut t,
            &mut m,
            &format!(
                "{}{}{grey}",
                osc("PwshPredictionColor=[90m"),
                prompt("PS> ")
            ),
        );
        let e = t.extract(&m).unwrap();
        assert_eq!(
            (e.buffer(), e.cursor(), e.shell_ghost()),
            ("git ch", 6, true)
        );
        // The default colour, reported: bright white + dim + italic; a plain bright white cell is typed text.
        feed(
            &mut t,
            &mut m,
            &format!(
                "\r\x1b[K{}{}ls\x1b[97;2;3m -Force\x1b[0;97m x\x1b[0m\x1b[9D",
                osc("PwshPredictionColor=[97;2;3m"),
                prompt("PS> ")
            ),
        );
        let e = t.extract(&m).unwrap();
        assert_eq!((e.buffer(), e.cursor()), ("ls        x", 2));
    }

    #[test]
    fn no_history_marker_drops_the_command() {
        let (mut t, mut m) = setup(40, 6, Shell::Fish);
        let run = |t: &mut Tracker, m: &mut Model, cmd: &str, private: bool| {
            let nh = if private {
                osc("NoHistory")
            } else {
                String::new()
            };
            feed(
                t,
                m,
                &format!(
                    "{cmd}\r\n{}{nh}{}out\r\n{}",
                    osc(&format!("OSCLock={SID}")),
                    osc("PreExec"),
                    precmd(&["ExitCode=0"], "> ")
                ),
            );
            std::mem::take(&mut t.events)
        };
        feed(&mut t, &mut m, &prompt("> "));
        t.events.clear();
        // Sent while locked (fish emits it after OSCLock): still honoured; the menu still resets.
        // r3-nohistory-skips-command-finish: it still ends (the engine resets its generator results).
        assert_eq!(
            run(&mut t, &mut m, " set -l TOKEN hunter2", true),
            [ShellEvent::PreExec, ShellEvent::CommandEnded]
        );
        assert!(!t.marker.no_history());
        let ev = run(&mut t, &mut m, "ls", false);
        assert!(
            matches!(&ev[..], [ShellEvent::PreExec, ShellEvent::NewCommand(Some(h))] if h.command() == "ls"),
            "{ev:?}"
        );
        // A running command printing the marker cannot hide itself or the next command.
        feed(
            &mut t,
            &mut m,
            &format!(
                "make\r\n{}{}",
                osc(&format!("OSCLock={SID}")),
                osc("PreExec")
            ),
        );
        feed(
            &mut t,
            &mut m,
            &format!("{}{}", osc("NoHistory"), precmd(&[], "> ")),
        );
        assert!(
            matches!(&t.events[..], [ShellEvent::PreExec, ShellEvent::NewCommand(Some(h))] if h.command() == "make"),
            "{:?}",
            t.events
        );
    }

    #[test]
    fn submitted_line_drops_shell_output_before_preexec() {
        let (mut t, mut m) = setup(40, 6, Shell::Bash);
        let finish = |t: &mut Tracker, m: &mut Model, screen: &str, submitted: &str| {
            t.events.clear();
            feed(t, m, &prompt("$ "));
            feed(t, m, screen);
            t.marker.set_submitted(Some(submitted.into()));
            feed(
                t,
                m,
                &format!(
                    "{}{}\r\n{}",
                    osc(&format!("OSCLock={SID}")),
                    osc("PreExec"),
                    precmd(&[], "$ ")
                ),
            );
            let Some(ShellEvent::NewCommand(Some(h))) = t.events.last() else {
                panic!("{:?}", t.events)
            };
            assert_eq!(t.marker.submitted(), None);
            h.command().to_string()
        };
        // bash/zsh echo the `!!` expansion (and zsh its correction prompt) before PreExec.
        assert_eq!(
            finish(&mut t, &mut m, "echo !!\r\necho echo first", "echo !! "),
            "echo !!"
        );
        // A line the shell rewrote on Enter (fish abbreviation, PS2 loop) is taken from the screen.
        assert_eq!(
            finish(&mut t, &mut m, "git checkout", "gco"),
            "git checkout"
        );
        assert_eq!(
            finish(
                &mut t,
                &mut m,
                &format!("echo 'a\r\n{SP}> {EP}b'"),
                "echo 'a\nb'"
            ),
            "echo 'a\nb'"
        );
    }

    #[test]
    fn bash_redraw_of_the_prompt_alone_keeps_the_line() {
        // bash-readline-reused-cells-lose-buffer: leaving i-search deletes its prefix (DCH) and reprints only the
        // prompt and the changed start; the shifted rest of the line stays.
        let (mut t, mut m) = setup(80, 5, Shell::Bash);
        feed(
            &mut t,
            &mut m,
            &format!(
                "{}\r(reverse-i-search)`onel': git log --oneline -1 feature-",
                prompt("$ ")
            ),
        );
        feed(
            &mut t,
            &mut m,
            &format!(
                "\r\x1b[24P{}git log --onel{}",
                prompt("$ "),
                "\x08".repeat(5)
            ),
        );
        assert_eq!(buf(&t, &m), ("git log --oneline -1 feature-".into(), 9));
        // vi mode with show-mode-in-prompt: the mode string is redrawn and the line moved over with CUF.
        let (mut t, mut m) = setup(40, 5, Shell::Bash);
        feed(&mut t, &mut m, &format!("(ins){}git ch", prompt("$ ")));
        feed(&mut t, &mut m, &format!("\r(cmd){}\x1b[5C", prompt("$ ")));
        feed(&mut t, &mut m, &format!("\r(ins){}\x1b[6Ce", prompt("$ ")));
        assert_eq!(buf(&t, &m), ("git che".into(), 7));
        // A two-line prompt: only its last line is redrawn, without StartPrompt; it is still the prompt.
        let (mut t, mut m) = setup(40, 5, Shell::Bash);
        feed(
            &mut t,
            &mut m,
            &format!("{SP}repo\r\n[I]> {EP}{}git ch", nc()),
        );
        feed(&mut t, &mut m, &format!("\r[N]> {EP}{}\x1b[5C", nc()));
        let e = t.extract(&m).unwrap();
        assert_eq!(
            (e.buffer(), e.cursor(), e.at_prompt()),
            ("git ch", 5, true)
        );
        // A finished command's cells are not taken over by the next prompt.
        feed(
            &mut t,
            &mut m,
            &format!("\r\n{}{}", osc("PreExec"), prompt("$ ")),
        );
        assert_eq!(buf(&t, &m), (String::new(), 0));
    }

    #[test]
    fn at_prompt_only_on_the_command_line() {
        let at = |t: &Tracker, m: &Model| t.extract(m).map(|e| (e.buffer().to_string(), e.at_prompt()));
        let (mut t, mut m) = setup(40, 12, Shell::Zsh);
        // R4-7: fzf Ctrl-T after typed text draws its query row below the line (no wrap, no prompt).
        feed(
            &mut t,
            &mut m,
            &format!("{}cat \r\n  3/3\r\n> read", prompt("% ")),
        );
        assert_eq!(at(&t, &m).map(|e| e.1), Some(false));
        // fzf exits; the widget redraws the prompt and the line.
        feed(
            &mut t,
            &mut m,
            &format!("\r\x1b[2A\x1b[J{}cat readme.md", prompt("% ")),
        );
        assert_eq!(at(&t, &m), Some(("cat readme.md".into(), true)));
        // fish-ctrl-r-pager-hijacked: the history pager's search field below the line.
        let (mut t, mut m) = setup(40, 12, Shell::Fish);
        feed(
            &mut t,
            &mut m,
            &format!(
                "{}git s\r\nsearch: git s\x1b[s\r\n\u{25ba} git status --short\x1b[u",
                prompt("> ")
            ),
        );
        assert_eq!(at(&t, &m).map(|e| e.1), Some(false));
        // A long line wrapped onto the next rows is still the line; so is a PS2 row.
        let (mut t, mut m) = setup(20, 8, Shell::Bash);
        feed(
            &mut t,
            &mut m,
            &format!("{}abcdefghijklmnopqrstuvwxyz0123456789", prompt("$ ")),
        );
        assert_eq!(at(&t, &m).map(|e| e.1), Some(true));
        feed(&mut t, &mut m, &format!("\r\n{SP}> {EP}b"));
        assert_eq!(at(&t, &m).map(|e| e.1), Some(true));
        // pwsh: PSReadLine's ContinuationPrompt (drawn after EndPrompt) starts a line of the command too.
        let (mut t, mut m) = setup(40, 8, Shell::Pwsh);
        feed(
            &mut t,
            &mut m,
            &format!("{}ls |\r\n>> git ch", prompt("PS> ")),
        );
        assert_eq!(at(&t, &m), Some(("ls |\ngit ch".into(), true)));
    }

    #[test]
    fn soft_wrap_redrawn_with_crlf_is_still_the_line() {
        // fish redraws a wrapped line after an edit on its first row: up to the margin, then CRLF (no WRAPLINE).
        let at = |t: &Tracker, m: &Model| t.extract(m).map(|e| (e.buffer().to_string(), e.at_prompt()));
        let (mut t, mut m) = setup(20, 8, Shell::Fish);
        feed(
            &mut t,
            &mut m,
            &format!("{}abcdefghijklmnopqr\r\nstu", prompt("$ ")),
        );
        assert_eq!(at(&t, &m), Some(("abcdefghijklmnopqrstu".into(), true)));
        let (mut t, mut m) = setup(20, 8, Shell::Zsh);
        feed(
            &mut t,
            &mut m,
            &format!("{}\r\ngit ch", prompt("xxxxxxxxxxxxxxxxxx% ")),
        );
        assert_eq!(at(&t, &m), Some(("git ch".into(), true)));
        // A right prompt at the margin does not join an inline TUI's query row below (fzf --layout=reverse).
        let (mut t, mut m) = setup(20, 8, Shell::Zsh);
        feed(
            &mut t,
            &mut m,
            &format!("{}cat\x1b[18G{SP}[R]{EP}\r\n> rea", prompt("$ ")),
        );
        assert_eq!(at(&t, &m).map(|e| e.1), Some(false));
    }

    #[test]
    fn prompt_ending_in_a_newline_is_at_the_prompt() {
        // r4-newline-terminated-prompt-no-menu: PS1 ends in `\n`, so the line's row holds no prompt cell.
        let at = |t: &Tracker, m: &Model| t.extract(m).map(|e| (e.buffer().to_string(), e.at_prompt()));
        let (mut t, mut m) = setup(20, 12, Shell::Zsh);
        // The prompt's own line wraps too.
        feed(
            &mut t,
            &mut m,
            &format!("{SP}~/a/very/long/path/here\r\n{EP}{}git ch", nc()),
        );
        assert_eq!(at(&t, &m), Some(("git ch".into(), true)));
        // fzf (default layout) from an empty line starts on the line's row: its query row is not the line.
        feed(
            &mut t,
            &mut m,
            &format!(
                "\r\n{}{SP}~\r\n{EP}{}  a.txt\r\n  b.txt\r\n  2/2\r\n> rea",
                osc("PreExec"),
                nc()
            ),
        );
        assert_eq!(at(&t, &m).map(|e| e.1), Some(false));
        // After typed text it starts below the line.
        feed(
            &mut t,
            &mut m,
            &format!("\r\x1b[4A\x1b[J{SP}~\r\n{EP}{}cat \r\n> rea", nc()),
        );
        assert_eq!(at(&t, &m).map(|e| e.1), Some(false));
        // Reverse layout (fzf's Ctrl-T default) from an empty line: the query row is the line's own row.
        feed(
            &mut t,
            &mut m,
            &format!(
                "\r\x1b[2A\x1b[J{SP}~\r\n{EP}{}\x1b[s\r\n  2/2\r\n  a.txt\x1b[u  rea",
                nc()
            ),
        );
        assert_eq!(at(&t, &m).map(|e| e.1), Some(false));
        // fzf exits (clearing its rows); the line is back.
        feed(
            &mut t,
            &mut m,
            &format!("\r\x1b[J\x1b[A{SP}~\r\n{EP}{}cat a.txt", nc()),
        );
        assert_eq!(at(&t, &m), Some(("cat a.txt".into(), true)));
    }

    #[test]
    fn prompt_drawn_without_markers_is_at_the_prompt() {
        // R4-1: pwsh re-wraps a foreign prompt at ReadLine and sends NewCmd without StartPrompt/EndPrompt.
        let (mut t, mut m) = setup(40, 8, Shell::Pwsh);
        feed(
            &mut t,
            &mut m,
            &format!(
                "{}PS top> {}{}git ch",
                osc("PreExec"),
                osc(&format!("OSCUnlock={SID}")),
                nc()
            ),
        );
        let e = t.extract(&m).unwrap();
        assert_eq!(
            (e.buffer(), e.cursor(), e.at_prompt()),
            ("git ch", 6, true)
        );
        // The same after a command ran.
        feed(
            &mut t,
            &mut m,
            &format!(
                "\r\n{}{}out\r\nPS top> {}{}ls",
                osc(&format!("OSCLock={SID}")),
                osc("PreExec"),
                osc(&format!("OSCUnlock={SID}")),
                nc()
            ),
        );
        let e = t.extract(&m).unwrap();
        assert_eq!((e.buffer(), e.at_prompt()), ("ls", true));
    }

    #[test]
    fn transient_prompt_before_preexec_keeps_the_submitted_line() {
        let (mut t, mut m) = setup(60, 8, Shell::Zsh);
        let run = |t: &mut Tracker, m: &mut Model, typed: &str, finish: &str| {
            feed(t, m, &format!("{}{typed}", prompt("% ")));
            t.marker.set_submitted(Some(typed.into()));
            feed(
                t,
                m,
                &format!(
                    "{finish}{}{}out\r\n{}",
                    osc(&format!("OSCLock={SID}")),
                    osc("PreExec"),
                    precmd(&[], "% ")
                ),
            );
            let Some(ShellEvent::NewCommand(Some(h))) = std::mem::take(&mut t.events).pop() else {
                panic!()
            };
            h.command().to_string()
        };
        // r3-transient-redraw-clears-submitted: zle-line-finish re-expands PS1 (NewCmd) between Enter and PreExec.
        let redraw = format!("\r{}echo !!\r\necho echo second\r\n", prompt("% "));
        assert_eq!(run(&mut t, &mut m, "echo !!", &redraw), "echo !!");
        // transient-prompt-pollutes-history: a transient prompt drawn without markers lands in the scrape.
        let transient = "\r\x1b[K\u{276f} gti status\r\nzsh: correct 'gti' to 'git' [nyae]? y\r\n";
        assert_eq!(run(&mut t, &mut m, "gti status", transient), "gti status");
        assert_eq!(
            run(&mut t, &mut m, "echo hi", "\r\x1b[K\u{276f} echo hi\r\n"),
            "echo hi"
        );
        // Only as whole words ending a line: a rewritten line is still read from the screen.
        assert_eq!(
            run(&mut t, &mut m, "gco", "\r\x1b[K\u{276f} git checkout\r\n"),
            "\u{276f} git checkout"
        );
        assert_eq!(
            run(&mut t, &mut m, "ls", "\r\x1b[K\u{276f} lsd\r\n"),
            "\u{276f} lsd"
        );
    }

    #[test]
    fn nested_shell_prompt_keeps_outer_command_running() {
        let (mut t, mut m) = setup(40, 8, Shell::Fish);
        let at = |pid: u32, exit: u32| {
            precmd(&[&format!("PID={pid}"), &format!("ExitCode={exit}")], "> ")
        };
        let run = |t: &mut Tracker, m: &mut Model, cmd: &str, then: String| {
            feed(
                t,
                m,
                &format!(
                    "{cmd}\r\n{}{}{then}",
                    osc(&format!("OSCLock={SID}")),
                    osc("PreExec")
                ),
            );
            std::mem::take(&mut t.events)
                .into_iter()
                .filter_map(|e| {
                    if let ShellEvent::NewCommand(h) = e {
                        Some(h.map(|h| (h.command().to_string(), h.exit_code())))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        };
        feed(&mut t, &mut m, &at(100, 0));
        t.events.clear();
        // fish (100) → bash (200) → `false` → `exit 5` → fish prompt (exit 5).
        assert_eq!(run(&mut t, &mut m, "bash", at(200, 0)), [None]);
        assert_eq!(
            run(&mut t, &mut m, "false", at(200, 1)),
            [Some(("false".into(), Some(1)))]
        );
        let done = run(&mut t, &mut m, "exit 5", at(100, 5));
        assert_eq!(
            done,
            [
                Some(("exit 5".into(), Some(5))),
                Some(("bash".into(), Some(5)))
            ]
        );
        assert!(t.marker.pending_is_empty());
        // A prompt redraw of the same shell finishes nothing.
        feed(&mut t, &mut m, &format!("\r{}", prompt("> ")));
        assert_eq!(t.events, [ShellEvent::NewCommand(None)]);
    }

    #[test]
    fn pwsh_continuation_and_prompt_text_stripped() {
        // Default ContinuationPrompt `>> ` before the prompt reports it; the cursor keeps its character.
        let (mut t, mut m) = setup(40, 6, Shell::Pwsh);
        feed(
            &mut t,
            &mut m,
            &format!("{}if ($true) {{\r\n>> git ch", prompt("PS> ")),
        );
        assert_eq!(buf(&t, &m), ("if ($true) {\ngit ch".into(), 19));
        feed(&mut t, &mut m, "\x1b[2D");
        assert_eq!(buf(&t, &m), ("if ($true) {\ngit ch".into(), 17));
        // Reported prompts; PSReadLine re-renders the PromptText `> ` (red) on a parse error.
        let (mut t, mut m) = setup(40, 6, Shell::Pwsh);
        let prompts = osc("PwshPrompts=... \x1f> ");
        feed(
            &mut t,
            &mut m,
            &format!(
                "{prompts}{}echo (2\x1b[3G\x1b[31m> \x1b[0m\x1b[12G",
                prompt("PS> ")
            ),
        );
        assert_eq!(buf(&t, &m), ("echo (2".into(), 7));
        feed(&mut t, &mut m, "\r\n... )");
        assert_eq!(buf(&t, &m), ("echo (2\n)".into(), 9));
        // Cursor inside a stripped prompt: at its line start.
        feed(&mut t, &mut m, "\x1b[2G");
        assert_eq!(buf(&t, &m), ("echo (2\n)".into(), 8));
        feed(&mut t, &mut m, "\x1b[A\x1b[3G");
        assert_eq!(buf(&t, &m), ("echo (2".into(), 0));
    }

    #[test]
    fn pwsh_list_view_rows_not_in_buffer() {
        // PSReadLine pads the input row to the full width, so its ListView rows can look like a wrap continuation.
        let screen = format!(
            "{}git ch{}<-/4> > cherry-pick\x1b[1;11H",
            prompt("PS> "),
            " ".repeat(10)
        );
        let (mut t, mut m) = setup(20, 6, Shell::Pwsh);
        feed(&mut t, &mut m, &screen);
        assert_ne!(buf(&t, &m).0, "git ch");
        let (mut t, mut m) = setup(20, 6, Shell::Pwsh);
        feed(
            &mut t,
            &mut m,
            &format!("{}{screen}", osc("PwshListView=1")),
        );
        assert!(t.marker.pwsh_list_view());
        assert_eq!(buf(&t, &m), ("git ch".into(), 6));
        feed(&mut t, &mut m, &osc("PwshListView=0"));
        assert!(!t.marker.pwsh_list_view());
        feed(
            &mut t,
            &mut m,
            &format!("{}{}", osc("PwshListView=1"), osc("PreExec")),
        );
        assert!(
            !t.marker.pwsh_list_view(),
            "a command (maybe a nested shell) does not inherit it"
        );
    }

    #[test]
    fn extract_prompt_scrolled_by_output() {
        let (mut t, mut m) = setup(10, 3, Shell::Bash);
        feed(&mut t, &mut m, "1\r\n2\r\n3\r\n");
        let cmd = "echo hello world again";
        feed(&mut t, &mut m, &format!("{}{cmd}", prompt("$ ")));
        // The prompt row has scrolled into history; tags moved with it.
        assert!(m.grid().history_size() > 0);
        assert_eq!(buf(&t, &m), (cmd.into(), cmd.len()));
        // Command output below a finished command does not leak into the next buffer.
        feed(
            &mut t,
            &mut m,
            &format!("\r\n{}out\r\n{}x", osc("PreExec"), prompt("$ ")),
        );
        assert_eq!(buf(&t, &m), ("x".into(), 1));
    }

    #[test]
    fn windows_deferral_fifo() {
        let mut t = Tracker::with_defer(SID.into(), Some(Shell::Pwsh), 5, true);
        let mut m = Term::new(Config::default(), &TermSize::new(40, 5), VoidListener);
        feed(&mut t, &mut m, &format!("{SP}PS> {EP}{}", nc()));
        assert_eq!(t.marker.cycle(), None);
        assert_eq!(
            t.marker.deferred(),
            &[DeferredMarker::EndPrompt, DeferredMarker::NewCmd]
        );
        // A batch with markers keeps deferring.
        feed(&mut t, &mut m, &osc("Dir=/x"));
        assert_eq!(t.marker.cycle(), None);
        // A marker-free batch applies the queue in order, at the current cursor.
        feed(&mut t, &mut m, "\x1b[?25h");
        assert!(t.marker.deferred().is_empty());
        assert_eq!(
            (t.marker.cycle(), &t.events[..]),
            (Some(1), &[ShellEvent::NewCommand(None)][..])
        );
        feed(&mut t, &mut m, "ls");
        assert_eq!(buf(&t, &m), ("ls".into(), 2));
    }

    #[test]
    fn suggestion_colors_cell_matching() {
        let z = ShellSuggestionColor::parse_zsh("fg=8,bold").unwrap();
        // Cell colours: named, indexed and truecolor all map to xterm-256.
        let mut cell = Cell {
            fg: Color::Named(alacritty_terminal::vte::ansi::NamedColor::BrightBlack),
            ..Cell::default()
        };
        assert!(matches_cell(&z, &cell));
        cell.fg = Color::Spec(alacritty_terminal::vte::ansi::Rgb {
            r: 0xaa,
            g: 0xaa,
            b: 0xaa,
        });
        assert!(matches_cell(
            &ShellSuggestionColor::parse_zsh("fg=#aaaaaa").unwrap(),
            &cell
        ));
        assert!(!matches_cell(&z, &cell));
    }

    // --- Characterisation of the tracker at the start of Phase 9: scripted chunks, frozen text fixtures. ----------

    /// A tracker and its model, built the way the three characterisation scenarios need them.
    struct TrackerRig {
        t: Tracker,
        m: Model,
    }

    fn tracker_rig(cols: usize, rows: usize, shell: Shell, defer: bool) -> TrackerRig {
        let t = Tracker::with_defer(SID.into(), Some(shell), rows, defer);
        let m = Term::new(Config::default(), &TermSize::new(cols, rows), VoidListener);
        TrackerRig { t, m }
    }

    /// The hyperlink runs of the model, as ` row<r>:<from>..<to>=<uri>`.
    fn tags(m: &Model) -> String {
        use alacritty_terminal::grid::Dimensions;
        use alacritty_terminal::index::{Column, Line};
        let mut out = String::new();
        for r in 0..m.screen_lines() {
            let mut run: Option<(usize, usize, String)> = None;
            for c in 0..=m.columns() {
                let uri = (c < m.columns())
                    .then(|| {
                        m.grid()[Line(r as i32)][Column(c)]
                            .hyperlink()
                            .map(|h| h.uri().to_string())
                    })
                    .flatten();
                match (&mut run, uri) {
                    (Some((_, end, u)), Some(n)) if *u == n => *end = c + 1,
                    (_, n) => {
                        if let Some((a, b, u)) = run.take() {
                            out += &format!(" row{r}:{a}..{b}={u}");
                        }
                        run = n.map(|u| (c, c + 1, u));
                    }
                }
            }
        }
        out
    }

    fn describe_event(e: &ShellEvent) -> String {
        match e {
            ShellEvent::NewCommand(None) => "NewCmd(None)".into(),
            ShellEvent::NewCommand(Some(h)) => {
                let p = h.to_primitives();
                format!(
                    "NewCmd(command={:?} shell={:?} pid={:?} session_id={:?} cwd={:?} exit_code={:?} hostname={:?} end_time_set={})",
                    p.command,
                    p.shell,
                    p.pid,
                    p.session_id,
                    p.cwd,
                    p.exit_code,
                    p.hostname,
                    p.end_time.is_some()
                )
            }
            ShellEvent::CommandEnded => "CommandEnded".into(),
            ShellEvent::PreExec => "PreExec".into(),
        }
    }

    /// Feeds `chunks` one by one and logs, per chunk: the filtered bytes for the terminal, the events, the cycle
    /// state, the extract tuple and the hyperlink runs of the model.
    fn transcript<S: AsRef<str>>(rig: &mut TrackerRig, chunks: &[S]) -> String {
        let mut log = String::new();
        for (i, chunk) in chunks.iter().enumerate() {
            let chunk = chunk.as_ref();
            let out = feed(&mut rig.t, &mut rig.m, chunk);
            let events: Vec<String> = std::mem::take(&mut rig.t.events)
                .iter()
                .map(describe_event)
                .collect();
            let extract = rig
                .t
                .extract(&rig.m)
                .map(|e| (e.buffer().to_string(), e.cursor(), e.shell_ghost(), e.at_prompt()));
            log += &format!(
                "#{i} in={chunk:?}\n  out={}\n  events={events:?}\n  at_prompt={} preexec={} cycle={:?} cwd={:?}\n  extract={extract:?}\n  tags={}\n",
                out.escape_ascii(),
                rig.t.at_prompt(),
                rig.t.marker.preexec(),
                rig.t.marker.cycle(),
                rig.t.marker.cwd(),
                tags(&rig.m)
            );
        }
        log
    }

    #[test]
    fn should_filter_and_track_a_bash_cycle_exactly_as_at_the_phase_9_start() {
        let long = "x".repeat(40);
        let chunks = [
            format!("{}{}{}", osc("Shell=bash"), osc("PID=7"), osc("Dir=/work")),
            prompt("$ "),
            "ls -la ".to_string(),
            format!("{long}\x1b[2K"),
            "\r\n".to_string(),
            format!(
                "{}{}hello\x1b]0;title\x07 world\r\n",
                osc(&format!("OSCLock={SID}")),
                osc("PreExec")
            ),
            "\x1b]697;NoHist".to_string(),
            "ory\x07".to_string(),
            precmd(&["ExitCode=3"], "$ "),
            "\x1b[?2004h\x1b[>4;1m\x1b[?9001h\x1b[=1;1u\x1b[u\x1b[>c\x1b[1;5r\x1b[r".to_string(),
        ];
        let mut rig = tracker_rig(30, 8, Shell::Bash, false);

        let log = transcript(&mut rig, &chunks);

        assert_matches_fixture("terminal_session/characterisation/tracker_bash.log", &log);
    }

    #[test]
    fn should_defer_windows_markers_in_order_exactly_as_at_the_phase_9_start() {
        let chunks = [
            format!("{SP}PS> {EP}{}", nc()),
            osc("Dir=/x"),
            "\x1b[?25h".to_string(),
            "ls".to_string(),
            format!("\r\n{}", osc("PreExec")),
            format!("{}{}", osc("NoHistory"), "out\r\n"),
            format!("{}{SP}PS> {EP}{}", osc(&format!("OSCUnlock={SID}")), nc()),
            "\x1b[?25h".to_string(),
            "sleep".to_string(),
            format!("\r\n{}", osc("PreExec")),
            format!("{EP}{}{}", nc(), osc("PreExec")),
            "\x1b[?25h".to_string(),
        ];
        let mut rig = tracker_rig(30, 5, Shell::Pwsh, true);

        let log = transcript(&mut rig, &chunks);

        assert_matches_fixture("terminal_session/characterisation/tracker_defer.log", &log);
    }

    #[test]
    fn should_track_zsh_colours_and_a_nested_shell_exactly_as_at_the_phase_9_start() {
        let chunks = [
            osc("Shell=zsh"),
            osc("ZshAutosuggestionColor=fg=8"),
            prompt("% "),
            "ec".to_string(),
            "\x1b[90mho hi\x1b[0m\x1b[6D".to_string(),
            "\r\n".to_string(),
            format!("{}{}", osc(&format!("OSCLock={SID}")), osc("PreExec")),
            format!("{}{}", osc("PID=99"), prompt("inner> ")),
            "exit\r\n".to_string(),
            format!("{}{}", osc(&format!("OSCUnlock={SID}")), prompt("% ")),
        ];
        let mut rig = tracker_rig(40, 6, Shell::Zsh, false);

        let log = transcript(&mut rig, &chunks);

        assert_matches_fixture("terminal_session/characterisation/tracker_zsh.log", &log);
    }
}
