/// One executed command (SPEC §3.5). The glossary's identity is (session id, start time), but equality stays
/// structural: entries of one session started within the same second share an identity.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct HistoryEntry {
    command: String,
    shell: String,
    pid: Option<i32>,
    session_id: String,
    cwd: Option<String>,
    start_time: u64,
    end_time: Option<u64>,
    exit_code: Option<i32>,
    hostname: Option<String>,
}

/// A `HistoryEntry` as plain values: what it is built from and persisted as.
#[derive(Default)]
pub(crate) struct HistoryEntryPrimitives {
    pub(crate) command: String,
    /// The raw OSC `Shell` marker value, not a `Shell`: history keeps what the shell reported.
    pub(crate) shell: String,
    pub(crate) pid: Option<i32>,
    pub(crate) session_id: String,
    pub(crate) cwd: Option<String>,
    /// Seconds since the epoch.
    pub(crate) start_time: u64,
    /// Seconds since the epoch.
    pub(crate) end_time: Option<u64>,
    pub(crate) exit_code: Option<i32>,
    pub(crate) hostname: Option<String>,
}

impl HistoryEntry {
    pub(crate) fn new(primitives: HistoryEntryPrimitives) -> Self {
        let HistoryEntryPrimitives {
            command,
            shell,
            pid,
            session_id,
            cwd,
            start_time,
            end_time,
            exit_code,
            hostname,
        } = primitives;
        HistoryEntry {
            command,
            shell,
            pid,
            session_id,
            cwd,
            start_time,
            end_time,
            exit_code,
            hostname,
        }
    }

    /// The command ended at `end_time` (seconds) with `exit_code`, if the shell reported one.
    pub(crate) fn finish(&mut self, exit_code: Option<i32>, end_time: u64) {
        self.exit_code = exit_code;
        self.end_time = Some(end_time);
    }

    pub(crate) fn command(&self) -> &str {
        &self.command
    }

    pub(crate) fn pid(&self) -> Option<i32> {
        self.pid
    }

    pub(crate) fn exit_code(&self) -> Option<i32> {
        self.exit_code
    }

    pub(crate) fn to_primitives(&self) -> HistoryEntryPrimitives {
        HistoryEntryPrimitives {
            command: self.command.clone(),
            shell: self.shell.clone(),
            pid: self.pid,
            session_id: self.session_id.clone(),
            cwd: self.cwd.clone(),
            start_time: self.start_time,
            end_time: self.end_time,
            exit_code: self.exit_code,
            hostname: self.hostname.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_record_the_exit_code_and_end_time_when_the_command_finishes() {
        let mut entry = HistoryEntry::new(HistoryEntryPrimitives {
            command: "ls".into(),
            start_time: 10,
            ..Default::default()
        });

        entry.finish(Some(2), 12);

        let primitives = entry.to_primitives();
        assert_eq!((primitives.exit_code, primitives.end_time), (Some(2), Some(12)));
    }
}
