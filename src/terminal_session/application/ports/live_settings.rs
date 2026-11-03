use crate::settings::domain::value_objects::settings::Settings;
#[cfg(test)]
use std::cell::{Cell, RefCell};
use std::time::SystemTime;

/// The user's settings as the session sees them while it runs, reloaded when they change.
pub(crate) trait LiveSettings {
    /// When the saved settings last changed; none while there are none, so removing them counts as a change.
    fn last_modified(&self) -> Option<SystemTime>;
    /// Lenient: settings that cannot be loaded give the defaults, never an error.
    fn load(&self) -> Settings;
}

/// Test double: the settings it holds and the time they were saved.
#[cfg(test)]
pub(crate) struct InMemoryLiveSettings {
    settings: RefCell<Settings>,
    saved_at: Cell<Option<SystemTime>>,
}

#[cfg(test)]
impl InMemoryLiveSettings {
    pub(crate) fn new(settings: Settings) -> Self {
        InMemoryLiveSettings { settings: RefCell::new(settings), saved_at: Cell::new(None) }
    }

    pub(crate) fn set(&self, settings: Settings, saved_at: SystemTime) {
        *self.settings.borrow_mut() = settings;
        self.saved_at.set(Some(saved_at));
    }

    pub(crate) fn remove(&self) {
        *self.settings.borrow_mut() = Settings::default();
        self.saved_at.set(None);
    }
}

#[cfg(test)]
impl LiveSettings for InMemoryLiveSettings {
    fn last_modified(&self) -> Option<SystemTime> {
        self.saved_at.get()
    }

    fn load(&self) -> Settings {
        self.settings.borrow().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::domain::value_objects::setting_value::SettingValue;
    use std::time::{Duration, UNIX_EPOCH};

    #[test]
    fn should_load_the_settings_it_holds_and_report_their_time() {
        let saved = Settings::default().with("inline.enabled", SettingValue::Bool(false));
        let saved_at = UNIX_EPOCH + Duration::from_secs(5);
        let live = InMemoryLiveSettings::new(Settings::default());
        live.set(saved.clone(), saved_at);

        let loaded = live.load();

        assert_eq!((loaded, live.last_modified()), (saved, Some(saved_at)));
    }

    #[test]
    fn should_load_the_defaults_and_no_time_once_removed() {
        let live = InMemoryLiveSettings::new(Settings::default());
        live.set(
            Settings::default().with("inline.enabled", SettingValue::Bool(false)),
            UNIX_EPOCH + Duration::from_secs(5),
        );

        live.remove();

        assert_eq!((live.load(), live.last_modified()), (Settings::default(), None));
    }
}
