//! The completion engine thread (SPEC §4.3, §6): owns the JS host, spec/parse caches, generator state,
//! recency, history and the inline cache. Serves only the latest request.
//! OWNER: engine agent.
use crate::settings::Settings;
use crate::types::{EngineMsg, Msg};
use std::sync::mpsc::Sender;

/// Spawn the engine thread. Replies go to `tx` as `Msg::Completion` / `Msg::Ghost`.
/// Returns the engine's inbox. The thread exits on `EngineMsg::Shutdown` or when all senders are dropped.
pub fn spawn(tx: Sender<Msg>, settings: Settings) -> Sender<EngineMsg> {
    let _ = (tx, settings);
    todo!("engine agent")
}
