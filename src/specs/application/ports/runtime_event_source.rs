use crate::specs::application::ports::runtime_event_poll::RuntimeEventPoll;
use crate::specs::application::ports::runtime_event_sink::RuntimeEventSink;
use std::time::Duration;
#[cfg(test)]
use crate::specs::application::ports::runtime_event::RuntimeEvent;
#[cfg(test)]
use std::cell::Cell;
#[cfg(test)]
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
#[cfg(test)]
use std::sync::Arc;

/// The engine inbox, as the spec runtime sees it while it waits for its tasks.
pub(crate) trait RuntimeEventSource {
    /// Where the runtime's worker threads post their events.
    fn sink(&self) -> RuntimeEventSink;
    /// A newer completion request was received and is kept for the engine. The runtime checks it BEFORE every pump.
    fn has_newer_request(&self) -> bool;
    /// Receives AT MOST ONE message within `timeout`. Any other message is kept, in arrival order, for the engine.
    fn poll(&self, timeout: Duration) -> RuntimeEventPoll;
}

/// An inbox over a channel, for tests: `None` stands for a newer request. It keeps its own sender, so it never
/// disconnects.
#[cfg(test)]
pub(crate) struct ChannelRuntimeEventSource {
    sender: Sender<Option<RuntimeEvent>>,
    receiver: Receiver<Option<RuntimeEvent>>,
    /// Newer requests received by `poll` and not taken yet.
    kept: Cell<usize>,
}

#[cfg(test)]
impl ChannelRuntimeEventSource {
    pub(crate) fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        ChannelRuntimeEventSource {
            sender,
            receiver,
            kept: Cell::new(0),
        }
    }

    pub(crate) fn send_newer_request(&self) {
        let _ = self.sender.send(None);
    }
}

#[cfg(test)]
impl RuntimeEventSource for ChannelRuntimeEventSource {
    fn sink(&self) -> RuntimeEventSink {
        let sender = self.sender.clone();
        Arc::new(move |event: RuntimeEvent| {
            let _ = sender.send(Some(event));
        })
    }

    fn has_newer_request(&self) -> bool {
        self.kept.get() > 0
    }

    fn poll(&self, timeout: Duration) -> RuntimeEventPoll {
        match self.receiver.recv_timeout(timeout) {
            Ok(Some(event)) => RuntimeEventPoll::Event(event),
            Ok(None) => {
                self.kept.set(self.kept.get() + 1);
                RuntimeEventPoll::MessageKept
            }
            Err(RecvTimeoutError::Timeout) => RuntimeEventPoll::TimedOut,
            Err(RecvTimeoutError::Disconnected) => RuntimeEventPoll::Disconnected,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_report_a_newer_request_only_once_received() {
        let events = ChannelRuntimeEventSource::new();
        events.send_newer_request();

        let reported_before_the_poll = events.has_newer_request();
        let poll = events.poll(Duration::from_secs(1));

        assert!(!reported_before_the_poll);
        assert!(matches!(poll, RuntimeEventPoll::MessageKept));
        assert!(events.has_newer_request());
    }
}
