use super::*;
use crate::modules::completion::domain::test_fixtures::comp;
use crate::modules::completion::domain::value_objects::completion_result::CompletionResult;
use crate::modules::completion::domain::value_objects::suggestion::Suggestion;
use crate::modules::shared::infrastructure::testing::fixture_log::assert_matches_fixture;
use crate::modules::terminal_session::application::ports::completion_engine::{CompletionEngineCall, PanickingCompletionEngine};
use crate::modules::terminal_session::application::ports::terminal_writer::CapturingTerminalWriter;
use crate::modules::terminal_session::application::session_ports_parts::SessionPortsParts;
use crate::modules::terminal_session::application::test_fixtures::{
    describe_engine_call, set_panic_in_key_handling, subs, use_cases_over, ScenarioLog, TerminalSessionHarness, SID,
};
use crate::modules::terminal_session::infrastructure::adapters::pty_wrapped_shell::RecordingMasterPty;
use crate::modules::terminal_session::infrastructure::pty::pty_reader::PTY_CREDITS;
use portable_pty::PtySize;
use std::sync::mpsc;
use std::sync::mpsc::Sender;
use std::time::Duration;

// --- The runner driven without a terminal, over the parts of the test harness: PTY output through the use cases,
// keys through `on_input`. ------------------------------------------------------------------------------------------

/// What a session sent out: the bytes it wrote to the shell (read from the writer thread's channel) and to the
/// user's terminal.
struct Taps {
    shell_input: Receiver<Vec<u8>>,
    stdout: Rc<CapturingTerminalWriter>,
}

fn session(json: &str) -> (SessionRunner, Taps) {
    let (s, taps, _, _) = session_with_ports(json);
    (s, taps)
}

/// A runner built from the parts of a harness, its shell a `PtyWrappedShell` whose input channel the test reads.
/// Also returns the ports its use cases are wired over and the calls the engine received.
fn session_with_ports(json: &str) -> (SessionRunner, Taps, SessionPortsParts, Receiver<CompletionEngineCall>) {
    let harness = TerminalSessionHarness::new(json);
    let (input, shell_input) = mpsc::channel();
    let pty = Rc::new(PtyWrappedShell::new(input, Box::new(RecordingMasterPty::new())));
    let mut ports = harness.ports.clone();
    ports.shell = pty.clone();
    let runner = SessionRunner::new(SessionRunnerParts {
        inbox: mpsc::channel().1,
        credits: mpsc::sync_channel(PTY_CREDITS).0,
        pty,
        session: harness.session,
        screen: harness.screen,
        output: harness.writer.clone(),
        use_cases: use_cases_over(ports.clone()),
        clock: harness.clock,
    });
    (runner, Taps { shell_input, stdout: harness.writer }, ports, harness.engine_calls)
}

/// The use cases hold the ports they were built over: a test that swaps a port of the session builds them again.
fn rewire_use_cases(s: &mut SessionRunner, ports: &SessionPortsParts) {
    s.use_cases = use_cases_over(ports.clone());
}

/// A bash prompt `$ ` with the integration markers, then `text` echoed.
fn prompt(s: &mut SessionRunner, text: &str) {
    let p = format!(
        "\x1b]697;StartPrompt\x07$ \x1b]697;EndPrompt\x07\x1b]697;NewCmd={SID}\x07{text}"
    );
    s.use_cases.process_shell_output.execute(&mut s.session, p.as_bytes());
}

/// Engine reply for the latest request (twice: the first one after a reset only arms the menu).
fn reply(s: &mut SessionRunner, items: Vec<Suggestion>, ghost: Option<&str>) {
    let (buffer, _) = s.session.sent().expect("a request was sent");
    let mut c = comp(buffer, 1, items);
    c = c.with_request_id(s.session.request_id()).with_ghost(ghost.map(String::from));
    s.use_cases.apply_completion_result.execute(&mut s.session, c.clone());
    s.use_cases.apply_completion_result.execute(&mut s.session, c);
}

fn sent(pty: &Taps) -> Vec<u8> {
    pty.shell_input.try_iter().flatten().collect()
}

