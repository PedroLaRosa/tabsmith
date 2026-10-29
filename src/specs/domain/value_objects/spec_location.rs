use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum SpecLocation {
    /// A bundled/user spec by global name, e.g. `git`, `aws/s3`.
    Global { name: String },
    /// `<dir>/.fig/autocomplete/build/<name>.js`.
    Local { dir: PathBuf, name: String },
}
