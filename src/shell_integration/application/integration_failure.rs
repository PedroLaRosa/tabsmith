/// One failure of a run over integrations: a shell whose integrations could not be listed, or one integration
/// (its description) whose operation failed. `reason` is the error's text.
#[derive(Debug)]
pub(crate) enum IntegrationFailure {
    Listing { shell: String, reason: String },
    Operation { description: String, reason: String },
}
