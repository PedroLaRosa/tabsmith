use crate::modules::completion::domain::test_fixtures::{comp, item};
use crate::modules::completion::domain::value_objects::completion_result::CompletionResult;
use crate::modules::completion::domain::value_objects::suggestion::Suggestion;
use crate::modules::shared::domain::value_objects::data_directory::DataDirectory;
use crate::modules::shared::infrastructure::testing::fixture_log::assert_matches_fixture;
use crate::modules::shared::infrastructure::testing::temp_dir::TempDir;
use crate::modules::specs::domain::value_objects::suggestion_kind::SuggestionKind;
use crate::modules::settings::infrastructure::adapters::json_file_settings_repository::JsonFileSettingsRepository;
use crate::modules::terminal_session::application::test_fixtures::{subs, ScenarioLog, TerminalSessionHarness, SID};
use crate::modules::terminal_session::infrastructure::adapters::lenient_live_settings::LenientLiveSettings;
use std::rc::Rc;
use std::time::{Duration, Instant};

/// The scenarios drive the session only through `Rig`, and force a deadline only through a `poke_*` helper, so a later
/// move of the session edits the Rig and the helpers and leaves the scenario bodies and fixtures alone.
struct Rig {
    h: TerminalSessionHarness,
    log: ScenarioLog,
}

impl Rig {
    fn new(settings_json: &str) -> Rig {
        Rig { h: TerminalSessionHarness::new(settings_json), log: ScenarioLog::new() }
    }

    fn read_settings_from(&mut self, temp_dir: &TempDir) {
        let live_settings = Rc::new(LenientLiveSettings::new(Rc::new(JsonFileSettingsRepository::new(
            DataDirectory::new(Some(temp_dir.path().to_path_buf()), None),
        ))));
        self.h.rewire(|ports| ports.live_settings = live_settings);
    }

    fn note(&mut self, step: &str) {
        self.log.note(step, &self.h.written(), &self.h.sent(), &self.h.engine_calls());
    }

    fn finish(self, fixture: &str) {
        assert_matches_fixture(&format!("terminal_session/characterisation/{fixture}.log"), self.log.text());
    }

    fn prompt(&mut self, text: &str) {
        self.h.prompt(text);
    }

    fn reply(&mut self, items: Vec<Suggestion>, ghost: Option<&str>) {
        self.h.reply(items, ghost);
    }

    fn reply_error(&mut self, buffer: &str, cursor: usize) {
        let request = self.h.session.request_id();
        self.h.complete(CompletionResult::error(request, buffer.into(), cursor));
    }

    fn reply_loading(&mut self, buffer: &str, items: Vec<Suggestion>) {
        let loading = comp(buffer, 1, items)
            .with_request_id(self.h.session.request_id())
            .with_loading(true);
        self.h.complete(loading);
    }

    fn request_id(&self) -> u64 {
        self.h.session.request_id()
    }

    fn input(&mut self, bytes: &[u8]) {
        self.h.input(bytes);
    }

    fn output(&mut self, bytes: &[u8]) {
        self.h.output(bytes);
    }

    fn tick(&mut self) {
        self.h.tick();
    }

    fn resize(&mut self, columns: u16, rows: u16) {
        self.h.resize(columns, rows);
    }

    fn end(&mut self) {
        self.h.end_session();
    }

    fn poke_lock_due(&mut self) {
        self.h.session.expire_insertion_deadline(Instant::now());
    }

    fn poke_resize_due(&mut self) {
        self.h.session.expire_resize_settle_at(Instant::now());
    }

    fn poke_llm_due(&mut self) {
        self.h.session.schedule_llm_request_at(Instant::now());
    }

    fn poke_poll_due(&mut self) {
        self.h.session.expire_poll_at(Instant::now());
    }

    fn poke_spinner_due(&mut self) {
        self.h.session.set_spinner_at(Some(Instant::now()));
    }

    fn llm_reply(&mut self, request: u64, text: Option<String>) {
        self.h.llm_reply(request, text.as_deref());
    }
}