#[test]
fn late_probe_reply_is_not_typed() {
    let (mut s, pty) = session("{}");
    s.session.expect_late_probe_reply(Instant::now());
    prompt(&mut s, "");
    s.on_input(b"\x1b]11;rgb:ffff/ffff/ffff\x1b\\\x1b[5;1Rx");
    assert_eq!(sent(&pty), b"x");
    assert_eq!(
        (s.session.terminal_background().map(TerminalBackground::rgb), s.session.late_probe_until()),
        (Some((255, 255, 255)), None)
    );
}

#[test]
fn panic_turns_the_session_into_a_pass_through() {
    let (mut s, pty) = session("{}");
    prompt(&mut s, "git ch");
    reply(&mut s, subs("ch"), None);
    pty.stdout.clear();
    assert!(!s.guard(|_| panic!("injected")));
    assert!(
        s.session.is_passthrough() && pty.stdout.written().starts_with(b"\x1b[?2026h"),
        "overlay cleared"
    );
    pty.stdout.clear();
    let mut stash = None;
    assert!(s.step(SessionMessage::Pty(b"out\x1b]697;NewCmd\x07".to_vec()), &mut stash));
    assert_eq!(pty.stdout.written(), b"out\x1b]697;NewCmd\x07");
    assert!(s.step(SessionMessage::Stdin(b"\r".to_vec()), &mut stash));
    assert_eq!(sent(&pty), b"\r", "keys reach the shell unintercepted");
    assert!(s.step(SessionMessage::ChildExit(3), &mut stash));
    assert!(!s.step(SessionMessage::PtyEof, &mut stash));
    assert_eq!((s.session.reported_exit_code(), s.next_deadline().is_some()), (Some(3), true));
}

#[test]
fn panic_while_handling_keys_still_forwards_them() {
    let (mut s, pty) = session("{}");
    prompt(&mut s, "ls");
    set_panic_in_key_handling(true);
    assert!(s.step(SessionMessage::Stdin(b"BOOM\r".to_vec()), &mut None));
    set_panic_in_key_handling(false);
    assert!(s.session.is_passthrough());
    assert_eq!(sent(&pty), b"BOOM\r", "the read is not lost");
    // With the menu intercepting it is dropped: an Enter meant for an item never runs the line.
    let (mut s, pty) = session("{}");
    prompt(&mut s, "git ch");
    reply(&mut s, subs("ch"), None);
    set_panic_in_key_handling(true);
    s.step(SessionMessage::Stdin(b"\r".to_vec()), &mut None);
    set_panic_in_key_handling(false);
    assert!(s.session.is_passthrough() && sent(&pty).is_empty());
}

#[cfg(windows)]
#[test]
fn conpty_is_closed_off_the_main_loop() {
    use anyhow::bail;
    use portable_pty::MasterPty;
    use std::io::Write;
    use std::thread;
    struct Master(mpsc::Sender<thread::ThreadId>);
    impl MasterPty for Master {
        fn resize(&self, _: PtySize) -> anyhow::Result<()> {
            Ok(())
        }
        fn get_size(&self) -> anyhow::Result<PtySize> {
            Ok(PtySize::default())
        }
        fn try_clone_reader(&self) -> anyhow::Result<Box<dyn std::io::Read + Send>> {
            bail!("unused")
        }
        fn take_writer(&self) -> anyhow::Result<Box<dyn Write + Send>> {
            bail!("unused")
        }
    }
    // ClosePseudoConsole runs here and may wait for the reader, which waits for credits from the main loop.
    impl Drop for Master {
        fn drop(&mut self) {
            let _ = self.0.send(thread::current().id());
        }
    }
    let (mut s, _) = session("{}");
    let (tx, rx) = mpsc::channel();
    s.pty = Rc::new(PtyWrappedShell::new(mpsc::channel().0, Box::new(Master(tx))));
    assert!(s.step(SessionMessage::ChildExit(0), &mut None));
    let on = rx.recv_timeout(Duration::from_secs(5)).expect("closed");
    assert_ne!(on, thread::current().id());
}

// --- Phase 9 characterisation of the runner: what the session writes to stdout, to the PTY and to the engine when the
// loop hands it messages. A scenario drives the session only through `Rig`, and forces a deadline only through a
// `poke_*` helper. ---------------------------------------------------------------------------------------------------

/// A session plus a log of what each step wrote to stdout, to the PTY and to the engine.
/// `inbox`, `credit_returns` and `stash` are the live ends of the session's message channel, for the scenarios
/// that drive `SessionRunner::step`.
struct Rig {
    s: SessionRunner,
    ports: SessionPortsParts,
    pty: Taps,
    engine: Receiver<CompletionEngineCall>,
    log: ScenarioLog,
    inbox: Sender<SessionMessage>,
    credit_returns: Receiver<()>,
    stash: Option<SessionMessage>,
}

