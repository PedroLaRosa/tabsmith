use crate::shared::infrastructure::os::process_info::parent_exe;
use std::io::Write;

/// `_ get-shell`: the parent process' executable, with no newline; exit 1 and nothing printed when it is unknown.
pub(crate) fn run_get_shell_command() -> anyhow::Result<i32> {
    match parent_exe() {
        Some(p) => {
            print!("{p}");
            let _ = std::io::stdout().flush();
            Ok(0)
        }
        None => Ok(1),
    }
}