/// The painter holds no frame of its own: the screen remembers what was drawn, so a frame drawn for one use case
/// is skipped as identical by the redraw of another only while both draw over the same screen and writer.
#[test]
fn should_skip_an_identical_frame_across_the_completion_and_llm_use_cases() {
    let mut harness = TerminalSessionHarness::new("{}");
    harness.prompt("git ch");
    harness.reply(subs("ch"), None);
    harness.session.expect_llm_reply(harness.session.request_id());
    let frame_drawn_for_the_reply = !harness.written().is_empty();
    harness.clear_written();

    harness.llm_reply(harness.session.request_id(), None);

    assert_eq!((frame_drawn_for_the_reply, harness.written().is_empty()), (true, true));
}

#[test]
fn should_draw_and_insert_when_typing_with_a_menu_exactly_as_at_the_phase_9_start() {
    let mut rig = Rig::new("{}");
    rig.prompt("git ch");
    rig.note("prompt: git ch");
    rig.reply(subs("ch"), None);
    rig.note("two replies");
    rig.input(b"\x1b[B");
    rig.note("Down");
    rig.input(b"\t");
    rig.note("Tab");
    rig.output(b"erry-pick");
    rig.note("the echo");
    rig.input(b"\x1b");
    rig.note("Esc");
    rig.finish("typing");
}

