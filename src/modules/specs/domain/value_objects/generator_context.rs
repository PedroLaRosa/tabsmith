use std::collections::HashMap;

/// `generatorContext` given to custom generators, plus what `exec` needs (SPEC §6.5, §9.3).
#[derive(Clone, Debug, Default)]
pub(crate) struct GeneratorContext {
    cwd: String,
    env: HashMap<String, String>,
    /// Shell name (`currentProcess`); "" when unknown.
    shell: String,
    search_term: String,
    is_dangerous: bool,
}

impl GeneratorContext {
    pub(crate) fn new(
        cwd: String,
        env: HashMap<String, String>,
        shell: String,
        search_term: String,
        is_dangerous: bool,
    ) -> Self {
        GeneratorContext {
            cwd,
            env,
            shell,
            search_term,
            is_dangerous,
        }
    }

    pub(crate) fn cwd(&self) -> &str {
        &self.cwd
    }

    pub(crate) fn env(&self) -> &HashMap<String, String> {
        &self.env
    }

    pub(crate) fn shell(&self) -> &str {
        &self.shell
    }

    pub(crate) fn search_term(&self) -> &str {
        &self.search_term
    }

    pub(crate) fn is_dangerous(&self) -> bool {
        self.is_dangerous
    }
}
