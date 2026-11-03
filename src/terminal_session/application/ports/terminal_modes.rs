/// Gives the user's terminal back the modes it had before the session took it over. Fire and forget: it never
/// fails, and the session calls it on every way out.
pub(crate) trait TerminalModes {
    fn restore(&self);
}

/// Test double: counts the restores.
#[cfg(test)]
pub(crate) struct RecordingTerminalModes {
    restores: std::cell::Cell<usize>,
}

#[cfg(test)]
impl RecordingTerminalModes {
    pub(crate) fn new() -> Self {
        RecordingTerminalModes { restores: std::cell::Cell::new(0) }
    }

    pub(crate) fn restores(&self) -> usize {
        self.restores.get()
    }
}

#[cfg(test)]
impl TerminalModes for RecordingTerminalModes {
    fn restore(&self) {
        self.restores.set(self.restores.get() + 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_count_each_restore() {
        let modes = RecordingTerminalModes::new();

        modes.restore();
        modes.restore();

        assert_eq!(modes.restores(), 2);
    }
}
