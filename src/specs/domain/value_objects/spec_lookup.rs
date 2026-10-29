use crate::settings::domain::services::settings_catalog::AUTOCOMPLETE_DISABLE_FOR_COMMANDS;
use crate::settings::domain::value_objects::settings::Settings;
use crate::specs::domain::value_objects::generator_context::GeneratorContext;
use crate::specs::domain::value_objects::spec_index::SpecIndex;
use crate::specs::domain::value_objects::spec_location::SpecLocation;

/// What finding a spec reads besides its location: the live settings, the spec index and the parse's generator
/// context (for the CLI version probe and a versioned spec's `default()`). Borrowed per find, like the parse.
pub(crate) struct SpecLookup<'a> {
    settings: &'a Settings,
    index: &'a SpecIndex,
    generator_context: &'a GeneratorContext,
}

impl<'a> SpecLookup<'a> {
    pub(crate) fn new(
        settings: &'a Settings,
        index: &'a SpecIndex,
        generator_context: &'a GeneratorContext,
    ) -> Self {
        SpecLookup {
            settings,
            index,
            generator_context,
        }
    }

    pub(crate) fn settings(&self) -> &'a Settings {
        self.settings
    }

    pub(crate) fn index(&self) -> &'a SpecIndex {
        self.index
    }

    pub(crate) fn generator_context(&self) -> &'a GeneratorContext {
        self.generator_context
    }

    /// `autocomplete.disableForCommands` lists the location's name (the base name for a Local).
    pub(crate) fn disables(&self, location: &SpecLocation) -> bool {
        let (SpecLocation::Global { name } | SpecLocation::Local { name, .. }) = location;
        self.settings
            .str_list(AUTOCOMPLETE_DISABLE_FOR_COMMANDS)
            .contains(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::domain::value_objects::setting_value::SettingValue;

    #[test]
    fn should_disable_only_a_listed_command_name() {
        let disabled = SettingValue::List(vec![
            SettingValue::Text("git".into()),
            SettingValue::Text("make".into()),
        ]);
        let settings = Settings::default().with(AUTOCOMPLETE_DISABLE_FOR_COMMANDS, disabled);
        let (index, generator_context) = (SpecIndex::default(), GeneratorContext::default());
        let lookup = SpecLookup::new(&settings, &index, &generator_context);
        let listed_global = SpecLocation::Global { name: "git".into() };
        let listed_local = SpecLocation::Local {
            dir: "/repo/".into(),
            name: "make".into(),
        };
        let unlisted = SpecLocation::Global {
            name: "cargo".into(),
        };

        let answers = [&listed_global, &listed_local, &unlisted].map(|l| lookup.disables(l));

        assert_eq!(answers, [true, true, false]);
    }
}
