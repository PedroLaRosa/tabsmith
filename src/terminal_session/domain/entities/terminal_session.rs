use crate::completion::domain::value_objects::completion_phase::CompletionPhase;
use crate::completion::domain::value_objects::completion_result::CompletionResult;
use crate::completion::domain::value_objects::insertion::Insertion;
use crate::dropdown::domain::entities::menu::Menu;
use crate::dropdown::domain::value_objects::key::Key;
use crate::dropdown::domain::value_objects::key_action::KeyAction;
use crate::dropdown::domain::value_objects::keymap::Keymap;
use crate::dropdown::domain::value_objects::menu_effect::MenuEffect;
use crate::ghost_text::domain::services::ghost_text_source_resolver;
use crate::ghost_text::domain::value_objects::ghost_text::GhostText;
use crate::ghost_text::domain::value_objects::ghost_text_source::GhostTextSource;
use crate::settings::domain::services::settings_catalog::{
    AUTOCOMPLETE_DISABLE, AUTOCOMPLETE_ONLY_SHOW_ON_TAB, INLINE_DEBOUNCE_MS, INLINE_ENABLED,
};
use crate::settings::domain::value_objects::settings::Settings;
use crate::shared::domain::value_objects::shell::Shell;
use crate::terminal_session::domain::entities::echo_hold::EchoHold;
use crate::terminal_session::domain::entities::ghost_candidates::GhostCandidates;
use crate::terminal_session::domain::entities::insertion_lock::InsertionLock;
use crate::terminal_session::domain::entities::key_decoder::KeyDecoder;
use crate::terminal_session::domain::entities::marker_splitter::MarkerSplitter;
use crate::terminal_session::domain::entities::marker_tracker::MarkerTracker;
use crate::terminal_session::domain::entities::reload_watch::ReloadWatch;
use crate::terminal_session::domain::entities::resize_debounce::ResizeDebounce;
use crate::terminal_session::domain::entities::shell_exit::ShellExit;
use crate::terminal_session::domain::services::insertion_reconciler::{expected_buffer, insertion_bytes};
use crate::terminal_session::domain::services::key_router;
use crate::terminal_session::domain::services::render_guard;
use crate::terminal_session::domain::services::session_deadlines::{self, REDRAW_GRACE, SPINNER_TICK};
use crate::terminal_session::domain::value_objects::completion_query::CompletionQuery;
use crate::terminal_session::domain::value_objects::edit_buffer::EditBuffer;
use crate::terminal_session::domain::value_objects::key_event::KeyEvent;
use crate::terminal_session::domain::value_objects::key_facts::{KeyFacts, KeyFactsParts};
use crate::terminal_session::domain::value_objects::key_route::KeyRoute;
use crate::terminal_session::domain::value_objects::lock_settlement::LockSettlement;
use crate::terminal_session::domain::value_objects::screen_state::ScreenState;
use crate::terminal_session::domain::value_objects::session_id::SessionId;
use crate::terminal_session::domain::value_objects::shell_event::ShellEvent;
use crate::terminal_session::domain::value_objects::terminal_background::TerminalBackground;
use std::path::Path;
use std::time::{Duration, Instant, SystemTime};

/// After a startup probe timeout, a late reply arriving within this window is dropped instead of typed into the shell.
pub(crate) const LATE_PROBE: Duration = Duration::from_secs(3);

