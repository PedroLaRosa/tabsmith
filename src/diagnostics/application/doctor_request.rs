/// What the user asked of `tabsmith doctor`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DoctorRequest {
    all: bool,
    strict: bool,
}

impl DoctorRequest {
    pub(crate) fn new(all: bool, strict: bool) -> Self {
        DoctorRequest { all, strict }
    }

    /// Run and show every check, even those that pass, and fix nothing.
    pub(crate) fn all(&self) -> bool {
        self.all
    }

    /// Soft problems count as errors.
    pub(crate) fn strict(&self) -> bool {
        self.strict
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_keep_the_all_and_strict_flags_it_was_built_with() {
        let request = DoctorRequest::new(true, false);

        assert_eq!((request.all(), request.strict()), (true, false));
    }

    #[test]
    fn should_stay_usable_after_a_request_is_copied() {
        let request = DoctorRequest::new(false, true);

        let copy = request;

        assert_eq!(copy, request);
    }
}
