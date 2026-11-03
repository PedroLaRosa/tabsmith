use crate::specs::application::ports::runtime_event::RuntimeEvent;
use std::sync::Arc;

/// Posts a `RuntimeEvent` into the engine inbox from any thread. It ONLY enqueues: a result is never settled
/// inline, because JS registers its resolver after `__ts_exec` returns.
pub(crate) type RuntimeEventSink = Arc<dyn Fn(RuntimeEvent) + Send + Sync>;
