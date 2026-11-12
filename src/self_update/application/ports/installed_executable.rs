use crate::self_update::domain::value_objects::install_location::InstallLocation;
#[cfg(test)]
use std::cell::RefCell;
#[cfg(test)]
use std::path::PathBuf;

/// The tabsmith binary this process runs from.
pub(crate) trait InstalledExecutable {
    /// Where it is, and the user home that tells whether that is the native install.
    fn location(&self) -> InstallLocation;
    /// Puts `bytes`, the binary of the release of `expected_version`, in place of the executable.
    fn replace(&self, bytes: &[u8], expected_version: &str);
}

/// An executable kept in memory, by default the native install of `/home/ana`: it remembers the bytes it was
/// replaced with.
#[cfg(test)]
pub(crate) struct InMemoryExecutable {
    location: InstallLocation,
    installed_bytes: RefCell<Option<Vec<u8>>>,
}

#[cfg(test)]
impl InMemoryExecutable {
    pub(crate) fn at(location: InstallLocation) -> Self {
        InMemoryExecutable {
            location,
            installed_bytes: RefCell::new(None),
        }
    }

    /// The bytes of the last replacement; `None` while the executable is untouched.
    pub(crate) fn installed_bytes(&self) -> Option<Vec<u8>> {
        self.installed_bytes.borrow().clone()
    }
}

#[cfg(test)]
impl Default for InMemoryExecutable {
    fn default() -> Self {
        InMemoryExecutable::at(InstallLocation::new(
            PathBuf::from("/home/ana/.local/bin/tabsmith"),
            PathBuf::from("/home/ana"),
        ))
    }
}

#[cfg(test)]
impl InstalledExecutable for InMemoryExecutable {
    fn location(&self) -> InstallLocation {
        self.location.clone()
    }

    fn replace(&self, bytes: &[u8], _expected_version: &str) {
        *self.installed_bytes.borrow_mut() = Some(bytes.to_vec());
    }
}
