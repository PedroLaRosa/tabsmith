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
            Representation::Unsigned(number) => Some(number as i64),
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
}
