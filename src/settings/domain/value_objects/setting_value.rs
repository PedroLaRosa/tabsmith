use std::collections::BTreeMap;

/// A JSON-shaped setting value. `Object` is sorted by key, like serde_json's map without `preserve_order`.
pub(crate) enum SettingValue {
    Null,
    Bool(bool),
    Object(BTreeMap<String, SettingValue>),
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `settings list` and the saved file print keys sorted: serde_json's map has no insertion order.
    #[test]
    fn should_keep_the_keys_of_an_object_sorted_whatever_order_they_came_in() {
        let entries = [("b", SettingValue::Null), ("a", SettingValue::Bool(true))];

        let value = SettingValue::Object(
            entries
                .into_iter()
                .map(|(key, value)| (key.to_string(), value))
                .collect(),
        );

        let SettingValue::Object(object) = &value else {
            unreachable!()
        };
        assert_eq!(object.keys().collect::<Vec<_>>(), ["a", "b"]);
    }
}
