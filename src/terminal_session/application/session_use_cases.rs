use crate::terminal_session::application::apply_completion_result_use_case::ApplyCompletionResultUseCase;
use crate::terminal_session::application::apply_llm_ghost_text_use_case::ApplyLlmGhostTextUseCase;
use crate::terminal_session::application::apply_terminal_background_use_case::ApplyTerminalBackgroundUseCase;
use crate::terminal_session::application::completion_requester::CompletionRequester;
use crate::terminal_session::application::end_session_use_case::EndSessionUseCase;
use crate::terminal_session::application::handle_keystrokes_use_case::HandleKeystrokesUseCase;
use crate::terminal_session::application::handle_shell_exit_use_case::HandleShellExitUseCase;
use crate::terminal_session::application::handle_timer_tick_use_case::HandleTimerTickUseCase;
use crate::terminal_session::application::keystroke_handler::KeystrokeHandler;
use crate::terminal_session::application::overlay_painter::OverlayPainter;
use crate::terminal_session::application::process_shell_output_use_case::{
    ProcessShellOutputParts, ProcessShellOutputUseCase,
};
use crate::terminal_session::application::resize_terminal_use_case::ResizeTerminalUseCase;
use crate::terminal_session::application::session_file_poller::SessionFilePoller;
use crate::terminal_session::application::session_ports_parts::SessionPortsParts;
use crate::terminal_session::application::shell_output_feeder::ShellOutputFeeder;
use std::rc::Rc;

/// Everything a session does, wired once over the ports it runs on. Production and the test harness build it the same
/// way, so a wiring mistake shows in a use-case test.
pub(crate) struct SessionUseCases {
    #[cfg(test)]
    painter: Rc<OverlayPainter>,
    #[cfg(test)]
    requester: Rc<CompletionRequester>,
    #[cfg(test)]
    poller: Rc<SessionFilePoller>,
    #[cfg(test)]
    keystroke_handler: Rc<KeystrokeHandler>,
    pub(crate) handle_keystrokes: HandleKeystrokesUseCase,
    pub(crate) process_shell_output: ProcessShellOutputUseCase,
    pub(crate) apply_completion_result: ApplyCompletionResultUseCase,
    pub(crate) apply_llm_ghost_text: ApplyLlmGhostTextUseCase,
    pub(crate) resize_terminal: ResizeTerminalUseCase,
    pub(crate) handle_shell_exit: HandleShellExitUseCase,
    pub(crate) handle_timer_tick: HandleTimerTickUseCase,
    pub(crate) apply_terminal_background: ApplyTerminalBackgroundUseCase,
    pub(crate) end_session: EndSessionUseCase,
}

impl SessionUseCases {
    pub(crate) fn new(ports: SessionPortsParts) -> Self {
        let painter = Rc::new(OverlayPainter::new(ports.screen.clone(), ports.writer.clone(), ports.clock.clone()));
        let requester = Rc::new(CompletionRequester::new(
            ports.engine.clone(),
            ports.screen.clone(),
            ports.working_directory,
            ports.clock.clone(),
        ));
        let poller = Rc::new(SessionFilePoller::new(
            ports.live_settings,
            ports.session_contexts.clone(),
            ports.engine.clone(),
            ports.screen.clone(),
            ports.themes.clone(),
        ));
        let keystroke_handler = Rc::new(KeystrokeHandler::new(
            ports.engine.clone(),
            ports.screen.clone(),
            ports.shell.clone(),
            ports.writer.clone(),
            ports.clock.clone(),
            requester.clone(),
            painter.clone(),
        ));
        let handle_keystrokes = HandleKeystrokesUseCase::new(keystroke_handler.clone());
        let apply_completion_result =
            ApplyCompletionResultUseCase::new(ports.clock.clone(), painter.clone(), keystroke_handler.clone());
        let apply_llm_ghost_text = ApplyLlmGhostTextUseCase::new(painter.clone());
        let resize_terminal = ResizeTerminalUseCase::new(ports.screen.clone(), ports.shell, ports.clock.clone());
        let handle_shell_exit = HandleShellExitUseCase::new(ports.clock.clone());
        let handle_timer_tick = HandleTimerTickUseCase::new(
            ports.engine.clone(),
            ports.writer.clone(),
            ports.clock.clone(),
            requester.clone(),
            painter.clone(),
            poller.clone(),
            keystroke_handler.clone(),
        );
        let apply_terminal_background = ApplyTerminalBackgroundUseCase::new(ports.screen.clone(), ports.themes);
        let end_session = EndSessionUseCase::new(
            ports.screen.clone(),
            ports.writer.clone(),
            ports.engine.clone(),
            ports.reaper,
            ports.modes,
            ports.session_contexts,
        );
        let process_shell_output = ProcessShellOutputUseCase::new(ProcessShellOutputParts {
            engine: ports.engine,
            feeder: ShellOutputFeeder::new(ports.screen.clone(), ports.clock.clone()),
            screen: ports.screen,
            writer: ports.writer,
            clock: ports.clock,
            requester: requester.clone(),
            painter: painter.clone(),
            poller: poller.clone(),
            handler: keystroke_handler.clone(),
        });
        SessionUseCases {
            #[cfg(test)]
            painter,
            #[cfg(test)]
            requester,
            #[cfg(test)]
            poller,
            #[cfg(test)]
            keystroke_handler,
            handle_keystrokes,
            process_shell_output,
            apply_completion_result,
            apply_llm_ghost_text,
            resize_terminal,
            handle_shell_exit,
            handle_timer_tick,
            apply_terminal_background,
            end_session,
        }
    }

    // interim: dies with 9.6d (the tests of the services and the Rig reach the service through it)
    #[cfg(test)]
    pub(crate) fn painter(&self) -> &OverlayPainter {
        &self.painter
    }

    // interim: dies with 9.6d (the tests of the services reach the service through it)
    #[cfg(test)]
    pub(crate) fn requester(&self) -> &CompletionRequester {
        &self.requester
    }

    // interim: dies with 9.6d (the tests of the services reach the service through it)
    #[cfg(test)]
    pub(crate) fn poller(&self) -> &SessionFilePoller {
        &self.poller
    }

    // interim: dies with 9.6d (the tests of the services reach the service through it)
    #[cfg(test)]
    pub(crate) fn keystroke_handler(&self) -> &KeystrokeHandler {
        &self.keystroke_handler
    }
}

#[cfg(test)]
mod tests {
    use crate::terminal_session::application::test_fixtures::{subs, TerminalSessionHarness};

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
}
