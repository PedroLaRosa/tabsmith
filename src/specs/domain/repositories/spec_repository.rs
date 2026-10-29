use crate::specs::domain::entities::subcommand::Subcommand;
use crate::specs::domain::spec_error::SpecError;
use crate::specs::domain::value_objects::spec_location::SpecLocation;
use crate::specs::domain::value_objects::spec_lookup::SpecLookup;
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

/// Specs by location (`name` for a Global, `dir|name` for a Local), for tests.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct InMemorySpecRepository {
    specs: HashMap<String, Rc<Subcommand>>,
}

#[cfg(test)]
impl InMemorySpecRepository {
    pub(crate) fn with(specs: &[(&str, &Rc<Subcommand>)]) -> Self {
        InMemorySpecRepository {
            specs: specs
                .iter()
                .map(|(k, v)| (k.to_string(), (*v).clone()))
                .collect(),
        }
    }
}

#[cfg(test)]
impl SpecRepository for InMemorySpecRepository {
    fn find(
        &self,
        location: &SpecLocation,
        _lookup: &SpecLookup<'_>,
    ) -> Result<Option<Rc<Subcommand>>, SpecError> {
        let key = match location {
            SpecLocation::Global { name } => name.clone(),
            SpecLocation::Local { dir, name } => format!("{}|{name}", dir.display()),
        };
        Ok(self.specs.get(&key).cloned())
    }

    fn forget_loaded_specs(&self) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::domain::value_objects::settings::Settings;
    use crate::specs::domain::value_objects::generator_context::GeneratorContext;
    use crate::specs::domain::value_objects::spec_index::SpecIndex;

    fn spec(name: &str) -> Rc<Subcommand> {
        Rc::new(Subcommand::default().with_names(vec![name.into()]))
    }

    fn global(name: &str) -> SpecLocation {
        SpecLocation::Global { name: name.into() }
    }

    #[test]
    fn should_return_the_stored_spec_unchanged() {
        let git = spec("git");
        let repository = InMemorySpecRepository::with(&[("git", &git)]);
        let (settings, index, generator_context) =
            (Settings::default(), SpecIndex::default(), GeneratorContext::default());

        let found = repository
            .find(&global("git"), &SpecLookup::new(&settings, &index, &generator_context))
            .unwrap();

        assert!(found.is_some_and(|found| Rc::ptr_eq(&found, &git)));
    }

    #[test]
    fn should_find_a_local_spec_by_dir_and_name() {
        let (here, elsewhere) = (spec("here"), spec("elsewhere"));
        let repository = InMemorySpecRepository::with(&[("/cwd/|run", &here), ("/other/|run", &elsewhere)]);
        let (settings, index, generator_context) =
            (Settings::default(), SpecIndex::default(), GeneratorContext::default());
        let location = SpecLocation::Local {
            dir: "/cwd/".into(),
            name: "run".into(),
        };

        let found = repository
            .find(&location, &SpecLookup::new(&settings, &index, &generator_context))
            .unwrap();

        assert!(found.is_some_and(|found| Rc::ptr_eq(&found, &here)));
    }

    #[test]
    fn should_answer_none_for_an_unknown_location() {
        let repository = InMemorySpecRepository::with(&[("git", &spec("git"))]);
        let (settings, index, generator_context) =
            (Settings::default(), SpecIndex::default(), GeneratorContext::default());

        let found = repository
            .find(&global("cargo"), &SpecLookup::new(&settings, &index, &generator_context))
            .unwrap();

        assert!(found.is_none());
    }
}
