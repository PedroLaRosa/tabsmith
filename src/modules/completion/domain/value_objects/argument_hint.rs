#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub(crate) struct ArgumentHint {
    name: String,
    description: String,
}

impl ArgumentHint {
    pub(crate) fn new(name: String, description: String) -> Self {
        ArgumentHint { name, description }
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn description(&self) -> &str {
        &self.description
    }
}
