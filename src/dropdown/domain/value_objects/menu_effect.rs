use crate::completion::domain::value_objects::insertion::Insertion;

/// What the wrapper must do after an action.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum MenuEffect {
    /// Write an insertion to the PTY (with buffer reconciliation + insertion lock, SPEC §7.3).
    /// `full` = a full insertion (visibility HiddenByInsertion already applied); `record` = (cmd, name) for recency.
    Insert {
        ins: Insertion,
        full: bool,
        record: Option<(String, String)>,
    },
    /// Write raw bytes to the PTY (e.g. "\r" for `execute`).
    Write(Vec<u8>),
    /// Ring the bell (common-prefix failure "shake").
    Bell,
    /// Re-request completions for the current buffer (history mode toggled).
    Rerequest,
    /// Toggle fuzzy search in the engine, then re-request.
    ToggleFuzzy,
}
