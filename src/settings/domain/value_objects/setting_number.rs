/// A JSON number that keeps integers and floats apart the way serde_json's `Number` does, so a stored `5` stays `5`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SettingNumber(Representation);

/// serde_json's `PosInt`, `NegInt` and `Float`.
#[derive(Clone, Copy, Debug)]
enum Representation {
    Unsigned(u64),
}

impl From<u64> for SettingNumber {
    fn from(number: u64) -> Self {
        SettingNumber(Representation::Unsigned(number))
    }
}

impl SettingNumber {
    pub(crate) fn as_i64(&self) -> Option<i64> {
        match self.0 {
            Representation::Unsigned(number) => i64::try_from(number).ok(),
        }
    }

    pub(crate) fn as_u64(&self) -> Option<u64> {
        match self.0 {
            Representation::Unsigned(number) => Some(number),
        }
    }

    pub(crate) fn as_f64(&self) -> Option<f64> {
        match self.0 {
            Representation::Unsigned(number) => Some(number as f64),
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
}
