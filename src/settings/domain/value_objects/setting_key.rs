/// Dotted key such as `autocomplete.height`.
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
}
