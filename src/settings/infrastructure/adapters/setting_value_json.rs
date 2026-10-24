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

/// A settings snapshot as the settings.json object.
pub(crate) fn settings_to_json(settings: &Settings) -> Map<String, Value> {
    settings
        .entries()
        .map(|(key, value)| (key.to_string(), setting_value_to_json(value)))
        .collect()
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
        SettingValue::Null => Value::Null,
        SettingValue::Bool(value) => Value::Bool(*value),
        SettingValue::Number(number) => json_number(number),
        SettingValue::Text(text) => Value::String(text.clone()),
        SettingValue::List(items) => Value::Array(items.iter().map(setting_value_to_json).collect()),
        SettingValue::Object(fields) => Value::Object(
            fields
                .iter()
                .map(|(key, value)| (key.clone(), setting_value_to_json(value)))
                .collect(),
        ),
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

/// `VALUE` as JSON, else as a JSON string (`true` → bool, `5` → number, `foo` → "foo").
pub(crate) fn parse_value(value: &str) -> Value {
    serde_json::from_str(value).unwrap_or_else(|_| Value::String(value.to_string()))
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

    #[test]
    fn should_write_null_bool_and_text_setting_values_as_the_same_json() {
        let values = [
            SettingValue::Null,
            SettingValue::Bool(true),
            SettingValue::Text("dark".to_string()),
        ];

        let json: Vec<_> = values.iter().map(setting_value_to_json).collect();

        assert_eq!(json, [json!(null), json!(true), json!("dark")]);
    }

    /// An `inline.profiles.<name>` object is saved with every member, nested lists included.
    #[test]
    fn should_write_setting_lists_and_objects_as_json_all_the_way_down() {
        let json = json!({"model": "m", "max_tokens": 64, "tags": ["a", 1.5, {"x": null}]});
        let value = setting_value_from_json(json.clone());

        let written = setting_value_to_json(&value);

        assert_eq!(written, json);
    }

    #[test]
    fn should_write_a_settings_snapshot_as_the_settings_file_object_it_was_read_from() {
        let Value::Object(map) = json!({"autocomplete.height": 5, "inline.enabled": false}) else {
            unreachable!()
        };
        let settings = settings_from_json(map.clone());

        let written = settings_to_json(&settings);

        assert_eq!(written, map);
    }

    /// Loading and saving settings.json must not rewrite a single number, however it was spelled.
    #[test]
    fn should_write_back_exactly_the_settings_file_text_it_read() {
        let text = r#"{"a": 18446744073709551615, "b": -9223372036854775808, "c": -0, "d": 1e3,
            "e": 0.1, "f": 1.7976931348623157e308, "g": [0, -1, 2.5e-7], "h": {"z": "\u00e9", "a": ""}}"#;
        let Value::Object(map) = serde_json::from_str::<Value>(text).unwrap() else {
            unreachable!()
        };
        let original = serde_json::to_string_pretty(&map).unwrap();

        let written = serde_json::to_string_pretty(&settings_to_json(&settings_from_json(map))).unwrap();

        assert_eq!(written, original);
    }

    #[test]
    fn settings_set_parses_json_or_string() {
        assert_eq!(parse_value("true"), json!(true));
        assert_eq!(parse_value("5"), json!(5));
        assert_eq!(parse_value("[\"git\"]"), json!(["git"]));
        assert_eq!(parse_value("\"x\""), json!("x"));
        assert_eq!(parse_value("most recent"), json!("most recent"));
        assert_eq!(parse_value("{bad"), json!("{bad"));
    }
}
