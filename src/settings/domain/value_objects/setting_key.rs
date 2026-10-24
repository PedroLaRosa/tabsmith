/// Dotted key such as `autocomplete.height`. Not validated: the empty key is a key too (D1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SettingKey(String);

impl SettingKey {
    pub(crate) fn new(key: &str) -> Self {
        SettingKey(key.to_string())
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_keep_the_dotted_key_it_was_built_from() {
        let key = SettingKey::new("autocomplete.height");

        let text = key.as_str();

        assert_eq!(text, "autocomplete.height");
    }

    /// A key is not validated: the empty key is a key like any other (D1).
    #[test]
    fn should_accept_the_empty_key_the_settings_command_stores_today() {
        let key = SettingKey::new("");

        let text = key.as_str();

        assert_eq!(text, "");
    }
}