/// The wrapper's view of the shell it runs (SPEC §4.3-4.5, §7): the latest completion request, the menu, the ghost
/// texts, what was sent ahead of the shell's echo, the timers and the exit state. It decides; it never reads a clock
/// (every method that needs the time takes it), a file or the screen (`ScreenState` is passed in), and it never calls
/// the engine, the shell or the terminal: the use cases do, with what it returns.
pub(crate) struct TerminalSession {
    id: SessionId,
    /// Shell of the spawned program (the tracker's `Shell=` wins once known).
    launched_shell: Shell,
    settings: Settings,
    keymap: Keymap,
    menu: Menu,
    /// Shell-integration state of the session (SPEC 5.1).
    tracker: MarkerTracker,
    /// Splits the shell's output into bytes, window names and OSC 697 markers; keeps the cut sequences between reads.
    splitter: MarkerSplitter,
    /// Kept across stdin reads: a bracketed paste split over several reads stays one opaque unit (SPEC §7.2.1).
    decoder: KeyDecoder,
    terminal_background: Option<TerminalBackground>,
    /// The startup probe timed out: drop its late reply from stdin until then.
    late_probe_until: Option<Instant>,
    /// Latest completion request id and the buffer/cursor it was sent for.
    request: u64,
    sent: Option<(String, usize)>,
    changed_at: Instant,
    /// Edit buffer at the last batch.
    edit_buffer: Option<EditBuffer>,
    /// Insertion lock (SPEC §7.3), re-armed up to its cap while the shell has echoed nothing of the insertion.
    lock: Option<InsertionLock>,
    /// What was sent ahead of the shell's echo and the keys held back (SPEC §7.3).
    echo: EchoHold,
    /// Ghost candidates (SPEC §8): local (engine) and LLM, with the LLM request id and its debounce deadline.
    ghost: GhostCandidates,
    /// A prompt redraw is in progress (see `REDRAW_GRACE`).
    redraw_until: Option<Instant>,
    /// The quiet spell after a resize and the repaint it owes (SPEC §7.6).
    resize: ResizeDebounce,
    spinner: usize,
    spinner_at: Option<Instant>,
    /// When the settings and the session context files are looked at, and the modification times last seen.
    reload: ReloadWatch,
    /// The exit code, the end of the pty output and the grace the rest of the output gets.
    exit: ShellExit,
    /// After a panic in a handler: plain pass-through until the child exits.
    passthrough: bool,
}

pub(crate) struct TerminalSessionParts {
    pub(crate) id: SessionId,
    pub(crate) launched_shell: Shell,
    pub(crate) settings: Settings,
    pub(crate) tracker: MarkerTracker,
    pub(crate) terminal_background: Option<TerminalBackground>,
    pub(crate) settings_modified: Option<SystemTime>,
    pub(crate) now: Instant,
}

impl TerminalSession {
    pub(crate) fn new(parts: TerminalSessionParts) -> Self {
        TerminalSession {
            id: parts.id,
            launched_shell: parts.launched_shell,
            keymap: Keymap::from_settings(&parts.settings),
            settings: parts.settings,
            menu: Menu::new(),
            tracker: parts.tracker,
            splitter: MarkerSplitter::default(),
            decoder: KeyDecoder::default(),
            terminal_background: parts.terminal_background,
            late_probe_until: None,
            request: 0,
            sent: None,
            changed_at: parts.now,
            edit_buffer: None,
            lock: None,
            echo: EchoHold::default(),
            ghost: GhostCandidates::default(),
            redraw_until: None,
            resize: ResizeDebounce::default(),
            spinner: 0,
            spinner_at: None,
            reload: ReloadWatch::new(parts.now, parts.settings_modified),
            exit: ShellExit::default(),
            passthrough: false,
        }
    }

    pub(crate) fn id(&self) -> &SessionId {
        &self.id
    }

    pub(crate) fn settings(&self) -> &Settings {
        &self.settings
    }

    pub(crate) fn menu(&self) -> &Menu {
        &self.menu
    }

    pub(crate) fn tracker(&self) -> &MarkerTracker {
        &self.tracker
    }

    /// The one place production reaches the tracker mutably: the output path feeds the splitter's pieces to it.
    pub(crate) fn output_parts(&mut self) -> (&mut MarkerSplitter, &mut MarkerTracker) {
        (&mut self.splitter, &mut self.tracker)
    }

    pub(crate) fn terminal_background(&self) -> Option<TerminalBackground> {
        self.terminal_background
    }

    pub(crate) fn is_passthrough(&self) -> bool {
        self.passthrough
    }

    /// The directory the shell reported (`Dir=`), once it did.
    pub(crate) fn tracked_cwd(&self) -> Option<&Path> {
        self.tracker.cwd()
    }

    /// The shell the tracker saw, else the one the wrapper spawned.
    pub(crate) fn shell(&self) -> Shell {
        self.tracker.shell().unwrap_or(self.launched_shell)
    }

    /// Ghost text (SPEC §8.1): survives typing that matches its start. `typed` = after the keys typed ahead of
    /// their echo (what accepting it must add), else after the buffer on screen (what is drawn).
    pub(crate) fn ghost_text(&self, typed: bool, screen: &ScreenState) -> Option<String> {
        if !self.settings.bool(INLINE_ENABLED) || self.tracker.preexec() || screen.alt_screen() {
            return None;
        }
        let e = self.edit_buffer.as_ref()?;
        if e.buffer().is_empty() || e.cursor() != e.buffer().len() || e.shell_ghost() {
            return None;
        }
        let buffer = if typed {
            self.basis()?
        } else {
            e.buffer().to_string()
        };
        self.ghost.rest_after(&buffer)
    }

