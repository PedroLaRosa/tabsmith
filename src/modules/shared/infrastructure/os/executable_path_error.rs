use std::fmt;
use std::io;

/// The running tabsmith binary could not be located. Its Display and `source()` chain print what `exe_path`'s
/// `anyhow` context printed before (`{:#}`): "could not locate the tabsmith executable: <I/O error>".
#[derive(Debug)]
pub(crate) struct ExecutablePathError {
    source: io::Error,
}

impl From<io::Error> for ExecutablePathError {
    fn from(source: io::Error) -> Self {
        ExecutablePathError { source }
    }
}

impl fmt::Display for ExecutablePathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "could not locate the tabsmith executable")
    }
}

impl std::error::Error for ExecutablePathError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_report_the_executable_it_could_not_locate_then_the_io_cause() {
        let error = ExecutablePathError::from(io::Error::from(io::ErrorKind::NotFound));

        assert_eq!(
            format!("{:#}", anyhow::Error::from(error)),
            "could not locate the tabsmith executable: entity not found"
        );
    }
}
