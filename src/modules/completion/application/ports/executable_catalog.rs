#[cfg(test)]
use std::cell::Cell;

/// The commands on the PATH, for the first-token suggestions (SPEC §6.9).
pub(crate) trait ExecutableCatalog {
    /// The executables of every directory of the `path` string (split with the OS separator), in PATH order: each
    /// directory sorted by bytes, duplicates across directories kept. `pathext` is read only off unix. `None` -> [].
    fn executables(&self, path: Option<&str>, pathext: Option<&str>) -> Vec<String>;
}

/// Test double: offers the executables it was built with, whatever `path` and `pathext` say, and counts the scans.
#[cfg(test)]
pub(crate) struct FixedExecutableCatalog {
    executables: Vec<String>,
    scans: Cell<usize>,
}

#[cfg(test)]
impl FixedExecutableCatalog {
    pub(crate) fn new(executables: Vec<String>) -> Self {
        FixedExecutableCatalog {
            executables,
            scans: Cell::new(0),
        }
    }

    pub(crate) fn scans(&self) -> usize {
        self.scans.get()
    }
}

#[cfg(test)]
impl ExecutableCatalog for FixedExecutableCatalog {
    fn executables(&self, _path: Option<&str>, _pathext: Option<&str>) -> Vec<String> {
        self.scans.set(self.scans.get() + 1);
        self.executables.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_offer_the_executables_it_was_given() {
        let catalog = FixedExecutableCatalog::new(vec!["mytool".into(), "git".into(), "mytool".into()]);

        let executables = catalog.executables(Some("/usr/bin"), None);

        assert_eq!(executables, ["mytool", "git", "mytool"]);
    }

    #[test]
    fn should_count_every_scan_of_the_path() {
        let catalog = FixedExecutableCatalog::new(vec!["git".into()]);

        catalog.executables(Some("/usr/bin"), None);
        catalog.executables(None, None);

        assert_eq!(catalog.scans(), 2);
    }
}
