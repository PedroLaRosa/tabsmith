/// Where an integration's code runs in a dotfile: `Pre` at the top, `Post` at the bottom.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IntegrationPhase {
    Pre,
    Post,
}

impl IntegrationPhase {
    pub(crate) fn name(self) -> &'static str {
        match self {
            IntegrationPhase::Pre => "pre",
            IntegrationPhase::Post => "post",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `init <shell> pre|post`, the `--rcfile` script names and the dotfile messages all spell the phase this way.
    #[test]
    fn should_name_each_phase_as_the_init_command_spells_it() {
        let phases = [IntegrationPhase::Pre, IntegrationPhase::Post];

        let names = phases.map(IntegrationPhase::name);

        assert_eq!(names, ["pre", "post"]);
    }
}
