/// Normal; Soft (an error becomes a warning unless `--strict`); Skip (not run).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum CheckSeverity {
    Normal,
    Soft,
    Skip,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_tell_the_three_severities_apart() {
        let severities = [CheckSeverity::Normal, CheckSeverity::Soft, CheckSeverity::Skip];

        assert_ne!(severities[0], severities[1]);
        assert_ne!(severities[1], severities[2]);
        assert_ne!(severities[0], severities[2]);
    }
}
