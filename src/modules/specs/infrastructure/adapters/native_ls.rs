use crate::modules::specs::domain::value_objects::process_output::ProcessOutput;
use std::path::Path;

/// Native answer to `ls -1ApL` in `dir` (SPEC §6.6.4, §9.3).
pub(crate) fn native_ls(dir: &Path) -> ProcessOutput {
    match std::fs::read_dir(dir) {
        Ok(entries) => {
            let names: Vec<String> = entries
                .flatten()
                .map(|e| {
                    let mut name = e.file_name().to_string_lossy().into_owned();
                    if std::fs::metadata(e.path()).is_ok_and(|m| m.is_dir()) {
                        name.push('/'); // -p, following symlinks (-L)
                    }
                    name
                })
                .collect();
            ProcessOutput::new(names.join("\n"), String::new(), 0)
        }
        Err(e) => ProcessOutput::new(String::new(), format!("ls: {}: {e}", dir.display()), 2),
    }
}