impl Rig {
    fn new(settings_json: &str) -> Rig {
        let (mut s, pty, ports, engine) = session_with_ports(settings_json);
        let (inbox, rx) = mpsc::channel();
        let (credits, credit_returns) = mpsc::sync_channel(PTY_CREDITS);
        s.inbox = rx;
        s.credits = credits;
        Rig { s, ports, pty, engine, log: ScenarioLog::new(), inbox, credit_returns, stash: None }
    }

    fn rewire_use_cases(&mut self) {
        rewire_use_cases(&mut self.s, &self.ports);
    }

    fn sent(&self) -> Vec<u8> {
        sent(&self.pty)
    }

    fn note(&mut self, step: &str) {
        let engine_calls: Vec<CompletionEngineCall> = self.engine.try_iter().collect();
        self.log.note(step, &self.pty.stdout.written(), &self.sent(), &engine_calls);
    }

    fn record(&mut self, line: &str) {
        self.log.record(line);
    }

    fn finish(self, fixture: &str) {
        assert_matches_fixture(&format!("terminal_session/characterisation/{fixture}.log"), self.log.text());
    }

    fn prompt(&mut self, text: &str) {
        prompt(&mut self.s, text);
    }

    fn reply(&mut self, items: Vec<Suggestion>, ghost: Option<&str>) {
        reply(&mut self.s, items, ghost);
    }

    fn reply_error(&mut self, buffer: &str, cursor: usize) {
        let request = self.s.session.request_id();
        let error = CompletionResult::error(request, buffer.into(), cursor);
        self.s.use_cases.apply_completion_result.execute(&mut self.s.session, error);
    }

    fn request_id(&self) -> u64 {
        self.s.session.request_id()
    }

    fn input(&mut self, bytes: &[u8]) {
        self.s.on_input(bytes);
    }

    fn redraw(&mut self) {
        self.s.use_cases.painter().redraw(&self.s.session);
    }

    fn poke_llm_due(&mut self) {
        self.s.session.schedule_llm_request_at(Instant::now());
    }

    fn poke_probe_window(&mut self) {
        self.s.session.expect_late_probe_reply(Instant::now());
    }

    fn step_pty(&mut self, bytes: &[u8]) -> bool {
        self.s.step(SessionMessage::Pty(bytes.to_vec()), &mut self.stash)
    }

    fn step_stdin(&mut self, bytes: &[u8]) -> bool {
        self.s.step(SessionMessage::Stdin(bytes.to_vec()), &mut self.stash)
    }

    fn step_resize(&mut self, columns: u16, rows: u16) -> bool {
        self.s.step(SessionMessage::Resize(columns, rows), &mut self.stash)
    }

    fn step_completion(&mut self, result: CompletionResult) -> bool {
        self.s.step(SessionMessage::Completion(Box::new(result)), &mut self.stash)
    }

    fn step_child_exit(&mut self, code: i32) -> bool {
        self.s.step(SessionMessage::ChildExit(code), &mut self.stash)
    }

    fn step_pty_eof(&mut self) -> bool {
        self.s.step(SessionMessage::PtyEof, &mut self.stash)
    }

    fn step_stashed(&mut self) -> bool {
        let message = self.stash.take().expect("a stashed message");
        self.s.step(message, &mut self.stash)
    }

    fn step_queued(&mut self) -> bool {
        let message = self.s.inbox.try_recv().expect("a queued message");
        self.s.step(message, &mut self.stash)
    }

    fn queue_pty(&mut self, bytes: &[u8]) {
        self.inbox.send(SessionMessage::Pty(bytes.to_vec())).unwrap();
    }

    fn queue_stdin(&mut self, bytes: &[u8]) {
        self.inbox.send(SessionMessage::Stdin(bytes.to_vec())).unwrap();
    }

    fn credits_returned(&self) -> usize {
        self.credit_returns.try_iter().count()
    }

    fn stash_is_stdin(&self) -> bool {
        matches!(self.stash, Some(SessionMessage::Stdin(_)))
    }

    fn panic_in_the_guard(&mut self) {
        assert!(!self.s.guard(|_| panic!("injected")));
    }

