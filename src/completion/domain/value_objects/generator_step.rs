/// Where a generator run is: the task it waits on finishes this step (SPEC §6.6.2-6.6.5).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum GeneratorStep {
    Debounce,
    Script,
    Exec,
    PostProcess,
    Custom,
    Filter,
}
