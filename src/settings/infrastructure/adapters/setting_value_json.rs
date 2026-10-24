use crate::settings::domain::value_objects::setting_number::SettingNumber;
use crate::settings::domain::value_objects::setting_value::SettingValue;
use serde_json::Value;

/// A serde_json value as a setting value, integers and floats kept apart.
pub(crate) fn setting_value_from_json(value: Value) -> SettingValue {
    match value {
        Value::Number(number) => SettingValue::Number(setting_number(&number)),
        _ => todo!(),
    }
}

/// serde_json's `PosInt`, `NegInt` and `Float`, read in that order.
fn setting_number(number: &serde_json::Number) -> SettingNumber {
    match number.as_u64() {
        Some(unsigned) => SettingNumber::from(unsigned),
        None => SettingNumber::from(number.as_i64().unwrap()),
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

    #[test]
    fn should_keep_a_negative_json_integer_a_negative_integer() {
        let json = json!(-3);

        let value = setting_value_from_json(json);

        assert_eq!(value, SettingValue::Number(SettingNumber::from(-3i64)));
    }
}
