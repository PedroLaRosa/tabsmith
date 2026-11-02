use crate::diagnostics::domain::value_objects::check_fix::CheckFix;
use crate::diagnostics::domain::value_objects::info_line::InfoLine;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum CheckOutcome {
    Ok,
    Warn(String),
    Err {
        reason: String,
        info: Vec<InfoLine>,
        fix: Option<CheckFix>,
    },
}

impl CheckOutcome {
    /// A failure with nothing more to say and nothing to repair it.
    pub(crate) fn err(reason: impl Into<String>) -> Self {
        CheckOutcome::Err {
            reason: reason.into(),
            info: Vec::new(),
            fix: None,
        }
    }

    pub(crate) fn is_ok(&self) -> bool {
        matches!(self, CheckOutcome::Ok)
    }

    pub(crate) fn is_err(&self) -> bool {
        matches!(self, CheckOutcome::Err { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_build_an_error_without_info_and_without_a_fix() {
        let outcome = CheckOutcome::err("data directory is missing");

        assert_eq!(
            outcome,
            CheckOutcome::Err {
                reason: "data directory is missing".into(),
                info: Vec::new(),
                fix: None,
            }
        );
    }

    #[test]
    fn should_be_ok_only_for_the_ok_outcome() {
        let outcomes = [CheckOutcome::Ok, CheckOutcome::Warn("w".into()), CheckOutcome::err("e")];

        let oks: Vec<bool> = outcomes.iter().map(CheckOutcome::is_ok).collect();

        assert_eq!(oks, [true, false, false]);
    }

    #[test]
    fn should_be_an_error_only_for_the_err_outcome() {
        let outcomes = [CheckOutcome::Ok, CheckOutcome::Warn("w".into()), CheckOutcome::err("e")];

        let errs: Vec<bool> = outcomes.iter().map(CheckOutcome::is_err).collect();

        assert_eq!(errs, [false, false, true]);
    }
}
