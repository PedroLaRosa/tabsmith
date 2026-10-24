use crate::settings::domain::value_objects::setting_number::SettingNumber;
use crate::settings::domain::value_objects::setting_value::SettingValue;
use serde_json::Value;

/// A serde_json value as a setting value, integers and floats kept apart.
pub(crate) fn setting_value_from_json(value: Value) -> SettingValue {
    match value {
        Value::Number(number) => SettingValue::Number(SettingNumber::from(number.as_u64().unwrap())),
        _ => todo!(),
    }
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
}
