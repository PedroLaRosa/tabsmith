use crate::bootstrap::engine_message::EngineMessage;
use crate::modules::specs::application::ports::runtime_event::RuntimeEvent;
use crate::modules::specs::application::ports::runtime_event_poll::RuntimeEventPoll;
use crate::modules::specs::application::ports::runtime_event_sink::RuntimeEventSink;
use crate::modules::specs::application::ports::runtime_event_source::RuntimeEventSource;
use crate::modules::specs::infrastructure::adapters::quickjs_spec_runtime::QuickJsSpecRuntime;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::time::Duration;

/// The engine's inbox: one channel for the engine's messages and the spec runtime's events. While the runtime
/// waits it polls the inbox, which keeps every other message, in arrival order, for the engine loop.
pub(crate) struct EngineInbox {
    receiver: Receiver<EngineMessage>,
    sender: Sender<EngineMessage>,
    deferred: RefCell<VecDeque<EngineMessage>>,
}

impl EngineInbox {
    pub(crate) fn new(receiver: Receiver<EngineMessage>, sender: Sender<EngineMessage>) -> Self {
        EngineInbox {
            receiver,
            sender,
            deferred: RefCell::new(VecDeque::new()),
        }
    }

    /// Next message for the engine loop: deferred messages first (incl. a request that aborted a wait), then a
    /// blocking receive. `Runtime` events are handed to the runtime at once. `None` when all senders are gone.
    pub(crate) fn next_msg(&self, runtime: &QuickJsSpecRuntime) -> Option<EngineMessage> {
        loop {
            if let Some(m) = self.deferred.borrow_mut().pop_front() {
                return Some(m);
            }
            let m = self.receiver.recv().ok()?;
            if let Some(m) = Self::deliver_runtime_event(runtime, m) {
                return Some(m);
            }
        }
    }

    /// Like `next_msg` but non-blocking.
    pub(crate) fn try_next_msg(&self, runtime: &QuickJsSpecRuntime) -> Option<EngineMessage> {
        loop {
            if let Some(m) = self.deferred.borrow_mut().pop_front() {
                return Some(m);
            }
            let m = self.receiver.try_recv().ok()?;
            if let Some(m) = Self::deliver_runtime_event(runtime, m) {
                return Some(m);
            }
        }
    }

    /// Hand a `Runtime` event to the runtime; return any other message.
    fn deliver_runtime_event(runtime: &QuickJsSpecRuntime, m: EngineMessage) -> Option<EngineMessage> {
        match m {
            EngineMessage::Runtime(event) => runtime.handle_event(event),
            m => return Some(m),
        }
        None
    }
}

impl RuntimeEventSource for EngineInbox {
    fn sink(&self) -> RuntimeEventSink {
        let sender = self.sender.clone();
        Arc::new(move |event: RuntimeEvent| {
            let _ = sender.send(EngineMessage::Runtime(event));
        })
    }

    /// Looks only at the kept messages, never at the channel: a `Complete` still unread there aborts nothing.
    fn has_newer_request(&self) -> bool {
        self.deferred
            .borrow()
            .iter()
            .any(|m| matches!(m, EngineMessage::Complete(_)))
    }

