use crate::terminal_session::application::keystroke_handler::KeystrokeHandler;
use crate::terminal_session::domain::entities::terminal_session::TerminalSession;
use std::rc::Rc;

/// A stdin read arrives (SPEC §7.2.4).
pub(crate) struct HandleKeystrokesUseCase {
    handler: Rc<KeystrokeHandler>,
}

impl HandleKeystrokesUseCase {
    pub(crate) fn new(handler: Rc<KeystrokeHandler>) -> Self {
        HandleKeystrokesUseCase { handler }
    }

    pub(crate) fn execute(&self, session: &mut TerminalSession, chunk: &[u8]) {
        self.handler.handle(session, chunk);
    }
}

#[cfg(test)]
mod tests {
    use crate::completion::domain::test_fixtures::{comp, item};
    use crate::completion::domain::value_objects::suggestion::Suggestion;
    use crate::dropdown::domain::value_objects::key_action::KeyAction;
    use crate::dropdown::domain::value_objects::menu_effect::MenuEffect;
    use crate::specs::domain::value_objects::suggestion_kind::SuggestionKind;
    use crate::terminal_session::application::ports::completion_engine::CompletionEngineCall;
    use crate::terminal_session::application::shell_output_feeder::ShellOutputFeeder;
    use crate::terminal_session::application::test_fixtures::{
        set_panic_in_key_handling, shell_line, subs, Io, TerminalSessionHarness, SID,
    };
    use std::time::Instant;

    /// Switches the key-handling fault injection off again, also when the test fails.
    struct KeyHandlingPanic;

    impl KeyHandlingPanic {
        fn on() -> Self {
            set_panic_in_key_handling(true);
            KeyHandlingPanic
        }
    }

    impl Drop for KeyHandlingPanic {
        fn drop(&mut self) {
            set_panic_in_key_handling(false);
        }
    }

    /// A session over the real screen whose shell prints through the feeder, as the output use case will do.
    struct Scene {
        harness: TerminalSessionHarness,
        feeder: ShellOutputFeeder,
        /// How many writes were made before the part of the scenario that is looked at.
        writes_before: usize,
    }

    impl Scene {
        fn new(settings_json: &str) -> Self {
            let harness = TerminalSessionHarness::new(settings_json);
            let feeder = ShellOutputFeeder::new(harness.screen.clone(), harness.clock.clone());
            Scene { harness, feeder, writes_before: 0 }
        }

        /// The shell prints `output`, markers and all.
        fn shell_prints(&mut self, output: &str) {
            let now = self.harness.clock.now_instant();
            let mut body = Vec::new();
            let (splitter, tracker) = self.harness.session.output_parts();
            let events = self.feeder.feed(splitter, tracker, output.as_bytes(), &mut body);
            for event in events {
                self.harness.session.on_shell_event(&event, now);
            }
        }

        /// The prompt `$ ` with the shell integration markers, then `text` echoed, as the shell prints it.
        fn shell_prints_prompt_and_echo(&mut self, text: &str) {
            self.shell_prints(&format!(
                "\x1b]697;StartPrompt\x07$ \x1b]697;EndPrompt\x07\x1b]697;NewCmd={SID}\x07{text}"
            ));
        }

        /// The engine answers the line on screen with `items` and `ghost` (twice: the first reply after a reset only
        /// arms the menu). The request and the reply stay out of the engine calls looked at.
        fn engine_replies(&mut self, items: Vec<Suggestion>, ghost: Option<&str>) {
            self.harness.use_cases.requester().refresh(&mut self.harness.session);
            let (buffer, _) = self.harness.session.sent().expect("a request was sent");
            let reply = comp(buffer, 1, items)
                .with_request_id(self.harness.session.request_id())
                .with_ghost(ghost.map(String::from));
            let now = self.harness.clock.now_instant();
            self.harness.session.apply_completion(reply.clone(), now);
            self.harness.session.apply_completion(reply, now);
            self.engine_calls();
        }

        /// A bash prompt with `text` typed and the engine's reply for it.
        fn at_a_prompt_with_menu(&mut self, text: &str, items: Vec<Suggestion>, ghost: Option<&str>) {
            self.shell_prints_prompt_and_echo(text);
            self.engine_replies(items, ghost);
        }

