use crate::completion::application::ports::executable_catalog::ExecutableCatalog;
use std::path::Path;

/// The `ExecutableCatalog` over the directories of the PATH string.
pub(crate) struct PathExecutableCatalog;

impl ExecutableCatalog for PathExecutableCatalog {
    fn executables(&self, path: Option<&str>, pathext: Option<&str>) -> Vec<String> {
        let mut out = Vec::new();
        for dir in path
            .map(|p| std::env::split_paths(p).collect::<Vec<_>>())
            .unwrap_or_default()
        {
            out.extend(executables(&dir, pathext));
        }
        out
    }
}

#[cfg(unix)]
fn exe_name(name: String, meta: &std::fs::Metadata, _pathext: Option<&str>) -> Option<String> {
    use std::os::unix::fs::PermissionsExt;
    (meta.permissions().mode() & 0o111 != 0).then_some(name)
}

#[cfg(not(unix))]
fn exe_name(name: String, _meta: &std::fs::Metadata, pathext: Option<&str>) -> Option<String> {
    let exts = pathext.unwrap_or(".COM;.EXE;.BAT;.CMD");
    let (stem, ext) = name.rsplit_once('.')?;
    exts.split(';')
        .any(|e| e.trim_start_matches('.').eq_ignore_ascii_case(ext))
        .then(|| stem.to_string())
}

fn executables(dir: &Path, pathext: Option<&str>) -> Vec<String> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<String> = rd
        .flatten()
        .filter_map(|e| {
            let meta = std::fs::metadata(e.path()).ok().filter(|m| m.is_file())?;
            exe_name(e.file_name().to_string_lossy().into_owned(), &meta, pathext)
        })
        .collect();
    out.sort();
    out
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::completion::domain::services::first_token_suggestions::first_token_items;
    use crate::shared::infrastructure::testing::temp_dir::TempDir;
    use crate::specs::domain::value_objects::suggestion_kind::SuggestionKind;
    use std::collections::HashMap;

    #[cfg(unix)]
    #[test]
    fn first_token_sources() {
        use std::os::unix::fs::PermissionsExt;
        let temp_dir = TempDir::new("path");
        let d = temp_dir.path().to_path_buf();
        for (f, mode) in [("mytool", 0o755), ("notexec", 0o644), ("git", 0o755)] {
            std::fs::write(d.join(f), "").unwrap();
            std::fs::set_permissions(d.join(f), std::fs::Permissions::from_mode(mode)).unwrap();
        }
        let specs = vec!["git".to_string(), "aws/s3".into(), "@scope/cli".into()];
        let aliases: HashMap<String, String> =
            [("gst".to_string(), "git status".to_string())].into();
        let items = first_token_items(&specs, &aliases, &PathExecutableCatalog.executables(d.to_str(), None));
        let got: Vec<(&str, SuggestionKind, f64)> = items
            .iter()
            .map(|s| (s.first(), s.kind, s.priority))
            .collect();
        assert_eq!(
            got,
            [
                ("git", SuggestionKind::Subcommand, 40.0),
                ("@scope/cli", SuggestionKind::Subcommand, 40.0),
                ("gst", SuggestionKind::Shortcut, 100.0),
                ("mytool", SuggestionKind::Subcommand, 40.0)
            ]
        );
        assert_eq!(items[2].description.as_deref(), Some("git status"));
        std::fs::remove_dir_all(&d).unwrap();
    }
}
