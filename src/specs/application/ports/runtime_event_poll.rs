use crate::specs::application::ports::runtime_event::RuntimeEvent;

/// What one poll of a `RuntimeEventSource` received.
pub(crate) enum RuntimeEventPoll {
    Event(RuntimeEvent),
    /// A message for the engine arrived; it is kept, in arrival order, for the engine.
    MessageKept,
    TimedOut,
    Disconnected,
}