        /// What was written so far does not count for the assertions.
        fn forget_the_writes_so_far(&mut self) {
            self.writes_before = self.harness.journal.events().len();
        }

        fn writes(&self) -> Vec<Io> {
            self.harness.journal.events().split_off(self.writes_before)
        }

        fn engine_calls(&self) -> Vec<CompletionEngineCall> {
            self.harness.engine_calls.try_iter().collect()
        }

        /// The calls to the engine by name, in order.
        fn engine_call_names(&self) -> Vec<&'static str> {
            self.engine_calls()
                .iter()
                .map(|call| match call {
                    CompletionEngineCall::Complete(_) => "Complete",
                    CompletionEngineCall::ToggleFuzzySearch => "ToggleFuzzySearch",
                    CompletionEngineCall::RecordInsertion { .. } => "RecordInsertion",
                    _ => "other",
                })
                .collect()
        }

        /// What the shell was sent, one entry per write.
        fn shell_writes(&self) -> Vec<Vec<u8>> {
            self.writes()
                .into_iter()
                .filter_map(|event| match event {
                    Io::ShellInput(bytes) => Some(bytes),
                    Io::Stdout(_) => None,
                })
                .collect()
        }

        /// The writes to the terminal and the shell in the order they happened: the bell, the frame, the shell.
        fn order_of_writes(&self) -> Vec<&'static str> {
            self.writes()
                .into_iter()
                .map(|event| match event {
                    Io::Stdout(bytes) if bytes == b"\x07" => "bell",
                    Io::Stdout(_) => "frame",
                    Io::ShellInput(_) => "shell",
                })
                .collect()
        }

        /// The hold on the keys times out, and they are replayed.
        fn held_keys_are_released(&mut self) {
            self.harness.session.expire_hold_at(Instant::now());
            self.harness.use_cases.keystroke_handler().release_held(&mut self.harness.session);
        }
    }

    fn special_items() -> Vec<Suggestion> {
        ["alpha", "beta"].iter().map(|name| item(name, SuggestionKind::Special, "ch")).collect()
    }

    #[test]
    fn should_forward_keys_untouched_when_there_is_no_command_line() {
        let mut scene = Scene::new("{}");

        scene.harness.input(b"ls");

        assert_eq!(
            (scene.writes(), scene.engine_calls().len()),
            (vec![Io::ShellInput(b"ls".to_vec())], 0)
        );
    }

    #[test]
    fn should_write_to_the_shell_only_after_the_frame_was_drawn() {
        let mut scene = Scene::new("{}");
        scene.at_a_prompt_with_menu("git ch", subs("ch"), None);
        scene.harness.redraw();
        scene.forget_the_writes_so_far();

        scene.harness.input(b"\r");

        assert_eq!(scene.order_of_writes(), ["frame", "shell"]);
    }

    #[test]
    fn should_ring_the_bell_before_the_frame_and_the_shell_write() {
        let mut scene = Scene::new("{}");
        scene.at_a_prompt_with_menu("git ch", special_items(), None);

        scene.harness.input(b"\tx");

        assert_eq!(scene.order_of_writes(), ["bell", "frame", "shell"]);
    }

    #[test]
    fn should_ask_the_engine_again_before_redrawing_when_the_history_mode_toggles() {
        let mut scene = Scene::new("{}");
        scene.at_a_prompt_with_menu("git ch", subs("ch"), None);

        scene.harness.input(b"\x12");

        let history_modes: Vec<_> = scene
            .engine_calls()
            .into_iter()
            .filter_map(|call| match call {
                CompletionEngineCall::Complete(request) => Some(request.history_mode),
                _ => None,
            })
            .collect();
        assert_eq!((history_modes, scene.order_of_writes()), (vec![true], vec!["frame"]));
    }

    #[test]
    fn should_toggle_fuzzy_search_in_the_engine_then_ask_again() {
        let mut scene = Scene::new(r#"{"autocomplete.keybindings.control+x": "toggleFuzzySearch"}"#);
        scene.at_a_prompt_with_menu("git ch", subs("ch"), None);

        scene.harness.input(b"\x18");

        assert_eq!(scene.engine_call_names(), ["ToggleFuzzySearch", "Complete"]);
    }

    #[test]
    fn should_record_the_insertion_in_the_engine_after_the_lock_is_set() {
        let mut scene = Scene::new("{}");
        scene.at_a_prompt_with_menu("git ch", subs("ch"), None);

        scene.harness.input(b"\r");

        let recorded: Vec<_> = scene
            .engine_calls()
            .into_iter()
            .filter_map(|call| match call {
                CompletionEngineCall::RecordInsertion { command, name } => Some((command, name)),
                _ => None,
            })
            .collect();
        assert_eq!(
            (scene.harness.session.has_insertion_lock(), recorded),
            (true, vec![("git".to_string(), "cherry-pick".to_string())])
        );
    }

    #[test]
    fn should_record_nothing_when_the_insertion_is_dropped_while_a_command_runs() {
        let mut scene = Scene::new("{}");
        scene.at_a_prompt_with_menu("git ch", subs("ch"), None);
        let Some(MenuEffect::Insert { ins, .. }) =
            scene.harness.session.run_menu_action(KeyAction::InsertSelected).into_iter().next()
        else {
            panic!("no insertion")
        };
        scene.shell_prints(&format!("\x1b]697;OSCLock={SID}\x07\x1b]697;PreExec\x07"));
        let mut forward = Vec::new();

        scene.harness.use_cases.keystroke_handler().insert(
            &mut scene.harness.session,
            &ins,
            Some(("git".to_string(), "cherry-pick".to_string())),
            &mut forward,
        );

        assert_eq!(
            (forward, scene.harness.session.has_insertion_lock(), scene.engine_call_names()),
            (Vec::new(), false, Vec::new())
        );
    }

    #[test]
    fn should_not_decode_a_read_that_arrives_behind_held_keys() {
        let mut scene = Scene::new("{}");
        scene.at_a_prompt_with_menu("git che", subs("che"), None);
        scene.harness.input(b"\x7f");
        scene.harness.input(b"\r");

        let _injected = KeyHandlingPanic::on();
        scene.harness.input(b"x");

        assert_eq!(
            (scene.shell_writes(), scene.harness.session.is_holding_keys()),
            (vec![b"\x7f".to_vec()], true)
        );
    }

    #[test]
    fn should_decode_the_replayed_held_keys_again() {
        let mut scene = Scene::new("{}");
        scene.at_a_prompt_with_menu("git che", subs("che"), None);
        scene.harness.input(b"\x7f");
        scene.harness.input(b"\r");

        scene.held_keys_are_released();

        assert_eq!(scene.shell_writes(), [b"\x7f".to_vec(), b"rry-pick".to_vec()]);
    }

    #[test]
    fn should_keep_a_split_paste_opaque_after_a_hold_reset_the_decoder() {
        let mut scene = Scene::new("{}");
        scene.at_a_prompt_with_menu("git che", subs("che"), None);
        scene.harness.input(b"\x7f");
        scene.harness.input(b"\r\x1b[200~ab");
        scene.harness.input(b"cd\x1b[201~");

        scene.held_keys_are_released();

        assert_eq!(
            scene.shell_writes(),
            [b"\x7f".to_vec(), b"rry-pick\x1b[200~abcd\x1b[201~".to_vec()]
        );
    }

    #[test]
    fn should_hold_a_key_that_would_act_while_the_echo_is_pending() {
        let mut scene = Scene::new("{}");
        scene.at_a_prompt_with_menu("git che", subs("che"), None);

        scene.harness.input(b"\x7f");
        scene.harness.input(b"\r");

        assert_eq!(
            (scene.shell_writes(), scene.harness.session.is_holding_keys()),
            (vec![b"\x7f".to_vec()], true)
        );
    }

    #[test]
    fn should_ghost_accept_a_next_word_with_alt_f_and_the_whole_ghost_with_right() {
        let mut scene = Scene::new("{}");
        scene.at_a_prompt_with_menu("git ch", Vec::new(), Some("eckout feature-login"));
        scene.harness.input(b"\x1bf");
        scene.shell_prints("eckout");
        scene.engine_replies(Vec::new(), Some(" feature-login"));

        scene.harness.input(b"\x1b[C");

        assert_eq!(scene.shell_writes(), [b"eckout".to_vec(), b" feature-login".to_vec()]);
    }

    #[test]
    fn should_set_the_submitted_line_for_execute_only_at_the_prompt() {
        let execute = r#"{"autocomplete.keybindings.control+x": "execute"}"#;
        let mut at_the_prompt = Scene::new(execute);
        at_the_prompt.at_a_prompt_with_menu("git ch", subs("ch"), None);
        let mut off_the_prompt = Scene::new(execute);

        at_the_prompt.harness.input(b"\x18");
        off_the_prompt.harness.input(b"\x18");

        assert_eq!(
            (
                at_the_prompt.harness.session.tracker().submitted().map(String::from),
                off_the_prompt.harness.session.tracker().submitted().map(String::from)
            ),
            (Some("git ch".to_string()), None)
        );
    }

    #[test]
    fn should_write_nothing_to_the_shell_when_every_key_is_swallowed() {
        let mut scene = Scene::new(r#"{"autocomplete.keybindings.enter": "nonsense"}"#);
        scene.at_a_prompt_with_menu("git ch", subs("ch"), None);

        scene.harness.input(b"\r");

        assert!(scene.shell_writes().is_empty());
    }

    #[test]
    fn split_paste_is_never_intercepted() {
        let mut h = TerminalSessionHarness::new("{}");
        h.prompt("git ch");
        h.reply(subs("ch"), None);
        assert!(h.session.menu().intercepting(h.session.settings()));
        h.input(b"\x1b[200~ONE");
        h.input(b"\rTWO\tx\x1b[A\x1b[201~");
        assert_eq!(h.sent(), b"\x1b[200~ONE\rTWO\tx\x1b[A\x1b[201~");
    }

    #[test]
    fn menu_hidden_by_render_guards_does_not_intercept() {
        let mut h = TerminalSessionHarness::new("{}");
        h.prompt("git ch");
        h.reply(subs("ch"), Some("erry-pick"));
        h.input(b"\t");
        assert!(!h.sent().contains(&b'\t'), "drawn: Tab is intercepted");
        // A crashed TUI left a scroll region: nothing is drawn, so Enter and End reach the shell.
        h.output(b"\x1b7\x1b[2;20r\x1b8");
        assert!(h.session.menu().intercepting(h.session.settings()) && !h.drawable());
        h.input(b"\r\x1b[F");
        assert_eq!(h.sent(), b"\r\x1b[F");
    }

    #[test]
    fn disabled_menu_lets_tab_through() {
        let mut h = TerminalSessionHarness::new(r#"{"autocomplete.disable": true, "autocomplete.onlyShowOnTab": true}"#);
        h.prompt("cat ");
        let mut c = comp(
            "cat ",
            1,
            vec![
                item("a.txt", SuggestionKind::File, ""),
                item("b.txt", SuggestionKind::File, ""),
            ],
        );
        c = c.with_request_id(h.session.request_id());
        h.complete(c.clone());
        c = c.with_arg_key(2); // a new argument: hidden until Tab, with items
        h.complete(c);
        assert!(h.session.menu().has_items());
        h.input(b"\t\t");
        assert_eq!(h.sent(), b"\t\t");
    }

    #[test]
    fn insertions_build_on_keys_not_yet_echoed() {
        // Tab + Enter (and Tab + Tab) in one read: the second insertion builds on the first one's result.
        let files = || {
            vec![
                item("abcd1.txt", SuggestionKind::File, "ab"),
                item("abcd2.txt", SuggestionKind::File, "ab"),
            ]
        };
        for (keys, want) in [(&b"\t\r"[..], "cat abcd1.txt"), (b"\t\t", "cat abcd")] {
            let mut h = TerminalSessionHarness::new("{}");
            h.prompt("cat ab");
            h.reply(files(), None);
            h.input(keys);
            assert_eq!(shell_line("cat ab", &h.sent()), want, "{keys:?}");
        }
        // `e` + Enter in one read: the typed-ahead `e` is erased before the selection is inserted.
        let mut h = TerminalSessionHarness::new("{}");
        h.prompt("git ch");
        h.reply(subs("ch"), None);
        h.input(b"e\r");
        assert_eq!(shell_line("git ch", &h.sent()), "git cherry-pick");
        // Same, with the echo arriving in between (consumed, not counted twice).
        let mut h = TerminalSessionHarness::new("{}");
        h.prompt("git ch");
        h.reply(subs("ch"), None);
        h.input(b"e");
        h.output(b"e");
        assert!(h.session.unechoed().is_empty());
        h.input(b"\r");
        assert_eq!(shell_line("git ch", &h.sent()), "git cherry-pick");
        // `l` + Right in one read: only the rest of the ghost is accepted.
        let mut h = TerminalSessionHarness::new("{}");
        h.prompt("echo hel");
        h.reply(vec![], Some("lo"));
        h.input(b"l\x1b[C");
        assert_eq!(shell_line("echo hel", &h.sent()), "echo hello");
    }

    #[test]
    fn unbracketed_paste_is_not_intercepted() {
        // pwsh never enables bracketed paste: a pasted line arrives as keys in one read.
        let mut h = TerminalSessionHarness::new("{}");
        h.prompt("git ");
        h.reply(subs(""), None);
        assert!(h.session.menu().intercepting(h.session.settings()));
        h.input(b"checkout main\r");
        assert_eq!(h.sent(), b"checkout main\r");
    }

    #[test]
    fn enter_records_the_submitted_line() {
        let mut h = TerminalSessionHarness::new("{}");
        h.prompt("echo !");
        h.input(b"!\r");
        assert_eq!(h.session.tracker().submitted(), Some("echo !!"));
        // transient-prompt-execute-insertion-pollutes-history: an insertion that executes (the ↪ row) records the line
        // it builds, not the transient prompt (p10k, oh-my-posh) redrawn without markers in front of it.
        let mut h = TerminalSessionHarness::new("{}");
        h.prompt("git ch");
        h.reply(subs("ch"), None);
        h.session.tracker_mut().set_submitted(Some("stale".into()));
        let fx = h.session.run_menu_action(KeyAction::InsertSelectedAndExecute);
        let Some(MenuEffect::Insert { ins, .. }) = fx.into_iter().next() else {
            panic!("no insertion")
        };
        h.use_cases.keystroke_handler().insert(&mut h.session, &ins, None, &mut Vec::new());
        assert_eq!(h.session.tracker().submitted(), Some("git cherry-pick"));
        let run = format!("\r\x1b[K\u{276f} git cherry-pick\r\n\x1b]697;OSCLock={SID}\x07\x1b]697;PreExec\x07x\r\n");
        h.output(format!("{run}\x1b]697;OSCUnlock={SID}\x07").as_bytes());
        h.prompt("");
        let done: Vec<_> = h
            .engine_calls()
            .into_iter()
            .filter_map(|m| {
                if let CompletionEngineCall::CommandFinished(entry) = m {
                    Some(entry.command().to_string())
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(done, ["git cherry-pick"]);
    }

    #[test]
    fn enter_clears_the_menu_before_the_line_runs() {
        // enter-redraws-stale-menu-below-line: the newline echo must not re-anchor the menu one row below the line.
        let mut h = TerminalSessionHarness::new("{}");
        h.prompt("echo one");
        h.reply(vec![], None);
        assert!(
            h.session.menu().shown(h.session.settings()) && !h.session.menu().intercepting(h.session.settings()),
            "the argument hint row"
        );
        h.input(b"\r");
        assert_eq!(h.sent(), b"\r");
        assert!(!h.session.menu().shown(h.session.settings()));
        h.clear_written();
        h.output(b"\r\n");
        assert!(
            !String::from_utf8_lossy(&h.written()).contains("path"),
            "{:?}",
            String::from_utf8_lossy(&h.written())
        );
    }

    #[test]
    fn pwsh_list_view_owns_the_rows_below() {
        let mut h = TerminalSessionHarness::new("{}");
        h.prompt("git ch");
        h.reply(subs("ch"), None);
        h.session.tracker_mut().set_pwsh_list_view(true);
        assert!(!h.drawable());
        h.clear_written();
        h.redraw();
        assert!(
            !String::from_utf8_lossy(&h.written()).contains("checkout"),
            "menu not drawn"
        );
        h.input(b"\r");
        assert_eq!(h.sent(), b"\r");
        let mut h = TerminalSessionHarness::new("{}");
        h.prompt("");
        h.session.set_redraw_until(None);
        h.session.tracker_mut().set_pwsh_list_view(true);
        assert!(h.drawable(), "an empty line has no list");
    }
}
