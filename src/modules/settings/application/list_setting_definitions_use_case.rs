use crate::modules::settings::domain::services::settings_catalog;
use crate::modules::settings::domain::value_objects::setting_definition::SettingDefinition;

/// `tabsmith settings list --all`: every documented setting, in the catalog's order.
pub(crate) struct ListSettingDefinitionsUseCase;

impl ListSettingDefinitionsUseCase {
    pub(crate) fn execute(&self) -> &'static [SettingDefinition] {
        settings_catalog::catalog()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `settings list --all` shows the 28 documented settings, `autocomplete.disable` first.
    #[test]
    fn should_list_every_documented_setting_in_catalog_order() {
        let use_case = ListSettingDefinitionsUseCase;

        let definitions = use_case.execute();

        assert_eq!(
            (definitions.len(), definitions[0].key()),
            (28, "autocomplete.disable")
        );
    }
}
