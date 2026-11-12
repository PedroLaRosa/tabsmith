use crate::modules::settings::domain::value_objects::setting_value::SettingValue;

/// What `theme` with no name shows: the saved theme name, or a saved value that is not text, as it is.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum CurrentTheme {
    Named(String),
    NotText(SettingValue),
}
