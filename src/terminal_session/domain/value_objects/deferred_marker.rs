/// A marker whose effect waits for a batch without markers (Windows ConPTY, SPEC §5.1.4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum DeferredMarker {
    EndPrompt,
    NewCmd,
}
