/// Kills the processes the completion generators started, so none outlives the session. Fire and forget: nothing
/// is reported, and a call with nothing running does nothing.
pub(crate) trait GeneratorProcessReaper {
    fn kill_live_children(&self);
}

/// Test double: counts the kills.
#[cfg(test)]
pub(crate) struct RecordingGeneratorProcessReaper {
    kills: std::cell::Cell<usize>,
}

#[cfg(test)]
impl RecordingGeneratorProcessReaper {
    pub(crate) fn new() -> Self {
        RecordingGeneratorProcessReaper { kills: std::cell::Cell::new(0) }
    }

    pub(crate) fn kills(&self) -> usize {
        self.kills.get()
    }
}

#[cfg(test)]
impl GeneratorProcessReaper for RecordingGeneratorProcessReaper {
    fn kill_live_children(&self) {
        self.kills.set(self.kills.get() + 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_count_each_kill_of_the_live_children() {
        let reaper = RecordingGeneratorProcessReaper::new();

        reaper.kill_live_children();
        reaper.kill_live_children();

        assert_eq!(reaper.kills(), 2);
    }
}
