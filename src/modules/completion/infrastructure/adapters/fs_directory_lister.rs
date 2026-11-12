use crate::modules::completion::application::ports::directory_lister::DirectoryLister;
use crate::modules::completion::domain::services::suggestion_filter::cmp_locale;
use crate::modules::specs::domain::value_objects::spec_suggestion::SpecSuggestion;
use crate::modules::specs::domain::value_objects::suggestion_kind::SuggestionKind;
use std::path::Path;

/// The `DirectoryLister` over the file system.
pub(crate) struct FsDirectoryLister;

impl DirectoryLister for FsDirectoryLister {
    fn list(&self, dir: &Path, files: bool, is_dangerous: bool) -> Vec<SpecSuggestion> {
        list_dir(dir, files, is_dangerous)
    }
}

/// Native `filepaths`/`folders` listing: `ls -1ApL` semantics, sorted non-dot then dot names, `.DS_Store`
/// skipped, `../` appended. Unreadable dir → `[]`.
pub(crate) fn list_dir(dir: &Path, files: bool, is_dangerous: bool) -> Vec<SpecSuggestion> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = rd
        .flatten()
        .map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let is_dir = std::fs::metadata(e.path()).is_ok_and(|m| m.is_dir()); // follows symlinks (-L)
            if is_dir {
                format!("{name}/")
            } else {
                name
            }
        })
        .filter(|n| !n.eq_ignore_ascii_case(".DS_Store"))
        .filter(|n| !n.chars().any(char::is_control)) // a CR in a name would run the line when inserted
        .collect();
    names.sort_by(|a, b| {
        a.starts_with('.')
            .cmp(&b.starts_with('.'))
            .then_with(|| cmp_locale(a, b))
    });
    names.push("../".into());
    names
        .into_iter()
        .filter_map(|n| {
            let folder = n.ends_with('/');
            if !folder && !files {
                return None;
            }
            Some(
                SpecSuggestion::default()
                    .with_insert_value(Some(n.clone()))
                    .with_kind(Some(if folder {
                        SuggestionKind::Folder
                    } else {
                        SuggestionKind::File
                    }))
                    .with_is_dangerous(is_dangerous)
                    .with_template_type(Some(if folder { "folders" } else { "filepaths" }.into()))
                    .with_names(vec![n]),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::shared::infrastructure::testing::temp_dir::TempDir;

    #[test]
    fn list_dir_sorted() {
        let temp_dir = TempDir::new("ls");
        let d = temp_dir.path().to_path_buf();
        for f in ["c.txt", "B.txt", ".hidden", ".DS_Store", "a.txt"] {
            std::fs::write(d.join(f), "").unwrap();
        }
        std::fs::create_dir(d.join("src")).unwrap();
        let got: Vec<String> = list_dir(&d, true, false)
            .into_iter()
            .map(|s| s.names()[0].clone())
            .collect();
        assert_eq!(got, ["a.txt", "B.txt", "c.txt", "src/", ".hidden", "../"]);
        let folders = list_dir(&d, false, false);
        assert_eq!(
            folders
                .iter()
                .map(|s| s.names()[0].as_str())
                .collect::<Vec<_>>(),
            ["src/", "../"]
        );
        assert_eq!(folders[0].template_type().as_deref(), Some("folders"));
        assert!(list_dir(&d.join("missing"), true, false).is_empty());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[cfg(unix)] // Windows file names cannot hold control chars
    #[test]
    fn list_dir_skips_control_char_names() {
        let temp_dir = TempDir::new("ctl");
        let d = temp_dir.path().to_path_buf();
        for f in ["ok", "aa\rtouch${IFS}PWNED\r", "esc\x1b[2J"] {
            std::fs::write(d.join(f), "").unwrap();
        }
        let got: Vec<String> = list_dir(&d, true, false)
            .into_iter()
            .map(|s| s.names()[0].clone())
            .collect();
        assert_eq!(got, ["ok", "../"]);
        std::fs::remove_dir_all(&d).unwrap();
    }
}