    /// The wrapper itself keeps the overlay away (SPEC §7.5.1): off the command line, a resize or a prompt redraw in
    /// progress, an open string on the terminal, PSReadLine's list view over the line.
    pub(crate) fn suppressed(&self, screen: &ScreenState) -> bool {
        !self.tracker.at_prompt()
            // The cursor is off the command line (an inline TUI's rows: fzf, fish's pager): a menu for the line
            // stays hidden and keeps its hands off the keys.
            || self.edit_buffer.is_none()
            || self.resize.is_settling()
            || self.resize.repaint_pending()
            || screen.in_string()
            || self.redraw_until.is_some()
            // PSReadLine's ListView predictions own the rows below a non-empty line.
            || (self.tracker.pwsh_list_view() && self.edit_buffer.as_ref().is_some_and(|e| !e.buffer().is_empty()))
    }

    /// No render guard (SPEC §7.5.1) holds, so a menu or ghost can be on screen and keys may act on it.
    /// ponytail: mirrors overlay_renderer.rs `compose`'s guards; its no-room-for-the-menu case (3-row terminal) is not covered.
    pub(crate) fn drawable(&self, screen: &ScreenState) -> bool {
        !self.suppressed(screen) && render_guard::allows_drawing(screen)
    }

    /// The startup probe got no cursor report by `probe_ended_at`: its late reply is dropped for `LATE_PROBE`.
    pub(crate) fn expect_late_probe_reply(&mut self, probe_ended_at: Instant) {
        self.late_probe_until = Some(probe_ended_at + LATE_PROBE);
    }

    pub(crate) fn late_probe_open(&self, now: Instant) -> bool {
        self.late_probe_until.is_some_and(|t| now < t)
    }

    /// A late reply carried the terminal's background.
    pub(crate) fn probe_background_reported(&mut self, background: TerminalBackground) {
        self.terminal_background = Some(background);
    }

    /// A cursor report is the probe's last reply: the window closes (an expired one stays open until then).
    pub(crate) fn probe_cursor_reported(&mut self) {
        self.late_probe_until = None;
    }

    /// When the session loop must wake up next.
    pub(crate) fn next_deadline(&self) -> Option<Instant> {
        session_deadlines::next_deadline(
            self.passthrough,
            self.exit.deadline(),
            [
                self.resize.deadline(),
                self.lock.as_ref().map(InsertionLock::deadline),
                self.ghost.llm_due_at(),
                self.spinner_at,
                Some(self.reload.deadline()),
                self.redraw_until,
                self.echo.deadline(),
            ],
        )
    }

    /// The buffer once the shell echoed everything already sent: the pending insertion's result (else the extracted
    /// buffer) plus the printable keys typed ahead of their echo (SPEC §7.3 reconciliation).
    pub(crate) fn basis(&self) -> Option<String> {
        let buffer = self
            .lock
            .as_ref()
            .map(InsertionLock::expected)
            .or(self.edit_buffer.as_ref().map(EditBuffer::buffer))?;
        Some(format!("{buffer}{}", self.echo.unechoed()))
    }

