use crate::modules::self_update::domain::self_update_error::SelfUpdateError;
use crate::modules::self_update::domain::value_objects::install_location::InstallLocation;
#[cfg(test)]
use std::cell::RefCell;
#[cfg(test)]
use std::path::{Path, PathBuf};

/// The tabsmith binary this process runs from.
pub(crate) trait InstalledExecutable {
    /// Where it is, and the user home that tells whether that is the native install.
    fn location(&self) -> InstallLocation;
    /// Puts `bytes`, the binary of the release of `expected_version`, in place of the executable, once it runs and
    /// reports that version. A failure is `InstallFailed` and leaves the executable as it was.
    fn replace(&self, bytes: &[u8], expected_version: &str) -> Result<(), SelfUpdateError>;
}

/// An executable kept in memory, by default the native install of `/home/ana`: it remembers the bytes it was
/// replaced with, or refuses every replacement for the reason it was given.
#[cfg(test)]
pub(crate) struct InMemoryExecutable {
    location: InstallLocation,
    refusal: Option<String>,
    installed_bytes: RefCell<Option<Vec<u8>>>,
}

#[cfg(test)]
impl InMemoryExecutable {
    pub(crate) fn at(location: InstallLocation) -> Self {
        InMemoryExecutable {
            location,
            refusal: None,
            installed_bytes: RefCell::new(None),
        }
    }

    /// The native install, failing every replacement with `reason`, as a binary that does not run would.
    pub(crate) fn refusing(reason: &str) -> Self {
        InMemoryExecutable {
            refusal: Some(reason.to_string()),
            ..Self::default()
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
            InstallLocation::native_dir(Path::new("/home/ana")),
        ))
    }
}

#[cfg(test)]
impl InstalledExecutable for InMemoryExecutable {
    fn location(&self) -> InstallLocation {
        self.location.clone()
    }

    fn replace(&self, bytes: &[u8], _expected_version: &str) -> Result<(), SelfUpdateError> {
        if let Some(reason) = &self.refusal {
            return Err(SelfUpdateError::InstallFailed(reason.clone()));
        }
        *self.installed_bytes.borrow_mut() = Some(bytes.to_vec());
        Ok(())
    }
}
