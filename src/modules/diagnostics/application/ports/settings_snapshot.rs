#[cfg(test)]
use crate::modules::diagnostics::application::test_fixtures::EventLog;
use crate::modules::settings::domain::value_objects::settings::Settings;
#[cfg(test)]
use std::cell::Cell;

/// The settings of the user as doctor sees them: never a failure, defaults stand in for a file that cannot be read.
pub(crate) trait SettingsSnapshot {
    fn load(&self) -> Settings;
}

/// Gives the settings it was given, counts the loads and logs each one.
#[cfg(test)]
pub(crate) struct InMemorySettingsSnapshot {
    settings: Settings,
    loads: Cell<usize>,
    log: EventLog,
}

#[cfg(test)]
impl InMemorySettingsSnapshot {
    pub(crate) fn returning(settings: Settings) -> Self {
        InMemorySettingsSnapshot {
            settings,
            loads: Cell::new(0),
            log: EventLog::default(),
        }
    }

    pub(crate) fn logging_to(mut self, log: EventLog) -> Self {
        self.log = log;
        self
    }

    pub(crate) fn loads(&self) -> usize {
        self.loads.get()
    }
}

#[cfg(test)]
impl SettingsSnapshot for InMemorySettingsSnapshot {
    fn load(&self) -> Settings {
        self.log.push("settings.load");
        self.loads.set(self.loads.get() + 1);
        self.settings.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::diagnostics::application::test_fixtures::EventLog;
    use crate::modules::settings::domain::value_objects::setting_value::SettingValue;

    fn strict_settings() -> Settings {
        Settings::default().with("strict", SettingValue::Bool(true))
    }

    #[test]
    fn should_load_the_settings_it_was_given() {
        let snapshot = InMemorySettingsSnapshot::returning(strict_settings());

        let settings = snapshot.load();

        assert_eq!(settings, strict_settings());
    }

    #[test]
    fn should_count_each_load() {
        let snapshot = InMemorySettingsSnapshot::returning(Settings::default());

        snapshot.load();
        snapshot.load();

        assert_eq!(snapshot.loads(), 2);
    }

    #[test]
    fn should_log_each_load() {
        let log = EventLog::default();
        let snapshot =
            InMemorySettingsSnapshot::returning(Settings::default()).logging_to(log.clone());

        snapshot.load();

        assert_eq!(log.entries(), vec!["settings.load".to_string()]);
    }
}
