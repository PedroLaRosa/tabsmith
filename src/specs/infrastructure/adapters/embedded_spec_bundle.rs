use crate::shared::infrastructure::error_chain::display_chain;
use crate::specs::domain::spec_error::SpecError;
use std::error::Error;
use std::fmt::Display;

/// The bundle cannot be used: `context`, then `cause` and each of its sources, as anyhow's `{:#}` printed them.
fn unavailable(context: impl Display, cause: &(dyn Error + 'static)) -> SpecError {
    SpecError::BundleUnavailable(format!("{context}: {}", display_chain(cause)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt;
    use std::io;

    /// What anyhow's `{:#}` printed for `cause` behind `context`, before the bundle had its own error.
    fn like_anyhow(cause: impl Error + Send + Sync + 'static, context: &str) -> String {
        format!("{:#}", anyhow::Error::new(cause).context(context.to_string()))
    }

    /// What `tar` wraps in an io::Error: its own text, with the OS error as the source.
    #[derive(Debug)]
    struct UnpackFailure(io::Error);

    impl fmt::Display for UnpackFailure {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("failed to unpack `aws/s3.js`")
        }
    }

    impl Error for UnpackFailure {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            Some(&self.0)
        }
    }

    #[test]
    fn should_render_an_os_error_like_anyhow() {
        let cause = || io::Error::from_raw_os_error(17);

        let failure = unavailable("creating /home/u/.tabsmith/specs", &cause());

        assert_eq!(
            failure,
            SpecError::BundleUnavailable(like_anyhow(cause(), "creating /home/u/.tabsmith/specs"))
        );
    }

    #[test]
    fn should_render_the_nested_source_of_an_io_error_like_anyhow() {
        let cause = || io::Error::other(UnpackFailure(io::Error::from_raw_os_error(28)));

        let failure = unavailable("extracting specs into /s/1.0.0.tmp-7", &cause());

        assert_eq!(
            failure,
            SpecError::BundleUnavailable(like_anyhow(cause(), "extracting specs into /s/1.0.0.tmp-7"))
        );
    }

    #[test]
    fn should_render_a_json_error_like_anyhow() {
        let cause = || serde_json::from_str::<serde_json::Value>("{").unwrap_err();

        let failure = unavailable("parsing /s/1.0.0/index.json", &cause());

        assert_eq!(
            failure,
            SpecError::BundleUnavailable(like_anyhow(cause(), "parsing /s/1.0.0/index.json"))
        );
    }
}
