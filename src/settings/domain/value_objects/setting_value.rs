use crate::settings::domain::value_objects::setting_number::SettingNumber;
use std::collections::BTreeMap;

/// A JSON-shaped setting value. `Object` is sorted by key, like serde_json's map without `preserve_order`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum SettingValue {
    Null,
    Bool(bool),
    Number(SettingNumber),
    Text(String),
    List(Vec<SettingValue>),
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

    /// `autocomplete.disableForCommands` keeps the order it was written in.
    #[test]
    fn should_keep_a_list_in_the_order_it_was_given() {
        let items = ["git", "cd"];

        let value = SettingValue::List(
            items
                .into_iter()
                .map(|item| SettingValue::Text(item.to_string()))
                .collect(),
        );

        let SettingValue::List(list) = &value else {
            unreachable!()
        };
        let texts: Vec<_> = list
            .iter()
            .map(|item| match item {
                SettingValue::Text(text) => text.as_str(),
                _ => unreachable!(),
            })
            .collect();
        assert_eq!(texts, items);
    }

    /// `settings autocomplete.height 5` prints `5` back, not `5.0`.
    #[test]
    fn should_tell_an_integer_value_from_the_float_of_the_same_value() {
        let integer = SettingValue::Number(SettingNumber::from(5u64));

        let float = SettingValue::Number(SettingNumber::from_f64(5.0).unwrap());

        assert_ne!(integer, float);
    }
}
