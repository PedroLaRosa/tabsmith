/// One row of `settings list --all` (SPEC §3.3).
pub(crate) struct SettingDefinition {
    key: &'static str,
    value_type: &'static str,
    default: &'static str,
    description: &'static str,
}

impl SettingDefinition {
    pub(crate) const fn new(
        key: &'static str,
        value_type: &'static str,
        default: &'static str,
        description: &'static str,
    ) -> Self {
        SettingDefinition {
            key,
            value_type,
            default,
            description,
        }
    }

    pub(crate) fn key(&self) -> &'static str {
        self.key
    }

    pub(crate) fn value_type(&self) -> &'static str {
        self.value_type
    }

    /// A JSON literal, or empty when the key has none (templates like `<name>`).
    pub(crate) fn default(&self) -> &'static str {
        self.default
    }

    pub(crate) fn description(&self) -> &'static str {
        self.description
    }
}
