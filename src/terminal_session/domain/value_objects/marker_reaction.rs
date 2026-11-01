use crate::terminal_session::domain::value_objects::marker_effects::MarkerEffects;

/// How the marker tracker took a marker.
#[derive(Debug, PartialEq)]
pub(crate) enum MarkerReaction {
    /// Done: apply the effects.
    Handled(MarkerEffects),
    /// An accepted `PreExec`, half done: apply the effects (the deferred markers it flushed), read the edit buffer
    /// from the screen with `edit_buffer_context()` and finish with `complete_pre_exec`.
    EditBufferNeeded(MarkerEffects),
}