    /// Re-read the edit buffer; the request to send when it changed (SPEC §4.5; held while insertion-locked).
    /// `extracted` = the command line read from the screen (the caller may skip reading it on the alternate screen;
    /// it is ignored there either way).
    pub(crate) fn refresh(
        &mut self,
        screen: &ScreenState,
        extracted: Option<EditBuffer>,
        now: Instant,
    ) -> Option<CompletionQuery> {
        // Off the command line the cursor is in a widget's inline TUI (fzf, fish's pager; see TerminalScreen::edit_buffer). Live
        // UI only: pre_exec still scrapes after a transient prompt, which is redrawn without markers.
        let new = if screen.alt_screen() {
            None
        } else {
            extracted.filter(EditBuffer::at_prompt)
        };
        let base = self
            .lock
            .as_ref()
            .map(InsertionLock::expected)
            .or(self.edit_buffer.as_ref().map(EditBuffer::buffer));
        self.echo
            .consume_echo(base, new.as_ref().map(EditBuffer::buffer), self.lock.is_some());
        self.edit_buffer = new;
        if let Some(lock) = &mut self.lock {
            let shown = self.edit_buffer.as_ref().map(EditBuffer::buffer);
            match lock.settle(shown, now) {
                LockSettlement::StillHeld => return None,
                LockSettlement::Released => self.lock = None,
            }
        }
        let e = self.edit_buffer.as_ref()?;
        match self.redraw_until {
            Some(t) if e.buffer().trim().is_empty() && now < t => return None,
            _ => self.redraw_until = None,
        }
        if self
            .sent
            .as_ref()
            .is_some_and(|(b, c)| *b == e.buffer() && *c == e.cursor())
        {
            return None;
        }
        let (buffer, cursor) = (e.buffer().to_string(), e.cursor());
        self.request += 1;
        self.sent = Some((buffer.clone(), cursor));
        self.changed_at = now;
        self.ghost.cancel_llm_schedule();
        if buffer.trim().is_empty() {
            self.menu.error_reset(); // empty buffer (SPEC §7.1); nothing to ask
            self.echo.forget_pending(); // nothing stale left for held keys to act on
            return None;
        }
        Some(CompletionQuery::new(self.request, buffer, cursor, self.menu.history_mode()))
    }

    /// A batch of shell output arrived: a resize in progress is still being redrawn.
    pub(crate) fn output_arrived(&mut self, now: Instant) {
        self.resize.output_arrived(now); // the shell / ConPTY is still redrawing
    }

    /// Leaving the alternate screen resets the menu.
    pub(crate) fn screen_changed(&mut self, before: &ScreenState, after: &ScreenState) {
        if before.alt_screen() && !after.alt_screen() {
            self.menu.error_reset();
        }
    }

    /// The tracker's event. True when the settings and the session context files are to be looked at.
    pub(crate) fn on_shell_event(&mut self, event: &ShellEvent, now: Instant) -> bool {
        match event {
            // A prompt redraw (SIGWINCH, `zle reset-prompt`, an empty line) re-emits NewCmd: line, menu and ghost
            // stay, and the unchanged line is not re-requested (its reply would count as a keypress). A cleared line
            // resets through the empty-buffer path in `refresh`, once a line split off into a later read had time.
            ShellEvent::NewCommand(None) => {
                self.redraw_until = Some(now + REDRAW_GRACE);
                return true;
            }
            ShellEvent::NewCommand(Some(_)) | ShellEvent::CommandEnded => {
                self.menu.error_reset();
                self.sent = None;
            }
            ShellEvent::PreExec => self.menu.error_reset(),
        }
        self.echo.reset();
        self.ghost.reset();
        self.lock = None;
        match event {
            ShellEvent::NewCommand(_) | ShellEvent::CommandEnded => true,
            ShellEvent::PreExec => false,
        }
    }

    /// The engine's reply. False when it is not for the latest request (dropped, nothing changed); true when it was
    /// applied and the frame is to be drawn again.
    pub(crate) fn apply_completion(&mut self, c: CompletionResult, now: Instant) -> bool {
        if c.request_id() != self.request {
            return false; // only the latest request is served
        }
        self.echo.answered_by(c.request_id());
        if c.phase() == CompletionPhase::Parse || c.ghost().is_some() {
            self.ghost
                .replace_local(c.ghost().map(String::from).map(|g| GhostText::new(c.buffer().to_string(), g)));
        }
        // LLM ghost: only when the local reply has no history match (SPEC §8.4). Error replies (no spec for the
        // command) too: the inline source depends on the buffer only. A mid-word cursor fails the length check.
        if c.phase() == CompletionPhase::Parse
            && c.ghost().is_none()
            && c.cursor() == c.buffer().len()
            && self.llm_enabled()
        {
            let debounce = self.settings.i64(INLINE_DEBOUNCE_MS).max(0) as u64;
            self.ghost
                .schedule_llm(self.changed_at + Duration::from_millis(debounce));
        }
        if c.is_loading() && self.spinner_at.is_none() {
            self.spinner_at = Some(now + SPINNER_TICK);
        }
        self.menu.on_completion(c, &self.settings, now);
        true
    }

    fn llm_enabled(&self) -> bool {
        self.settings.bool(INLINE_ENABLED)
            && matches!(
                ghost_text_source_resolver::source(&self.settings),
                Ok(GhostTextSource::Profile(_))
            )
    }

