/// A JSON number that keeps integers and floats apart the way serde_json's `Number` does, so a stored `5` stays `5`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SettingNumber(Representation);

/// serde_json's `PosInt`, `NegInt` and `Float`: `Negative` is below zero only, a finite `Float` only.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Representation {
    Unsigned(u64),
    Negative(i64),
    Float(f64),
}

impl From<u64> for SettingNumber {
    fn from(number: u64) -> Self {
        SettingNumber(Representation::Unsigned(number))
    }
}

impl From<i64> for SettingNumber {
    fn from(number: i64) -> Self {
        match u64::try_from(number) {
            Ok(unsigned) => SettingNumber(Representation::Unsigned(unsigned)),
            Err(_) => SettingNumber(Representation::Negative(number)),
        }
    }
}

impl SettingNumber {
    /// `None` for NaN and the infinities, which are not JSON numbers.
    pub(crate) fn from_f64(number: f64) -> Option<Self> {
        number
            .is_finite()
            .then_some(SettingNumber(Representation::Float(number)))
    }

    pub(crate) fn as_i64(&self) -> Option<i64> {
        match self.0 {
            Representation::Unsigned(number) => i64::try_from(number).ok(),
            Representation::Negative(number) => Some(number),
            Representation::Float(_) => None,
        }
    }

    pub(crate) fn as_u64(&self) -> Option<u64> {
        match self.0 {
            Representation::Unsigned(number) => Some(number),
            Representation::Negative(_) | Representation::Float(_) => None,
        }
    }

    pub(crate) fn as_f64(&self) -> Option<f64> {
        match self.0 {
            Representation::Unsigned(number) => Some(number as f64),
            Representation::Negative(number) => Some(number as f64),
            Representation::Float(number) => Some(number),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_read_a_positive_integer_as_every_number_type() {
        let number = SettingNumber::from(12u64);

        let read = (number.as_i64(), number.as_u64(), number.as_f64());

        assert_eq!(read, (Some(12), Some(12), Some(12.0)));
    }

    #[test]
    fn should_not_read_an_integer_above_the_i64_range_as_an_i64() {
        let number = SettingNumber::from(i64::MAX as u64 + 1);

        let read = number.as_i64();

        assert_eq!(read, None);
    }

    #[test]
    fn should_read_a_negative_integer_as_an_i64_and_an_f64_but_not_as_a_u64() {
        let number = SettingNumber::from(-3i64);

        let read = (number.as_i64(), number.as_u64(), number.as_f64());

        assert_eq!(read, (Some(-3), None, Some(-3.0)));
    }

    #[test]
    fn should_read_a_non_negative_i64_as_a_u64_too() {
        let number = SettingNumber::from(5i64);

        let read = number.as_u64();

        assert_eq!(read, Some(5));
    }

    /// `inline.historyCount: 10.0` falls back to the default because a float is not an i64.
    #[test]
    fn should_read_a_float_only_as_an_f64_even_when_it_is_whole() {
        let number = SettingNumber::from_f64(12.0).unwrap();

        let read = (number.as_i64(), number.as_u64(), number.as_f64());

        assert_eq!(read, (None, None, Some(12.0)));
    }

    #[test]
    fn should_refuse_a_float_that_json_cannot_hold() {
        let floats = [f64::NAN, f64::INFINITY, f64::NEG_INFINITY];

        let numbers: Vec<_> = floats.into_iter().map(SettingNumber::from_f64).collect();

        assert!(numbers.iter().all(Option::is_none), "{numbers:?}");
    }

    /// `settings autocomplete.height 5` stores `5`, not `5.0`.
    #[test]
    fn should_tell_an_integer_from_the_float_of_the_same_value() {
        let integer = SettingNumber::from(5u64);

        let float = SettingNumber::from_f64(5.0).unwrap();

        assert_ne!(integer, float);
    }
}
