#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(crate) enum FilterStrategy {
    #[default]
    Default,
    Prefix,
    Fuzzy,
}