    /// The LLM's reply. True when it answers the latest request that went out (the frame is to be drawn again).
    pub(crate) fn accept_llm_reply(&mut self, reply: u64, text: Option<String>) -> bool {
        let sent_buffer = self.sent.as_ref().map(|(buffer, _)| buffer.as_str());
        self.ghost.accept_llm_reply(reply, self.request, text, sent_buffer)
    }

    /// Re-ask for the current buffer: the next `refresh` sends it even though it did not change.
    pub(crate) fn forget_sent(&mut self) {
        self.sent = None;
    }

    /// The menu would take its bound keys now (SPEC §7.2.4).
    pub(crate) fn menu_intercepts(&self) -> bool {
        self.menu.intercepting(&self.settings)
    }

    /// Decodes a stdin read; a bracketed paste split over several reads stays one unit.
    pub(crate) fn decode_keys(&mut self, chunk: &[u8]) -> Vec<KeyEvent> {
        self.decoder.decode(chunk)
    }

    pub(crate) fn is_holding_keys(&self) -> bool {
        self.echo.is_holding()
    }

    /// Keys keep their order behind held ones.
    pub(crate) fn hold_behind(&mut self, chunk: &[u8]) {
        self.echo.hold_behind(chunk);
    }

    /// `keys` = the key that would act and the rest of its read. The held bytes start outside a paste and are decoded
    /// again.
    pub(crate) fn hold_keys(&mut self, keys: &[u8], now: Instant) {
        self.echo.hold(keys, now);
        self.decoder.reset();
    }

    /// Replay held keys once the reply for the buffer after the unmodelled key came (or the hold timed out).
    pub(crate) fn release_held_keys(&mut self, now: Instant) -> Option<Vec<u8>> {
        self.echo.release(now)
    }

    /// What one decoded key of a stdin read does. `typed` = the plain chars already seen in this read.
    pub(crate) fn route_key(&self, key: &Key, typed: usize, screen: &ScreenState) -> KeyRoute {
        // Two or more printable keys in one read are a paste (pwsh has no bracketed paste) or typeahead the menu was
        // not computed for: the rest of the read reaches the shell as is.
        // Off the command line (an inline TUI's rows) keys belong to the TUI, as on the alternate screen.
        let prompt = !self.tracker.preexec() && !screen.alt_screen() && self.edit_buffer.is_some() && typed < 2;
        let facts = KeyFacts::new(KeyFactsParts {
            prompt,
            menu_intercepts: self.menu_intercepts(),
            drawable: self.drawable(screen),
            menu_has_items: self.menu.has_items(),
            autocomplete_disabled: self.settings.bool(AUTOCOMPLETE_DISABLE),
            only_show_on_tab: self.settings.bool(AUTOCOMPLETE_ONLY_SHOW_ON_TAB),
            waiting_for_echo: self.echo.is_waiting_for_echo(),
            ghost_text: self.ghost_text(true, screen),
        });
        key_router::route(key, &self.keymap, &facts)
    }

    pub(crate) fn run_menu_action(&mut self, action: KeyAction) -> Vec<MenuEffect> {
        self.menu.handle(action, &self.settings)
    }

    pub(crate) fn show_menu_on_tab(&mut self) -> Vec<MenuEffect> {
        self.menu.show_on_tab(&self.settings)
    }

    /// A ghost text, or its next word, goes to the shell.
    pub(crate) fn accept_ghost(&mut self, text: &str) {
        self.echo.accepted_ghost(text);
        self.ghost.accepted();
    }

    /// A key goes to the shell as it is. True when it ends the line (the frame is to be drawn again).
    pub(crate) fn forward_key(&mut self, key: &Key) -> bool {
        if key_router::is_enter(key) && self.tracker.at_prompt() {
            self.tracker.set_submitted(self.basis()); // history: the line, not what follows it
        }
        self.note_forwarded(key);
        // Enter too: its newline echo would re-anchor the stale menu below the line until PreExec.
        if key_router::ends_line(key) {
            self.menu.error_reset();
            return true;
        }
        false
    }

    /// Track a forwarded key: a printable char at the end of the line comes back as echo; anything else
    /// (backspace, arrows, control keys, paste) cannot be modelled and drops the prediction.
    fn note_forwarded(&mut self, key: &Key) {
        let at_end = self
            .edit_buffer
            .as_ref()
            .is_some_and(|e| e.cursor() == e.buffer().len());
        match key_router::plain_char(key) {
            Some(c) if at_end => self.echo.typed_ahead(c),
            _ => self.echo.forwarded_unmodelled(self.request),
        }
    }

