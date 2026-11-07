use crate::modules::specs::domain::entities::subcommand::Subcommand;
use crate::modules::specs::domain::spec_error::SpecError;
use crate::modules::specs::domain::value_objects::spec_location::SpecLocation;
use crate::modules::specs::domain::value_objects::spec_lookup::SpecLookup;
#[cfg(test)]
use std::cell::{Cell, RefCell};
#[cfg(test)]
use std::collections::HashMap;
use std::rc::Rc;

/// The completion specs, found by where a command's spec lives (SPEC §6.3).
pub(crate) trait SpecRepository {
    /// `Ok(None)`: no spec file, a default export that is not usable, or a name in autocomplete.disableForCommands
    /// (checked on every find, before the cache, never cached). The caller then tries the next location; any `Err`
    /// stops the parse. An unchanged spec keeps its `Rc`.
    fn find(
        &self,
        location: &SpecLocation,
        lookup: &SpecLookup<'_>,
    ) -> Result<Option<Rc<Subcommand>>, SpecError>;
    /// After a settings save, loaded specs are dropped; spec identities by JSON, detected CLI versions and running
    /// probes survive.
    fn forget_loaded_specs(&self);
}

/// Specs by location (`name` for a Global, `dir|name` for a Local), for tests; records every find, never caches,
/// never fails, answers `None` for a disabled command like every repository, and counts its forgets.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct InMemorySpecRepository {
    specs: HashMap<String, Rc<Subcommand>>,
    finds: RefCell<Vec<SpecLocation>>,
    forgets: Cell<usize>,
}

#[cfg(test)]
impl InMemorySpecRepository {
    pub(crate) fn with(specs: &[(&str, &Rc<Subcommand>)]) -> Self {
        InMemorySpecRepository {
            specs: specs
                .iter()
                .map(|(k, v)| (k.to_string(), (*v).clone()))
                .collect(),
            ..Default::default()
        }
    }

    /// Every location asked for, in order, repeats included.
    pub(crate) fn finds(&self) -> Vec<SpecLocation> {
        self.finds.borrow().clone()
    }

    /// How many times the loaded specs were forgotten.
    pub(crate) fn forgets(&self) -> usize {
        self.forgets.get()
    }
}

#[cfg(test)]
impl SpecRepository for InMemorySpecRepository {
    fn find(
        &self,
        location: &SpecLocation,
        lookup: &SpecLookup<'_>,
    ) -> Result<Option<Rc<Subcommand>>, SpecError> {
        self.finds.borrow_mut().push(location.clone());
        if lookup.disables(location) {
            return Ok(None);
        }
        let key = match location {
            SpecLocation::Global { name } => name.clone(),
            SpecLocation::Local { dir, name } => format!("{}|{name}", dir.display()),
        };
        Ok(self.specs.get(&key).cloned())
    }

    fn forget_loaded_specs(&self) {
        self.forgets.set(self.forgets.get() + 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::settings::domain::services::settings_catalog::AUTOCOMPLETE_DISABLE_FOR_COMMANDS;
    use crate::modules::settings::domain::value_objects::setting_value::SettingValue;
    use crate::modules::settings::domain::value_objects::settings::Settings;
    use crate::modules::specs::domain::value_objects::generator_context::GeneratorContext;
    use crate::modules::specs::domain::value_objects::spec_index::SpecIndex;

    fn spec(name: &str) -> Rc<Subcommand> {
        Rc::new(Subcommand::default().with_names(vec![name.into()]))
    }

    fn global(name: &str) -> SpecLocation {
        SpecLocation::Global { name: name.into() }
    }

    fn local(dir: &str, name: &str) -> SpecLocation {
        SpecLocation::Local {
            dir: dir.into(),
            name: name.into(),
        }
    }

    /// One find under `settings`, with an empty spec index and generator context.
    fn find_with(
        repository: &InMemorySpecRepository,
        location: &SpecLocation,
        settings: &Settings,
    ) -> Option<Rc<Subcommand>> {
        let (index, generator_context) = (SpecIndex::default(), GeneratorContext::default());
        repository
            .find(
                location,
                &SpecLookup::new(settings, &index, &generator_context),
            )
            .unwrap()
    }

    #[test]
    fn should_return_the_stored_spec_unchanged() {
        let git = spec("git");
        let repository = InMemorySpecRepository::with(&[("git", &git)]);

        let found = find_with(&repository, &global("git"), &Settings::default());

        assert!(found.is_some_and(|found| Rc::ptr_eq(&found, &git)));
    }

    #[test]
    fn should_find_a_local_spec_by_dir_and_name() {
        let (here, elsewhere) = (spec("here"), spec("elsewhere"));
        let repository =
            InMemorySpecRepository::with(&[("/cwd/|run", &here), ("/other/|run", &elsewhere)]);

        let found = find_with(&repository, &local("/cwd/", "run"), &Settings::default());

        assert!(found.is_some_and(|found| Rc::ptr_eq(&found, &here)));
    }

    #[test]
    fn should_answer_none_for_an_unknown_location() {
        let repository = InMemorySpecRepository::with(&[("git", &spec("git"))]);

        let found = find_with(&repository, &global("cargo"), &Settings::default());

        assert!(found.is_none());
    }

    #[test]
    fn should_record_every_find_in_order() {
        let repository = InMemorySpecRepository::with(&[("git", &spec("git"))]);
        let locations = [
            global("git"),
            local("/cwd/", "run"),
            global("cargo"),
            global("git"),
        ];

        for location in &locations {
            find_with(&repository, location, &Settings::default());
        }

        assert_eq!(repository.finds(), locations);
    }

    #[test]
    fn should_answer_none_for_a_disabled_command() {
        let repository = InMemorySpecRepository::with(&[("git", &spec("git"))]);
        let disabled = SettingValue::List(vec![SettingValue::Text("git".into())]);
        let settings = Settings::default().with(AUTOCOMPLETE_DISABLE_FOR_COMMANDS, disabled);

        let found = find_with(&repository, &global("git"), &settings);

        assert!(found.is_none());
        assert_eq!(repository.finds(), [global("git")]);
    }

    #[test]
    fn should_count_the_times_it_forgot_the_loaded_specs() {
        let repository = InMemorySpecRepository::with(&[("git", &spec("git"))]);

        repository.forget_loaded_specs();
        repository.forget_loaded_specs();

        assert_eq!(repository.forgets(), 2);
    }
}
