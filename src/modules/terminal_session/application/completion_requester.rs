use crate::modules::completion::application::completion_request::CompletionRequest;
use crate::modules::shared::application::ports::clock::Clock;
use crate::modules::terminal_session::application::ports::completion_engine::CompletionEngine;
use crate::modules::terminal_session::application::ports::terminal_screen::TerminalScreen;
use crate::modules::terminal_session::application::ports::working_directory::WorkingDirectory;
use crate::modules::terminal_session::domain::entities::terminal_session::TerminalSession;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

/// Asks the completion engine for the suggestions of the command line the session shows: reads the line from the
/// screen, lets the session decide whether it needs a new request, and sends the request with the directory and the
/// shell of that moment.
pub(crate) struct CompletionRequester {
    engine: Rc<dyn CompletionEngine>,
    screen: Rc<dyn TerminalScreen>,
    working_directory: Rc<dyn WorkingDirectory>,
    clock: Arc<dyn Clock + Send + Sync>,
}

impl CompletionRequester {
    pub(crate) fn new(
        engine: Rc<dyn CompletionEngine>,
        screen: Rc<dyn TerminalScreen>,
        working_directory: Rc<dyn WorkingDirectory>,
        clock: Arc<dyn Clock + Send + Sync>,
    ) -> Self {
        CompletionRequester { engine, screen, working_directory, clock }
    }

    /// Re-reads the command line and requests completions when it changed (SPEC §4.5; held while insertion-locked).
    /// The request is the last thing done, so a gone engine or a panic in it leaves the session already updated.
    pub(crate) fn refresh(&self, session: &mut TerminalSession) {
        let state = self.screen.state();
        let extracted = if state.alt_screen() {
            None
        } else {
            session
                .tracker()
                .edit_buffer_context()
                .and_then(|context| self.screen.edit_buffer(&context))
        };
        let Some(query) = session.refresh(&state, extracted, self.clock.now_instant()) else {
            return;
        };
        self.engine.complete(CompletionRequest {
            id: query.request(),
            buffer: query.buffer().to_string(),
            cursor: query.cursor(),
            cwd: self.working_directory_of(session),
            shell: session.shell(),
            history_mode: query.history_mode(),
        });
    }

