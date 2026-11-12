#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(crate) enum Repeatability {
    /// Absent: may be passed once (suggestions); the parser does not enforce it. Explicit `false` → `Times(1)`.
    #[default]
    No,
    Times(u32),
    Unlimited,
}
