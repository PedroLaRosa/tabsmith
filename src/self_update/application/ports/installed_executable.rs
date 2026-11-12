#[cfg(test)]
use std::cell::RefCell;

/// The tabsmith binary this process runs from.
pub(crate) trait InstalledExecutable {
    /// Puts `bytes`, the binary of the release of `expected_version`, in place of the executable.
    fn replace(&self, bytes: &[u8], expected_version: &str);
}

/// An executable kept in memory: it remembers the bytes it was replaced with.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct InMemoryExecutable {
    installed_bytes: RefCell<Option<Vec<u8>>>,
}

#[cfg(test)]
impl InMemoryExecutable {
    /// The bytes of the last replacement; `None` while the executable is untouched.
    pub(crate) fn installed_bytes(&self) -> Option<Vec<u8>> {
        self.installed_bytes.borrow().clone()
    }
}

#[cfg(test)]
impl InstalledExecutable for InMemoryExecutable {
    fn replace(&self, bytes: &[u8], _expected_version: &str) {
        *self.installed_bytes.borrow_mut() = Some(bytes.to_vec());
    }
}