    /// The directory the shell reported, else the one the wrapper runs in, else none (the empty path). The wrapper's
    /// directory is asked only when the shell never reported one: it can vanish while the session runs (P9-T71).
    pub(crate) fn working_directory_of(&self, session: &TerminalSession) -> PathBuf {
        session
            .tracked_cwd()
            .map(|path| path.to_path_buf())
            .or_else(|| self.working_directory.current())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::dropdown::domain::value_objects::key_action::KeyAction;
    use crate::modules::shared::application::ports::clock::FixedClock;
    use crate::modules::shared::domain::value_objects::shell::Shell;
    use crate::modules::terminal_session::application::ports::completion_engine::{CompletionEngineCall, RecordingCompletionEngine};
    use crate::modules::terminal_session::application::shell_output_feeder::ShellOutputFeeder;
    use crate::modules::terminal_session::application::test_fixtures::{TerminalSessionHarness, SID};
    use std::cell::Cell;
    use std::sync::mpsc::{self, Receiver};

    /// The directory of the process that counts how often it was asked.
    struct CountingWorkingDirectory {
        directory: Option<PathBuf>,
        asked: Cell<usize>,
    }

    impl WorkingDirectory for CountingWorkingDirectory {
        fn current(&self) -> Option<PathBuf> {
            self.asked.set(self.asked.get() + 1);
            self.directory.clone()
        }
    }

    /// A session at a bash prompt over the real screen, with the requester under test on a clock that stands still.
    struct Scene {
        harness: TerminalSessionHarness,
        requester: CompletionRequester,
        feeder: ShellOutputFeeder,
        engine_calls: Receiver<CompletionEngineCall>,
        process_directory: Rc<CountingWorkingDirectory>,
    }

    impl Scene {
        fn new(process_directory: Option<&str>) -> Self {
            let clock = Arc::new(FixedClock::new(0));
            let harness = TerminalSessionHarness::with_clock("{}", clock.clone());
            let (calls, engine_calls) = mpsc::channel();
            let process_directory = Rc::new(CountingWorkingDirectory {
                directory: process_directory.map(PathBuf::from),
                asked: Cell::new(0),
            });
            let requester = CompletionRequester::new(
                Rc::new(RecordingCompletionEngine::new(calls)),
                harness.screen.clone(),
                process_directory.clone(),
                clock.clone(),
            );
            let feeder = ShellOutputFeeder::new(harness.screen.clone(), clock);
            Scene { harness, requester, feeder, engine_calls, process_directory }
        }

        /// The shell prints `output`, markers and all; nothing is requested.
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

        fn refresh(&mut self) {
            self.requester.refresh(&mut self.harness.session);
        }

        fn requests(&self) -> Vec<CompletionRequest> {
            self.engine_calls
                .try_iter()
                .filter_map(|call| match call {
                    CompletionEngineCall::Complete(request) => Some(request),
                    _ => None,
                })
                .collect()
        }

        /// The engine's reply for the latest request (twice: the first one after a reset only arms the menu).
        fn engine_replies_with_items(&mut self) {
            use crate::modules::completion::domain::test_fixtures::{comp, item};
            use crate::modules::specs::domain::value_objects::suggestion_kind::SuggestionKind;
            let items = ["cherry-pick", "checkout"]
                .iter()
                .map(|name| item(name, SuggestionKind::Subcommand, "ch"))
                .collect();
            let now = self.harness.clock.now_instant();
            let reply = comp("git ch", 1, items).with_request_id(self.harness.session.request_id());
            self.harness.session.apply_completion(reply.clone(), now);
            self.harness.session.apply_completion(reply, now);
        }
    }

    #[test]
    fn should_send_the_changed_line_with_the_tracked_directory_and_shell() {
        let mut scene = Scene::new(Some("/process"));
        scene.shell_prints("\x1b]697;Dir=/work\x07");
        scene.shell_prints_prompt_and_echo("git ch");

        scene.refresh();

        let requests = scene.requests();
        let sent: Vec<_> = requests
            .iter()
            .map(|r| (r.id, r.buffer.as_str(), r.cursor, r.cwd.as_path(), r.shell, r.history_mode))
            .collect();
        assert_eq!(sent, [(1, "git ch", 6, std::path::Path::new("/work"), Shell::Bash, false)]);
    }

    #[test]
    fn should_fall_back_to_the_process_directory_when_the_shell_reported_none() {
        let mut scene = Scene::new(Some("/process"));
        scene.shell_prints_prompt_and_echo("git ch");

        scene.refresh();

        let directories: Vec<_> = scene.requests().into_iter().map(|r| r.cwd).collect();
        assert_eq!(directories, [PathBuf::from("/process")]);
    }

    #[test]
    fn should_not_ask_the_directory_when_the_shell_reported_one() {
        let mut scene = Scene::new(Some("/process"));
        scene.shell_prints("\x1b]697;Dir=/work\x07");
        scene.shell_prints_prompt_and_echo("git ch");

        scene.refresh();

        assert_eq!((scene.requests().len(), scene.process_directory.asked.get()), (1, 0));
    }

    #[test]
    fn should_send_nothing_for_an_unchanged_line() {
        let mut scene = Scene::new(None);
        scene.shell_prints_prompt_and_echo("git ch");
        scene.refresh();

        scene.refresh();

        assert_eq!(scene.requests().len(), 1);
    }

    #[test]
    fn should_reset_the_menu_for_a_blank_line_without_asking_the_engine() {
        let mut scene = Scene::new(None);
        scene.shell_prints_prompt_and_echo("git ch");
        scene.refresh();
        scene.engine_replies_with_items();
        scene.shell_prints("\x08\x08\x08\x08\x08\x08\x1b[K");

        scene.refresh();

        assert_eq!((scene.harness.session.menu().has_items(), scene.requests().len()), (false, 1));
    }

    #[test]
    fn should_ask_in_history_mode_when_the_menu_is_in_it() {
        let mut scene = Scene::new(None);
        scene.shell_prints_prompt_and_echo("git ch");
        scene.refresh();
        scene.engine_replies_with_items();
        scene.harness.session.run_menu_action(KeyAction::ToggleHistoryMode);
        scene.shell_prints("e");

        scene.refresh();

        let modes: Vec<_> = scene.requests().into_iter().map(|r| r.history_mode).collect();
        assert_eq!(modes, [false, true]);
    }

    #[test]
    fn should_read_no_command_line_on_the_alternate_screen() {
        let mut scene = Scene::new(None);
        scene.harness.screen.pass_through(b"\x1b[?1049h", &mut Vec::new());
        scene.shell_prints_prompt_and_echo("git ch");

        scene.refresh();

        assert!(scene.requests().is_empty());
    }
}
