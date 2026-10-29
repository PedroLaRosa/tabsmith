/// What `tabsmith setup` has done so far, reported as it happens so the CLI prints each line at its moment.
#[derive(Debug, PartialEq)]
pub(crate) enum SetupProgress {
    /// The automatic install ran; true if something was installed.
    Installed(bool),
}