    fn poll(&self, timeout: Duration) -> RuntimeEventPoll {
        match self.receiver.recv_timeout(timeout) {
            Ok(EngineMessage::Runtime(event)) => RuntimeEventPoll::Event(event),
            Ok(m) => {
                self.deferred.borrow_mut().push_back(m); // the newer-request check at the top of a wait aborts
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
    use crate::modules::completion::application::completion_request::CompletionRequest;
    use crate::modules::settings::domain::value_objects::settings::Settings;
    use crate::modules::shared::domain::value_objects::shell::Shell;
    use crate::modules::specs::application::ports::spec_runtime::SpecRuntime;
    use crate::modules::specs::domain::spec_error::SpecError;
    use crate::modules::specs::domain::value_objects::exec_request::ExecRequest;
    use crate::modules::specs::domain::value_objects::generator_context::GeneratorContext;
    use crate::modules::specs::domain::value_objects::process_output::ProcessOutput;
    use serde_json::{json, Value};
    use std::collections::HashMap;
    use std::rc::Rc;
    use std::sync::mpsc;
    use std::time::Instant;

    /// A runtime over a real engine inbox, and a sender into that inbox.
    fn runtime_with_inbox() -> (QuickJsSpecRuntime, Rc<EngineInbox>, Sender<EngineMessage>) {
        let (tx, rx) = mpsc::channel();
        let inbox = Rc::new(EngineInbox::new(rx, tx.clone()));
        let js = QuickJsSpecRuntime::new(inbox.clone(), &Settings::default()).unwrap();
        (js, inbox, tx)
    }

    #[test]
    fn wait_aborts_and_defers_on_complete() {
        let (js, inbox, tx) = runtime_with_inbox();
        let req = |id| CompletionRequest {
            id,
            buffer: "git ".into(),
            cursor: 4,
            cwd: "/".into(),
            shell: Shell::Zsh,
            history_mode: false,
        };
        let slow = js.start_sleep(10_000);
        tx.send(EngineMessage::ToggleFuzzy).unwrap();
        tx.send(EngineMessage::Complete(req(7))).unwrap();
        tx.send(EngineMessage::Shutdown).unwrap();
        let t0 = Instant::now();
        assert_eq!(
            js.wait_json(&[slow], Instant::now() + Duration::from_secs(5)),
            Err(SpecError::Aborted)
        );
        assert!(t0.elapsed() < Duration::from_secs(1));
        // Still queued → a new wait aborts at once.
        assert_eq!(
            js.wait_json(&[slow], Instant::now() + Duration::from_secs(5)),
            Err(SpecError::Aborted)
        );
        assert!(matches!(inbox.next_msg(&js), Some(EngineMessage::ToggleFuzzy)));
        assert!(matches!(inbox.next_msg(&js), Some(EngineMessage::Complete(r)) if r.id == 7));
        // Non-Complete messages are deferred without aborting; the slow task is still pending.
        let quick = js.start_sleep(10);
        let r = js
            .wait_json(&[quick, slow], Instant::now() + Duration::from_millis(300))
            .unwrap();
        assert_eq!(r, vec![Some(Ok(Value::Null)), None]);
        assert!(matches!(inbox.try_next_msg(&js), Some(EngineMessage::Shutdown)));
        assert!(inbox.try_next_msg(&js).is_none());
        js.forget(&[slow]);
        let unknown = |t| Some(Err(format!("unknown task {t}")));
        assert_eq!(
            js.wait_json(&[slow, quick], Instant::now()).unwrap(),
            vec![unknown(slow), unknown(quick)]
        );
        // Exec tasks deliver `{stdout, stderr, status}`; its Runtime event is consumed internally.
        let e = js.start_exec(
            ExecRequest::new("fig".into(), vec![], None, HashMap::new(), None),
            &GeneratorContext::default(),
        );
        let r = js
            .wait_json(&[e], Instant::now() + Duration::from_secs(5))
            .unwrap();
        assert_eq!(
            r,
            vec![Some(Ok(json!({"stdout": "", "stderr": "", "status": 1})))]
        );
    }

    #[test]
    fn should_deliver_a_runtime_event_read_by_the_engine_loop_to_the_runtime() {
        let (rt, inbox, tx) = runtime_with_inbox();
        let id = rt.start_sleep(10_000);
        let output = Ok(ProcessOutput::new("x".into(), "".into(), 0));
        tx.send(EngineMessage::Runtime(RuntimeEvent { id, output })).unwrap();

        let read = inbox.try_next_msg(&rt);
        let delivered = rt.wait_json(&[id], Instant::now());

        assert!(read.is_none());
        assert_eq!(
            delivered,
            Ok(vec![Some(Ok(json!({"status": 0, "stderr": "", "stdout": "x"})))])
        );
    }
}
