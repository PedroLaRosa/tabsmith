use std::error::Error;

/// `error` as anyhow's `{:#}` prints it: its Display, then `: ` and the Display of each `source()` in turn.
pub(crate) fn display_chain(error: &(dyn Error + 'static)) -> String {
    std::iter::successors(Some(error), |&e| e.source())
        .map(|e| e.to_string())
        .collect::<Vec<_>>()
        .join(": ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::shared::infrastructure::os::executable_path_error::ExecutablePathError;
    use std::io;

    #[test]
    fn should_render_the_error_then_each_cause_after_a_colon() {
        let unlocatable = ExecutablePathError::from(io::Error::from(io::ErrorKind::NotFound));

        let text = display_chain(&unlocatable);

        assert_eq!(text, "could not locate the tabsmith executable: entity not found");
    }
}