    /// Insertion with reconciliation + lock (SPEC §7.3): the bytes for the shell. None while a command runs, or
    /// without a buffer.
    pub(crate) fn insert(&mut self, ins: &Insertion, now: Instant) -> Option<Vec<u8>> {
        if self.tracker.preexec() {
            return None;
        }
        let from = self.menu.last().map(|c| (c.buffer().to_string(), c.cursor()));
        let (ib, at) = from.or_else(|| self.edit_buffer.as_ref().map(|e| (e.buffer().to_string(), e.cursor())))?;
        // Against what the shell will show, not the last batch: keys sent ahead of their echo are reconciled too.
        let cur = self.basis().unwrap_or_else(|| ib.clone());
        let bytes = insertion_bytes(ins, &ib, &cur);
        self.echo.forget_unechoed();
        let expected = expected_buffer(ins, &ib, at);
        if ins.executes() {
            // As for Enter: preferred over a scrape with a transient prompt in front of the line; pre_exec still
            // reads a line the shell rewrote from the screen.
            self.tracker.set_submitted(Some(expected.clone()));
        }
        let before = self
            .edit_buffer
            .as_ref()
            .map_or_else(|| ib.clone(), |e| e.buffer().to_string());
        self.lock = Some(InsertionLock::new(now, expected, before));
        Some(bytes)
    }

    /// Raw bytes of a menu action went to the shell.
    pub(crate) fn wrote_to_shell(&mut self, bytes: &[u8]) {
        if bytes == b"\r" && self.tracker.at_prompt() {
            self.tracker.set_submitted(self.basis()); // `execute`
        }
        self.echo.forget_unechoed();
    }

    /// The terminal was resized to `columns` x `lines`; false for a size to ignore (zero, or the one the model has).
    pub(crate) fn resize_wanted(&self, columns: u16, lines: u16, screen: &ScreenState) -> bool {
        self.resize.wants(columns, lines, screen)
    }

    pub(crate) fn resize_applied(&mut self, now: Instant) {
        self.resize.resized(now);
    }

    pub(crate) fn settle_resize_if_due(&mut self, now: Instant) {
        self.resize.settle_if_due(now);
    }

    /// Full repaint once the resize settled (postponed while on the alternate screen).
    pub(crate) fn take_repaint(&mut self, screen: &ScreenState) -> bool {
        self.resize.take_repaint(screen.alt_screen())
    }

    /// The prompt redraw's grace is over: a line that really is empty may be acted on now.
    pub(crate) fn take_due_redraw_grace(&mut self, now: Instant) -> bool {
        if self.redraw_until.is_some_and(|d| d <= now) {
            self.redraw_until = None;
            return true;
        }
        false
    }

    pub(crate) fn insertion_lock_due(&self, now: Instant) -> bool {
        self.lock.as_ref().is_some_and(|l| l.is_due(now))
    }

    /// The LLM debounce ran out: the request id and the buffer to ask the LLM for.
    pub(crate) fn take_due_llm_request(&mut self, now: Instant) -> Option<(u64, String)> {
        if !self.ghost.take_due_llm(now) {
            return None;
        }
        let (buffer, _) = self.sent.clone()?;
        self.ghost.expect_llm_reply(self.request);
        Some((self.request, buffer))
    }

    /// The spinner tick: true when the frame is to be drawn again. `now` = the tick's instant (the next tick is
    /// scheduled from it), `judged_at` = a fresh read, the instant the menu judges "loading for > 200 ms" at.
    pub(crate) fn tick_spinner(&mut self, now: Instant, judged_at: Instant) -> bool {
        if !self.spinner_at.is_some_and(|d| d <= now) {
            return false;
        }
        self.spinner_at = None;
        if self.menu.last().is_some_and(|c| c.is_loading()) {
            self.spinner += 1;
            self.spinner_at = Some(now + SPINNER_TICK);
            return self.menu.loading(judged_at);
        }
        false
    }

    pub(crate) fn spinner_frame(&self) -> usize {
        self.spinner
    }

    /// The files are looked at again: the next look is one interval away.
    pub(crate) fn poll_due(&mut self, now: Instant) -> bool {
        self.reload.poll_due(now)
    }

    /// The context file's time, as the repository reports it. True when the context is to be read again.
    pub(crate) fn context_changed(&mut self, modified: Option<SystemTime>) -> bool {
        self.reload.context_changed(modified)
    }