#[test]
fn should_ring_the_bell_before_the_frame_and_write_the_line_last_exactly_as_at_the_phase_9_start() {
    let mut rig = Rig::new(r#"{"autocomplete.keybindings.tab": "insertCommonPrefix"}"#);
    rig.prompt("git ");
    let items = ["add", "branch"].iter().map(|n| item(n, SuggestionKind::Subcommand, "")).collect();
    rig.reply(items, None);
    rig.note("menu for 'git '");
    rig.input(b"\tx");
    rig.note("Tab (Bell) and a typed x in one read");
    rig.finish("bell");
}

#[test]
fn should_release_held_keys_after_the_frame_exactly_as_at_the_phase_9_start() {
    let mut rig = Rig::new("{}");
    rig.prompt("git che");
    rig.reply(subs("che"), None);
    rig.note("menu");
    rig.input(b"\x7f");
    rig.input(b"\r");
    rig.note("Backspace, Enter (held)");
    rig.output(b"\x08\x1b[K");
    rig.note("echo of the Backspace");
    rig.reply(subs("ch"), None);
    rig.note("reply for 'git ch': release_held after the frame");
    rig.finish("held");
}

#[test]
fn should_report_a_command_cycle_to_the_engine_exactly_as_at_the_phase_9_start() {
    let mut rig = Rig::new("{}");
    rig.prompt("echo hi");
    rig.note("prompt");
    rig.input(b"\r");
    rig.note("Enter");
    rig.output(b"\r\n\x1b]697;PreExec\x07hi\r\n");
    rig.note("PreExec and output");
    rig.output(
        format!("\x1b]697;ExitCode=0\x07\x1b]697;StartPrompt\x07$ \x1b]697;EndPrompt\x07\x1b]697;NewCmd={SID}\x07")
            .as_bytes(),
    );
    rig.note("next prompt");
    rig.finish("lifecycle");
}

#[test]
fn should_clear_the_overlay_and_shut_down_the_engine_at_the_end_exactly_as_at_the_phase_9_start() {
    let mut rig = Rig::new("{}");
    rig.prompt("git ch");
    rig.reply(subs("ch"), None);
    rig.note("menu");
    rig.end();
    rig.note("cleanup");
    rig.finish("cleanup");
}

#[test]
fn should_accept_the_ghost_whole_and_by_word_exactly_as_at_the_phase_9_start() {
    let mut rig = Rig::new("{}");
    rig.prompt("echo hel");
    rig.reply(vec![], Some("lo wor ld"));
    rig.note("ghost lo wor ld");
    rig.input(b"\x1bf");
    rig.note("Alt-f: next word");
    rig.output(b"lo");
    rig.note("echo of the word");
    rig.input(b"\x1b[C");
    rig.note("Right: the rest");
    rig.finish("ghost");
}

#[test]
fn should_keep_an_insertion_in_the_basis_until_it_is_echoed_exactly_as_at_the_phase_9_start() {
    let mut rig = Rig::new("{}");
    rig.prompt("git chec");
    let items = ["checkout", "checkout-index"].iter().map(|n| item(n, SuggestionKind::Subcommand, "chec")).collect();
    rig.reply(items, Some("kout feature-login"));
    rig.note("menu and ghost");
    rig.input(b"\t");
    rig.note("Tab inserts, lock armed");
    rig.poke_lock_due();
    rig.tick();
    rig.note("lock deadline: re-armed, nothing echoed");
    rig.input(b"\x1b[C");
    rig.note("Right: ghost on top of the insertion");
    rig.output(b"kout feature-login");
    rig.note("everything echoed");
    rig.input(b"e");
    rig.input(b"\r");
    rig.note("typed-ahead e, then Enter");
    rig.finish("lock");
}

#[test]
fn should_repaint_once_the_resize_settled_exactly_as_at_the_phase_9_start() {
    let mut rig = Rig::new("{}");
    rig.prompt("git ch");
    rig.reply(subs("ch"), None);
    rig.note("menu");
    rig.resize(60, 18);
    rig.note("resize 60x18");
    rig.output(b"x");
    rig.note("output while resizing re-arms the quiet time");
    rig.poke_resize_due();
    rig.tick();
    rig.note("resize settled: repaint and frame");
    rig.finish("resize");
}

#[test]
fn should_debounce_and_show_the_llm_ghost_text_exactly_as_at_the_phase_9_start() {
    let mut rig = Rig::new(
        r#"{"inline.source": "mock", "inline.profiles.mock": {"provider": "openai", "model": "m", "base_url": "http://127.0.0.1:9/v1"}}"#,
    );
    rig.prompt("zzzcmd foo");
    rig.note("prompt");
    let request = rig.request_id();
    rig.reply_error("zzzcmd foo", 10);
    rig.note("error reply arms the debounce");
    rig.poke_llm_due();
    rig.tick();
    rig.note("debounce due: LLM request");
    rig.llm_reply(request, Some(" bar".into()));
    rig.note("llm reply: ghost drawn");
    rig.llm_reply(request + 5, Some("stale".into()));
    rig.note("stale reply ignored");
    rig.finish("llm");
}

#[test]
fn should_reload_the_settings_and_the_theme_from_the_file_exactly_as_at_the_phase_9_start() {
    let temp_dir = TempDir::new("reload-the-settings");
    let mut rig = Rig::new("{}");
    rig.read_settings_from(&temp_dir);
    rig.prompt("git ch");
    rig.reply(subs("ch"), None);
    rig.note("menu");
    std::fs::write(temp_dir.path().join("settings.json"), r#"{"autocomplete.theme":"light","autocomplete.height":2}"#).unwrap();
    rig.poke_poll_due();
    rig.tick();
    rig.note("settings file appeared: reload, theme, redraw");
    std::fs::remove_file(temp_dir.path().join("settings.json")).unwrap();
    rig.poke_poll_due();
    rig.tick();
    rig.note("settings file deleted: reload to the defaults (the time went Some -> None), redraw in the dark theme");
    rig.finish("reload");
}

#[test]
fn should_serve_the_spinner_after_two_hundred_milliseconds_exactly_as_at_the_phase_9_start() {
    let mut rig = Rig::new("{}");
    rig.prompt("git ch");
    rig.reply_loading("git ch", subs("ch"));
    rig.note("first loading reply arms the spinner timer");
    rig.reply_loading("git ch", subs("ch"));
    std::thread::sleep(Duration::from_millis(250));
    rig.poke_spinner_due();
    rig.tick();
    rig.note("after 250 ms: spinner tick and redraw");
    rig.poke_spinner_due();
    rig.tick();
    rig.note("next tick");
    rig.finish("spinner");
}
