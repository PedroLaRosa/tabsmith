use crate::settings::domain::value_objects::setting_number::SettingNumber;
use crate::settings::domain::value_objects::setting_value::SettingValue;
use crate::settings::domain::value_objects::settings::Settings;
use serde_json::{Map, Value};

/// The settings.json object as a settings snapshot.
pub(crate) fn settings_from_json(map: Map<String, Value>) -> Settings {
    Settings::new(
        map.into_iter()
            .map(|(key, value)| (key, setting_value_from_json(value)))
            .collect(),
    )
}

/// A serde_json value as a setting value, integers and floats kept apart.
pub(crate) fn setting_value_from_json(value: Value) -> SettingValue {
    match value {
        Value::Null => SettingValue::Null,
        Value::Bool(value) => SettingValue::Bool(value),
        Value::Number(number) => SettingValue::Number(setting_number(&number)),
        Value::String(text) => SettingValue::Text(text),
        Value::Array(items) => {
            SettingValue::List(items.into_iter().map(setting_value_from_json).collect())
        }
        Value::Object(fields) => SettingValue::Object(
            fields
                .into_iter()
                .map(|(key, value)| (key, setting_value_from_json(value)))
                .collect(),
        ),
    }
}

/// serde_json's `PosInt`, `NegInt` and `Float`, read in that order.
fn setting_number(number: &serde_json::Number) -> SettingNumber {
    number
        .as_u64()
        .map(SettingNumber::from)
        .or_else(|| number.as_i64().map(SettingNumber::from))
        .or_else(|| number.as_f64().and_then(SettingNumber::from_f64))
        .expect("a serde_json number is an integer or a finite float")
}

/// A setting value as the serde_json value it was read from: integers stay integers, floats stay floats.
pub(crate) fn setting_value_to_json(value: &SettingValue) -> Value {
    match value {
        SettingValue::Number(number) => json_number(number),
        SettingValue::Null
        | SettingValue::Bool(_)
        | SettingValue::Text(_)
        | SettingValue::List(_)
        | SettingValue::Object(_) => todo!(),
    }
}

/// Back to serde_json's `PosInt`, `NegInt` or `Float`, tried in the order [`setting_number`] read them.
fn json_number(number: &SettingNumber) -> Value {
    number
        .as_u64()
        .map(Value::from)
        .or_else(|| number.as_i64().map(Value::from))
        .or_else(|| number.as_f64().map(Value::from))
        .expect("every setting number reads as an f64")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// `settings autocomplete.height 5` keeps reading back as `5`, not `5.0`.
    #[test]
    fn should_keep_a_json_integer_an_integer() {
        let json = json!(5);

        let value = setting_value_from_json(json);

        assert_eq!(value, SettingValue::Number(SettingNumber::from(5u64)));
    }

    #[test]
    fn should_keep_a_negative_json_integer_a_negative_integer() {
        let json = json!(-3);

        let value = setting_value_from_json(json);

        assert_eq!(value, SettingValue::Number(SettingNumber::from(-3i64)));
    }

    /// `inline.historyCount: 10.0` stays a float, which the integer getter refuses.
    #[test]
    fn should_keep_a_json_float_a_float_even_when_it_is_whole() {
        let json = json!(10.0);

        let value = setting_value_from_json(json);

        assert_eq!(
            value,
            SettingValue::Number(SettingNumber::from_f64(10.0).unwrap())
        );
    }

    #[test]
    fn should_turn_json_null_bools_and_strings_into_the_same_setting_values() {
        let json = [json!(null), json!(true), json!("dark")];

        let values: Vec<_> = json.into_iter().map(setting_value_from_json).collect();

        assert_eq!(
            values,
            [
                SettingValue::Null,
                SettingValue::Bool(true),
                SettingValue::Text("dark".to_string())
            ]
        );
    }

    /// An `inline.profiles.<name>` object with a nested list keeps every member.
    #[test]
    fn should_turn_json_arrays_and_objects_into_lists_and_objects_all_the_way_down() {
        let json = json!({"model": "m", "max_tokens": 64, "tags": ["a", 1.5]});

        let value = setting_value_from_json(json);

        let number = |n: SettingNumber| SettingValue::Number(n);
        let expected = SettingValue::Object(
            [
                ("max_tokens", number(SettingNumber::from(64u64))),
                ("model", SettingValue::Text("m".to_string())),
                (
                    "tags",
                    SettingValue::List(vec![
                        SettingValue::Text("a".to_string()),
                        number(SettingNumber::from_f64(1.5).unwrap()),
                    ]),
                ),
            ]
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect(),
        );
        assert_eq!(value, expected);
    }

    #[test]
    fn should_build_a_settings_snapshot_from_the_settings_file_object() {
        let json = json!({"autocomplete.height": 5, "inline.enabled": false});
        let Value::Object(map) = json else {
            unreachable!()
        };

        let settings = settings_from_json(map);

        assert_eq!(
            (
                settings.f64("autocomplete.height"),
                settings.bool("inline.enabled")
            ),
            (5.0, false)
        );
    }

    /// `settings autocomplete.height 5` writes `5` back to settings.json, not `5.0`.
    #[test]
    fn should_write_an_integer_setting_value_as_a_json_integer() {
        let value = SettingValue::Number(SettingNumber::from(5u64));

        let json = setting_value_to_json(&value);

        assert_eq!(json, json!(5));
    }

    #[test]
    fn should_write_a_negative_setting_number_as_a_negative_json_integer() {
        let value = SettingValue::Number(SettingNumber::from(-3i64));

        let json = setting_value_to_json(&value);

        assert_eq!(json, json!(-3));
    }

    /// `inline.historyCount: 10.0` is saved as `10.0`, so it keeps falling back to the default.
    #[test]
    fn should_write_a_float_setting_number_as_a_json_float_even_when_it_is_whole() {
        let value = SettingValue::Number(SettingNumber::from_f64(10.0).unwrap());

        let json = setting_value_to_json(&value);

        assert_eq!(json, json!(10.0));
    }
}