    /// The settings file's time. True when the settings are to be reloaded.
    pub(crate) fn settings_changed(&mut self, modified: Option<SystemTime>) -> bool {
        self.reload.settings_changed(modified)
    }

    /// The reloaded settings replace the old ones, and the keymap follows them.
    pub(crate) fn apply_settings(&mut self, settings: Settings) {
        self.keymap = Keymap::from_settings(&settings);
        self.settings = settings;
    }

    pub(crate) fn enter_passthrough(&mut self) {
        self.passthrough = true;
    }

    /// The child exited. True while the session must go on (waiting for the rest of its output).
    pub(crate) fn child_exited(&mut self, code: i32, now: Instant) -> bool {
        self.exit.child_exited(code, now)
    }

    /// The pty reached EOF. True while the session must go on (the child has not exited yet).
    pub(crate) fn shell_output_ended(&mut self) -> bool {
        self.exit.output_ended()
    }

    /// The wrapper was told to stop (SIGTERM and SIGHUP alike): it exits with 129.
    pub(crate) fn terminated(&mut self) {
        self.exit.terminated();
    }

    pub(crate) fn exit_grace_over(&self, now: Instant) -> bool {
        self.exit.grace_over(now)
    }

    /// The code the wrapper exits with.
    pub(crate) fn exit_code(&self) -> i32 {
        self.exit.exit_code()
    }
}

/// Test arrangement and inspection: the fields are private, so the pokes and the probes of the session tests go
/// through these.
#[cfg(test)]
impl TerminalSession {
    pub(crate) fn request_id(&self) -> u64 {
        self.request
    }

    /// The buffer and cursor the latest request was sent for.
    pub(crate) fn sent(&self) -> Option<(&str, usize)> {
        self.sent.as_ref().map(|(buffer, cursor)| (buffer.as_str(), *cursor))
    }

    pub(crate) fn edit_buffer(&self) -> Option<&EditBuffer> {
        self.edit_buffer.as_ref()
    }

    pub(crate) fn has_insertion_lock(&self) -> bool {
        self.lock.is_some()
    }

    pub(crate) fn expire_resize_settle_at(&mut self, at: Instant) {
        self.resize.expire_settle_at(at);
    }

    pub(crate) fn schedule_llm_request_at(&mut self, at: Instant) {
        self.ghost.schedule_llm(at);
    }

    pub(crate) fn expire_poll_at(&mut self, at: Instant) {
        self.reload.expire_poll_at(at);
    }

    pub(crate) fn set_spinner_at(&mut self, at: Option<Instant>) {
        self.spinner_at = at;
    }

    pub(crate) fn has_seen_pty_eof(&self) -> bool {
        self.exit.has_seen_eof()
    }

    pub(crate) fn exit_deadline(&self) -> Option<Instant> {
        self.exit.deadline()
    }

    pub(crate) fn tracker_mut(&mut self) -> &mut MarkerTracker {
        &mut self.tracker
    }

    pub(crate) fn unechoed(&self) -> &str {
        self.echo.unechoed()
    }

    pub(crate) fn llm_due_at(&self) -> Option<Instant> {
        self.ghost.llm_due_at()
    }

    pub(crate) fn cancel_llm_schedule(&mut self) {
        self.ghost.cancel_llm_schedule();
    }

    pub(crate) fn expect_llm_reply(&mut self, request: u64) {
        self.ghost.expect_llm_reply(request);
    }

    pub(crate) fn set_redraw_until(&mut self, at: Option<Instant>) {
        self.redraw_until = at;
    }

    pub(crate) fn expire_hold_at(&mut self, at: Instant) {
        self.echo.expire_hold_at(at);
    }

    pub(crate) fn expire_insertion_deadline(&mut self, deadline: Instant) {
        self.lock.as_mut().expect("an insertion lock").expire_deadline_at(deadline);
    }

    pub(crate) fn expire_insertion_lock(&mut self, deadline: Instant, cap: Instant) {
        self.lock.as_mut().expect("an insertion lock").expire_at(deadline, cap);
    }

    pub(crate) fn reported_exit_code(&self) -> Option<i32> {
        self.exit.code()
    }

    pub(crate) fn late_probe_until(&self) -> Option<Instant> {
        self.late_probe_until
    }

    pub(crate) fn context_modified(&self) -> Option<SystemTime> {
        self.reload.context_modified()
    }
}

#[cfg(test)]
mod tests;