    fn is_passthrough(&self) -> bool {
        self.s.session.is_passthrough()
    }

    fn exit_code(&self) -> Option<i32> {
        self.s.session.reported_exit_code()
    }

    fn has_deadline(&self) -> bool {
        self.s.next_deadline().is_some()
    }
}

#[test]
fn should_theme_the_late_probe_reply_and_not_type_it_exactly_as_at_the_phase_9_start() {
    let mut rig = Rig::new(r#"{"autocomplete.theme": "system"}"#);
    rig.poke_probe_window();
    rig.prompt("git ch");
    rig.reply(subs("ch"), None);
    rig.note("menu on the dark filler theme");
    rig.input(b"\x1b]11;rgb:ffff/ffff/ffff\x1b\\\x1b[5;1R");
    rig.note("late reply");
    rig.redraw();
    rig.note("next frame");
    rig.input(b"\x1b[B");
    rig.note("Down: frame in the light theme");
    rig.finish("probe");
}

#[test]
fn should_pass_everything_through_after_a_panic_exactly_as_at_the_phase_9_start() {
    let mut rig = Rig::new("{}");
    rig.prompt("git ch");
    rig.reply(subs("ch"), None);
    rig.note("menu");
    rig.panic_in_the_guard();
    rig.note("guard failed: the clear goes out");
    assert!(rig.step_pty(b"out\x1b]697;NewCmd\x07"));
    rig.note("raw batch in pass-through, markers kept");
    assert!(rig.step_stdin(b"\r"));
    rig.note("raw key");
    assert!(rig.step_resize(60, 20));
    rig.note("resize in pass-through");
    assert!(rig.step_completion(comp("git ch", 1, vec![])));
    rig.note("completion dropped");
    assert!(rig.step_child_exit(3));
    assert!(!rig.step_pty_eof());
    assert!(rig.is_passthrough());
    rig.record(&format!("exit_code={:?} deadline_set={}", rig.exit_code(), rig.has_deadline()));
    rig.finish("passthrough");
}

#[test]
fn should_merge_queued_chunks_into_one_batch_exactly_as_at_the_phase_9_start() {
    let mut rig = Rig::new("{}");
    rig.queue_pty(b"\x1b]697;StartPrompt\x07$ ");
    rig.queue_pty(format!("\x1b]697;EndPrompt\x07\x1b]697;NewCmd={SID}\x07gi").as_bytes());
    rig.queue_stdin(b"t");
    rig.queue_pty(b" ch");
    assert!(rig.step_pty(b"\r\n"));
    rig.note("first step: 3 chunks merged, Stdin stashed");
    assert!(rig.stash_is_stdin());
    let returned = rig.credits_returned();
    rig.record(&format!("credits returned={returned}"));
    assert!(rig.step_stashed());
    rig.note("the stashed Stdin");
    assert!(rig.step_queued());
    rig.note("the last Pty");
    let returned = rig.credits_returned();
    rig.record(&format!("credits returned={returned}"));
    rig.finish("batching");
}
// --- Phase 9 loop guards: how `SessionRunner::step` and `run` batch, hand back credits and decide to leave. ---------
// The tests drive the loop only through `LoopRig`, which builds the driver messages itself, so the move of the
// driver rewrites the rig and leaves the test bodies alone.

#[derive(Debug, PartialEq)]
struct ExitState {
    code: Option<i32>,
    output_ended: bool,
    grace_armed: bool,
}

/// Switches the key-handling fault injection off again, also when the test fails.
struct KeyHandlingPanic;

impl Drop for KeyHandlingPanic {
    fn drop(&mut self) {
        set_panic_in_key_handling(false);
    }
}

/// The session of a loop test with a live inbox, plus what the loop did to the outside world.
struct LoopRig {
    rig: Rig,
    stdout_mark: usize,
    stepped_before: Instant,
    stepped_after: Instant,
    key_handling_panic: Option<KeyHandlingPanic>,
    pty_slave: Option<Box<dyn portable_pty::SlavePty + Send>>,
}

fn loop_session() -> LoopRig {
    loop_session_reading("{}")
}

fn loop_session_with_an_llm() -> LoopRig {
    loop_session_reading(
        r#"{"inline.source": "mock", "inline.profiles.mock": {"provider": "openai", "model": "m", "base_url": "http://127.0.0.1:9/v1"}}"#,
    )
}

fn loop_session_reading(settings_json: &str) -> LoopRig {
    LoopRig {
        rig: Rig::new(settings_json),
        stdout_mark: 0,
        stepped_before: Instant::now(),
        stepped_after: Instant::now(),
        key_handling_panic: None,
        pty_slave: None,
    }
}

impl LoopRig {
    fn queue_pty(&mut self, bytes: &[u8]) {
        self.rig.queue_pty(bytes);
    }

    fn queue_stdin(&mut self, bytes: &[u8]) {
        self.rig.queue_stdin(bytes);
    }

    fn queue_resize(&mut self, columns: u16, rows: u16) {
        self.rig.inbox.send(SessionMessage::Resize(columns, rows)).unwrap();
    }

    fn queue_completion(&mut self, result: CompletionResult) {
        self.rig.inbox.send(SessionMessage::Completion(Box::new(result))).unwrap();
    }

    fn queue_ghost(&mut self, request: u64, text: Option<String>) {
        self.rig.inbox.send(SessionMessage::Ghost { req: request, text }).unwrap();
    }

    fn queue_child_exit(&mut self, code: i32) {
        self.rig.inbox.send(SessionMessage::ChildExit(code)).unwrap();
    }

    fn queue_pty_eof(&mut self) {
        self.rig.inbox.send(SessionMessage::PtyEof).unwrap();
    }

    fn queue_term(&mut self) {
        self.rig.inbox.send(SessionMessage::Term).unwrap();
    }

    fn disconnect(&mut self) {
        self.rig.inbox = mpsc::channel().0;
    }

    /// Takes the next queued message and handles it; false = leave the loop.
    fn step_next(&mut self) -> bool {
        self.stepped_before = Instant::now();
        let keeps_running = self.rig.step_queued();
        self.stepped_after = Instant::now();
        keeps_running
    }

    fn credits_returned(&self) -> usize {
        self.rig.credits_returned()
    }

    fn stash_is_stdin(&self) -> bool {
        self.rig.stash_is_stdin()
    }

    fn request_id(&self) -> u64 {
        self.rig.request_id()
    }

    fn exit_state(&self) -> ExitState {
        ExitState {
            code: self.rig.exit_code(),
            output_ended: self.rig.s.session.has_seen_pty_eof(),
            grace_armed: self.rig.s.session.exit_deadline().is_some(),
        }
    }

    /// The exit deadline lies exactly `delay` after the last step began.
    fn exit_deadline_is_set_after(&self, delay: Duration) -> bool {
        self.rig
            .s
            .session
            .exit_deadline()
            .is_some_and(|at| at >= self.stepped_before + delay && at <= self.stepped_after + delay)
    }

    /// What reached stdout since the last call.
    fn stdout(&mut self) -> Vec<u8> {
        let all = self.rig.pty.stdout.written();
        let written = all[self.stdout_mark..].to_vec();
        self.stdout_mark = all.len();
        written
    }

    fn pty_writes(&self) -> Vec<u8> {
        self.rig.sent()
    }

    fn is_passthrough(&self) -> bool {
        self.rig.is_passthrough()
    }

    /// A menu for `git ch` is on screen.
    fn draw_menu(&mut self) {
        self.rig.prompt("git ch");
        self.rig.reply(subs("ch"), None);
        self.stdout();
    }

    /// Keys typed behind a pending echo wait in the session until the output shows it.
    fn hold_keys_behind_a_pending_echo(&mut self) {
        self.rig.prompt("git che");
        self.rig.reply(subs("che"), None);
        self.rig.input(b"\x7f");
        self.rig.input(b"\r");
        self.stdout();
        self.pty_writes();
    }

    /// A caught panic turns the session into a pass-through; the clear the guard writes is dropped.
    fn pass_through(&mut self) {
        self.rig.panic_in_the_guard();
        self.stdout();
    }

    fn panic_when_the_engine_is_asked(&mut self) {
        self.rig.ports.engine = Rc::new(PanickingCompletionEngine);
        self.rig.rewire_use_cases();
    }

    fn panic_in_key_handling(&mut self) {
        set_panic_in_key_handling(true);
        self.key_handling_panic = Some(KeyHandlingPanic);
    }

    /// A real pseudo-terminal of 80x24 stands in for the child's.
    fn open_a_pty(&mut self) {
        let pair = portable_pty::native_pty_system()
            .openpty(PtySize { rows: 24, cols: 80, pixel_width: 0, pixel_height: 0 })
            .unwrap();
        let (input, shell_input) = mpsc::channel();
        let pty = Rc::new(PtyWrappedShell::new(input, pair.master));
        self.rig.ports.shell = pty.clone();
        self.rig.s.pty = pty;
        self.rig.rewire_use_cases();
        self.rig.pty.shell_input = shell_input;
        self.pty_slave = Some(pair.slave);
    }

    fn pty_size(&self) -> PtySize {
        self.rig.s.pty.size().unwrap()
    }

    fn model_size(&self) -> (usize, usize) {
        self.rig.s.screen.state().size()
    }

    /// A request is waiting for the debounce, which is due.
    fn arm_the_due_llm_debounce(&mut self) {
        self.rig.prompt("zzzcmd foo");
        self.rig.reply_error("zzzcmd foo", 10);
        self.rig.engine.try_iter().for_each(drop);
        self.rig.poke_llm_due();
    }

    /// A prompt is typed and its LLM request went out, so only the reply to the current request is taken.
    fn await_the_llm_reply_to_the_prompt(&mut self) {
        self.rig.prompt("git sta");
        self.rig.s.session.expect_llm_reply(self.rig.request_id());
        self.stdout();
    }

    /// Runs the loop to its end; returns the exit code and the engine calls it made.
    fn run(self) -> (i32, Vec<String>) {
        let rig = self.rig;
        let code = rig.s.run();
        let calls = rig.engine.try_iter().map(|c| describe_engine_call(&c)).collect();
        (code, calls)
    }
}

/// What the renderer writes to take the three-row menu of `draw_menu` off the screen, the cursor ending in `column`.
fn clear_of_the_menu(column: usize) -> Vec<u8> {
    let blank_row = " ".repeat(40);
    format!(
        "\x1b[?2026h\x1b[?25l\x1b[2;3H\x1b[0m{blank_row}\x1b[3;3H\x1b[0m{blank_row}\x1b[4;3H\x1b[0m{blank_row}\x1b[1;{column}H\x1b[0m\x1b[?25h\x1b[?2026l"
    )
    .into_bytes()
}

#[test]
fn should_write_every_queued_pty_chunk_to_stdout_as_one_batch_when_passing_through() {
    let mut rig = loop_session();
    rig.pass_through();
    rig.queue_pty(b"one \x1b]697;NewCmd\x07");
    rig.queue_pty(b"two ");
    rig.queue_pty(b"three");

    let keeps_running = rig.step_next();

    assert!(keeps_running);
    assert_eq!(rig.stdout(), b"one \x1b]697;NewCmd\x07two three");
}

#[test]
fn should_stash_the_first_message_that_is_not_pty_output_behind_the_batch() {
    let mut rig = loop_session();
    rig.pass_through();
    rig.queue_pty(b"one ");
    rig.queue_stdin(b"k");
    rig.queue_pty(b"two");

    rig.step_next();

    assert!(rig.stash_is_stdin());
    assert_eq!(rig.stdout(), b"one ");
}

#[test]
fn should_return_one_credit_per_pty_chunk_after_handling_the_batch() {
    let mut rig = loop_session();
    rig.queue_pty(b"a");
    rig.queue_pty(b"b");
    rig.queue_pty(b"c");

    rig.step_next();

    assert_eq!(rig.credits_returned(), 3);
}

#[test]
fn should_keep_running_at_pty_eof_until_the_child_has_exited() {
    let mut rig = loop_session();
    rig.queue_pty_eof();

    let keeps_running = rig.step_next();

    assert!(keeps_running);
    assert_eq!(rig.exit_state(), ExitState { code: None, output_ended: true, grace_armed: false });
}

#[test]
fn should_leave_the_loop_at_pty_eof_once_the_child_has_exited() {
    let mut rig = loop_session();
    rig.queue_child_exit(7);
    rig.queue_pty_eof();
    rig.step_next();

    let keeps_running = rig.step_next();

    assert!(!keeps_running);
    assert_eq!(rig.exit_state(), ExitState { code: Some(7), output_ended: true, grace_armed: true });
}

#[test]
fn should_leave_the_loop_when_the_child_exits_after_the_output_ended() {
    let mut rig = loop_session();
    rig.queue_pty_eof();
    rig.queue_child_exit(5);
    rig.step_next();

    let keeps_running = rig.step_next();

    assert!(!keeps_running);
    assert_eq!(rig.exit_state(), ExitState { code: Some(5), output_ended: true, grace_armed: false });
}

#[test]
fn should_give_the_output_two_hundred_milliseconds_after_the_child_exits() {
    let mut rig = loop_session();
    rig.queue_child_exit(0);

    let keeps_running = rig.step_next();

    assert!(keeps_running);
    assert!(rig.exit_deadline_is_set_after(Duration::from_millis(200)));
}

#[test]
fn should_exit_with_129_when_terminated() {
    let mut rig = loop_session();
    rig.queue_term();

    let keeps_running = rig.step_next();

    assert!(!keeps_running);
    assert_eq!(rig.exit_state(), ExitState { code: Some(129), output_ended: false, grace_armed: false });
}

#[test]
fn should_ignore_engine_replies_while_passing_through() {
    let mut rig = loop_session();
    rig.draw_menu();
    rig.pass_through();
    let request = rig.request_id();
    rig.queue_completion(comp("git ch", 1, subs("ch")).with_request_id(request));
    rig.queue_ghost(request, Some("eckout".into()));

    rig.step_next();
    rig.step_next();

    assert_eq!((rig.stdout(), rig.pty_writes()), (vec![], vec![]));
}

#[test]
fn should_draw_the_llm_ghost_text_of_the_current_request_through_the_guard() {
    let mut rig = loop_session();
    rig.await_the_llm_reply_to_the_prompt();
    rig.queue_ghost(rig.request_id(), Some("tus".into()));

    rig.step_next();

    assert!(String::from_utf8(rig.stdout()).unwrap().contains("tus"));
}

#[test]
fn should_draw_nothing_for_an_llm_reply_to_another_request_through_the_guard() {
    let mut rig = loop_session();
    rig.await_the_llm_reply_to_the_prompt();
    rig.queue_ghost(rig.request_id() + 1, Some("tus".into()));

    rig.step_next();

    assert_eq!(rig.stdout(), Vec::<u8>::new());
}

#[test]
fn should_resize_only_the_pty_while_passing_through() {
    let mut rig = loop_session();
    rig.open_a_pty();
    rig.pass_through();
    rig.queue_resize(100, 30);

    rig.step_next();

    assert_eq!(rig.pty_size(), PtySize { rows: 30, cols: 100, pixel_width: 0, pixel_height: 0 });
    assert_eq!(rig.model_size(), (80, 24));
}

#[test]
fn should_run_the_timers_after_every_message_outside_passthrough() {
    let mut rig = loop_session_with_an_llm();
    rig.arm_the_due_llm_debounce();
    rig.queue_stdin(b"\x1b[C");
    rig.disconnect();

    let (exit_code, engine_calls) = rig.run();

    assert_eq!(exit_code, 0);
    assert_eq!(engine_calls.len(), 2, "{engine_calls:?}");
    assert!(engine_calls[0].starts_with("RequestLlmGhostText(request="), "{engine_calls:?}");
}

#[test]
fn should_emit_the_cached_clear_then_the_raw_batch_and_return_the_credits_when_the_refresh_panics() {
    let mut rig = loop_session();
    rig.draw_menu();
    rig.panic_when_the_engine_is_asked();
    rig.queue_pty(b"e");
    rig.queue_pty(b"c\x1b]697;NewCmd\x07");

    rig.step_next();

    assert!(rig.is_passthrough());
    assert_eq!(rig.stdout(), [clear_of_the_menu(9), b"ec\x1b]697;NewCmd\x07".to_vec()].concat());
    assert_eq!(rig.credits_returned(), 2);
}

#[test]
fn should_write_the_batch_again_raw_when_release_held_panics_after_the_frame() {
    let mut rig = loop_session();
    rig.hold_keys_behind_a_pending_echo();
    rig.panic_in_key_handling();
    rig.queue_pty(b"\r\n\x1b]697;PreExec\x07");

    rig.step_next();

    assert!(rig.is_passthrough());
    let processed = [clear_of_the_menu(10), b"\r\n".to_vec()].concat();
    let raw_again = b"\r\n\x1b]697;PreExec\x07".to_vec();
    assert_eq!(rig.stdout(), [processed, raw_again].concat());
}
