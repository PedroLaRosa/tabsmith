use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;

/// Asks the file system about the target of a `cd` completion.
pub(crate) trait DirectoryProbe {
    /// Stands for `Path::is_dir`.
    fn is_dir(&self, path: &Path) -> bool;

    /// Stands for `path.canonicalize().is_ok_and(|p| p.is_dir())`.
    fn resolves_to_directory(&self, path: &Path) -> bool;

    /// The home directory `expand` falls back to when the environment map has no `HOME`/`USERPROFILE`.
    fn home_directory(&self) -> Option<String>;
}

/// Knows only the directories it was given: `is_dir` and `resolves_to_directory` both answer whether the path
/// equals one of them (nothing in memory is a symlink or holds `..`).
#[cfg(test)]
pub(crate) struct InMemoryDirectoryProbe {
    directories: Vec<PathBuf>,
    home: Option<String>,
}

#[cfg(test)]
impl InMemoryDirectoryProbe {
    pub(crate) fn new(directories: &[&str], home: Option<&str>) -> Self {
        Self {
            directories: directories.iter().map(PathBuf::from).collect(),
            home: home.map(String::from),
        }
    }
}

#[cfg(test)]
impl DirectoryProbe for InMemoryDirectoryProbe {
    fn is_dir(&self, path: &Path) -> bool {
        self.directories.iter().any(|directory| directory == path)
    }

    fn resolves_to_directory(&self, path: &Path) -> bool {
        self.is_dir(path)
    }

    fn home_directory(&self) -> Option<String> {
        self.home.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_know_only_the_directories_it_was_given() {
        let probe = InMemoryDirectoryProbe::new(&["/work/src"], None);

        let answers = (
            probe.is_dir(Path::new("/work/src")),
            probe.resolves_to_directory(Path::new("/work/src")),
            probe.is_dir(Path::new("/work/docs")),
            probe.resolves_to_directory(Path::new("/work/docs")),
        );

        assert_eq!(answers, (true, true, false, false));
    }

    #[test]
    fn should_have_no_home_unless_given_one() {
        let without = InMemoryDirectoryProbe::new(&[], None);
        let with = InMemoryDirectoryProbe::new(&[], Some("/home/u"));

        let homes = (without.home_directory(), with.home_directory());

        assert_eq!(homes, (None, Some("/home/u".to_string())));
    }
}
