use crate::modules::terminal_session::domain::value_objects::screen_command::ScreenCommand;
use crate::modules::terminal_session::domain::value_objects::shell_event::ShellEvent;

/// What one step of the marker tracker asks of the caller: the screen commands to apply, in order, right away, and
/// the events to hand on after the batch.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct MarkerEffects {
    screen_commands: Vec<ScreenCommand>,
    events: Vec<ShellEvent>,
}

impl MarkerEffects {
    pub(crate) fn screen_commands(&self) -> &[ScreenCommand] {
        &self.screen_commands
    }

    #[cfg(test)]
    pub(crate) fn events(&self) -> &[ShellEvent] {
        &self.events
    }

    pub(crate) fn into_events(self) -> Vec<ShellEvent> {
        self.events
    }

    pub(crate) fn push_screen_command(&mut self, command: ScreenCommand) {
        self.screen_commands.push(command);
    }

    pub(crate) fn push_event(&mut self, event: ShellEvent) {
        self.events.push(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::terminal_session::domain::value_objects::cell_tag::CellTag;

    #[test]
    fn should_keep_the_screen_commands_and_the_events_in_the_order_they_were_pushed() {
        let mut effects = MarkerEffects::default();

        effects.push_screen_command(ScreenCommand::Tag(CellTag::Cycle(3)));
        effects.push_event(ShellEvent::PreExec);
        effects.push_screen_command(ScreenCommand::ClearTag);
        effects.push_event(ShellEvent::CommandEnded);

        assert_eq!(
            (effects.screen_commands(), effects.events()),
            (
                &[
                    ScreenCommand::Tag(CellTag::Cycle(3)),
                    ScreenCommand::ClearTag
                ][..],
                &[ShellEvent::PreExec, ShellEvent::CommandEnded][..]
            )
        );
    }
}
