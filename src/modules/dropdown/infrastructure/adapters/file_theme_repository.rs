use crate::modules::dropdown::domain::dropdown_error::DropdownError;
use crate::modules::dropdown::domain::repositories::theme_repository::ThemeRepository;
use crate::modules::dropdown::domain::value_objects::theme_definition::ThemeDefinition;
use crate::modules::dropdown::domain::value_objects::theme_palette::ThemePalette;
use crate::modules::dropdown::infrastructure::adapters::theme_json_mapper::{
    parse_theme_definition, parse_theme_palette,
};
use crate::modules::shared::domain::value_objects::data_directory::DataDirectory;
use std::path::PathBuf;

/// The custom themes as `NAME.json` files in the data directory's themes folder. The folder is resolved and the
/// file read on every call, and the folder is never created.
pub(crate) struct FileThemeRepository {
    data_directory: DataDirectory,
}

impl FileThemeRepository {
    pub(crate) fn new(data_directory: DataDirectory) -> Self {
        FileThemeRepository { data_directory }
    }

    /// The name is joined as typed: `../up` and `sub/x` reach other folders, as they always did.
    fn theme_file(&self, name: &str) -> Option<PathBuf> {
        Some(
            self.data_directory
                .themes_dir()
                .ok()?
                .join(format!("{name}.json")),
        )
    }
}

impl ThemeRepository for FileThemeRepository {
    fn folder(&self) -> Result<PathBuf, DropdownError> {
        self.data_directory
            .themes_dir()
            .map_err(DropdownError::DataDirectory)
    }

    fn find(&self, name: &str) -> Result<Option<ThemeDefinition>, DropdownError> {
        let Some(path) = self.theme_file(name) else {
            return Ok(None);
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Ok(None);
        };
        parse_theme_definition(&text)
            .map(Some)
            .map_err(|error| DropdownError::InvalidThemeFile {
                path,
                reason: error.to_string(),
            })
    }

    fn list(&self) -> Vec<String> {
        let Ok(folder) = self.data_directory.themes_dir() else {
            return Vec::new();
        };
        let mut files: Vec<String> = std::fs::read_dir(folder)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let p = e.path();
                if p.extension()? != "json" {
                    return None;
                }
                Some(p.file_stem()?.to_str()?.to_string())
            })
            .collect();
        files.sort();
        files.dedup();
        files
    }

    fn find_palette(&self, name: &str) -> Option<ThemePalette> {
        let text = std::fs::read_to_string(self.theme_file(name)?).ok()?;
        parse_theme_palette(&text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::dropdown::domain::value_objects::theme_definition::ThemeDefinition;
    use crate::modules::dropdown::domain::value_objects::theme_palette::ThemePalette;
    use crate::modules::shared::infrastructure::testing::temp_dir::TempDir;
    use std::fs;
    use std::path::Path;

    fn repository_in(root: &Path) -> FileThemeRepository {
        FileThemeRepository::new(DataDirectory::new(Some(root.to_path_buf()), None))
    }

    fn author_name(definition: &ThemeDefinition) -> Option<&str> {
        definition.author().and_then(|author| author.name())
    }

    #[test]
    fn should_find_a_valid_theme_file() {
        let temp_dir = TempDir::new("themes-find");
        fs::create_dir(temp_dir.path().join("themes")).unwrap();
        fs::write(
            temp_dir.path().join("themes/ocean.json"),
            r#"{"author":{"name":"Ann","github":"ann"}}"#,
        )
        .unwrap();

        let found = repository_in(temp_dir.path()).find("ocean").unwrap();

        let author = found.as_ref().and_then(|definition| definition.author());
        assert_eq!(
            (
                author.and_then(|author| author.name()),
                author.and_then(|author| author.github())
            ),
            (Some("Ann"), Some("ann"))
        );
    }

    #[test]
    fn should_find_nothing_for_a_missing_file_or_a_directory_named_like_a_theme() {
        let temp_dir = TempDir::new("themes-missing");
        fs::create_dir_all(temp_dir.path().join("themes/dirtheme.json")).unwrap();
        let repository = repository_in(temp_dir.path());

        let found = (
            repository.find("missing").unwrap(),
            repository.find("dirtheme").unwrap(),
        );

        assert_eq!(found, (None, None));
    }

    #[test]
    fn should_find_nothing_for_a_file_that_is_not_utf8() {
        let temp_dir = TempDir::new("themes-binary");
        fs::create_dir(temp_dir.path().join("themes")).unwrap();
        fs::write(temp_dir.path().join("themes/bin.json"), [0xff, 0xfe]).unwrap();

        let found = repository_in(temp_dir.path()).find("bin").unwrap();

        assert_eq!(found, None);
    }

    #[test]
    fn should_report_an_invalid_theme_file_with_its_path() {
        let temp_dir = TempDir::new("themes-invalid");
        fs::create_dir(temp_dir.path().join("themes")).unwrap();
        fs::write(
            temp_dir.path().join("themes/bad.json"),
            r#"{"author":"Ann"}"#,
        )
        .unwrap();

        let error = repository_in(temp_dir.path()).find("bad").unwrap_err();

        let DropdownError::InvalidThemeFile { path, reason } = error else {
            panic!("expected an invalid theme file, got {error:?}");
        };
        assert_eq!(
            (path, reason.as_str()),
            (
                temp_dir.path().join("themes").join("bad.json"),
                "invalid type: string \"Ann\", expected struct ThemeAuthor at line 1 column 15"
            )
        );
    }

    #[test]
    fn should_find_nothing_without_a_home() {
        let repository = FileThemeRepository::new(DataDirectory::new(None, None));

        let result = (repository.find("x").unwrap(), repository.list());

        assert_eq!(result, (None, Vec::<String>::new()));
    }

    #[test]
    fn should_fail_to_name_the_folder_without_a_home() {
        let repository = FileThemeRepository::new(DataDirectory::new(None, None));

        let error = repository.folder().unwrap_err();

        assert_eq!(
            error.to_string(),
            "could not determine the home directory (set HOME or TABSMITH_HOME)"
        );
    }

    #[test]
    fn should_list_the_json_stems_sorted_and_deduped() {
        let temp_dir = TempDir::new("themes-list");
        let themes = temp_dir.path().join("themes");
        fs::create_dir_all(themes.join("dirtheme.json")).unwrap();
        for file in [
            "zed.json",
            "alpha.json",
            "dark.json",
            "Beta.json",
            "notes.txt",
            "UP.JSON",
            "x.json.bak",
            ".hidden.json",
            "a.b.json",
            "noext",
        ] {
            fs::write(themes.join(file), "{}").unwrap();
        }

        let listed = repository_in(temp_dir.path()).list();

        // `Beta` is the one uppercase stem: it pins byte order (`.` < `B` < `a`). A directory has no duplicate
        // stems, so `dedup()` cannot be observed here.
        assert_eq!(
            listed,
            [".hidden", "Beta", "a.b", "alpha", "dark", "dirtheme", "zed"]
        );
    }

    #[test]
    fn theme_list_order() {
        let temp_dir = TempDir::new("themes");
        let d = temp_dir.path().to_path_buf();
        fs::create_dir(d.join("themes")).unwrap();
        for f in ["zed.json", "alpha.json", "dark.json", "notes.txt"] {
            fs::write(d.join("themes").join(f), "{}").unwrap();
        }

        let listed = repository_in(&d).list();
        let listed_without_a_folder = repository_in(&d.join("missing")).list();

        assert_eq!(listed, ["alpha", "dark", "zed"]);
        assert!(listed_without_a_folder.is_empty());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn should_not_create_the_themes_directory() {
        let temp_dir = TempDir::new("themes-not-created");
        let repository = repository_in(temp_dir.path());

        let seen = (
            repository.folder().unwrap(),
            repository.find("x").unwrap(),
            repository.list(),
            temp_dir.path().join("themes").exists(),
        );

        assert_eq!(
            seen,
            (
                temp_dir.path().join("themes"),
                None,
                Vec::<String>::new(),
                false
            )
        );
    }

    #[test]
    fn should_join_the_name_unsanitised() {
        let temp_dir = TempDir::new("themes-join");
        let root = temp_dir.path();
        fs::create_dir_all(root.join("themes/sub")).unwrap();
        for (file, author) in [
            ("up.json", "Up"),
            ("themes/sub/x.json", "Sub"),
            ("themes/my.theme.json", "Dotted"),
            ("themes/my.json", "Plain"),
            ("abs.json", "Absolute"),
        ] {
            fs::write(
                root.join(file),
                format!(r#"{{"author":{{"name":"{author}"}}}}"#),
            )
            .unwrap();
        }
        let repository = repository_in(root);
        let absolute = root.join("abs").display().to_string();

        let authors = ["../up", "sub/x", "my.theme", absolute.as_str()]
            .map(|name| repository.find(name).unwrap());

        assert_eq!(
            authors
                .each_ref()
                .map(|found| found.as_ref().and_then(author_name)),
            [Some("Up"), Some("Sub"), Some("Dotted"), Some("Absolute")]
        );
    }

    #[cfg(unix)]
    #[test]
    fn should_find_nothing_for_an_unreadable_file() {
        use crate::modules::shared::infrastructure::os::process_info::is_root;
        use std::os::unix::fs::PermissionsExt;
        if is_root() {
            return; // root reads it anyway
        }
        let temp_dir = TempDir::new("themes-unreadable");
        let path = temp_dir.path().join("themes/locked.json");
        fs::create_dir(temp_dir.path().join("themes")).unwrap();
        fs::write(&path, "{}").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();

        let found = repository_in(temp_dir.path()).find("locked").unwrap();

        assert_eq!(found, None);
    }

    fn main_bg_of(palette: Option<ThemePalette>) -> Option<(u8, u8, u8)> {
        palette.map(|palette| palette.theme().main_bg())
    }

    #[test]
    fn should_find_a_palette_for_valid_json_that_is_not_a_theme() {
        let temp_dir = TempDir::new("palette-not-a-theme");
        let themes = temp_dir.path().join("themes");
        fs::create_dir(&themes).unwrap();
        fs::write(themes.join("empty.json"), "{}").unwrap();
        fs::write(themes.join("list.json"), "[1]").unwrap();
        fs::write(themes.join("my.theme.json"), "{}").unwrap();
        fs::write(themes.join("my.json"), r##"{"shade0":"#101010"}"##).unwrap();
        let repository = repository_in(temp_dir.path());

        let found = ["empty", "list", "my.theme"].map(|name| main_bg_of(repository.find_palette(name)));

        assert_eq!(found, [Some((24, 24, 24)); 3]);
    }

    #[test]
    fn should_find_no_palette_for_invalid_json() {
        let temp_dir = TempDir::new("palette-invalid");
        let themes = temp_dir.path().join("themes");
        fs::create_dir_all(themes.join("dir.json")).unwrap();
        fs::write(themes.join("text.json"), "not json").unwrap();
        fs::write(themes.join("blank.json"), "").unwrap();
        fs::write(themes.join("bom.json"), "\u{feff}{}").unwrap();
        fs::write(themes.join("bytes.json"), [0xff, 0xfe]).unwrap();
        let repository = repository_in(temp_dir.path());

        let found = ["text", "blank", "bom", "bytes", "dir", "missing"]
            .map(|name| repository.find_palette(name));

        assert_eq!(found, [None, None, None, None, None, None]);
    }

    #[test]
    fn should_find_no_palette_without_a_home() {
        let repository = FileThemeRepository::new(DataDirectory::new(None, None));

        let found = repository.find_palette("zed");

        assert_eq!(found, None);
    }

    #[test]
    fn should_read_the_file_again_on_every_call() {
        let temp_dir = TempDir::new("palette-reread");
        let themes = temp_dir.path().join("themes");
        fs::create_dir(&themes).unwrap();
        fs::write(themes.join("zed.json"), r##"{"shade0":"#101010"}"##).unwrap();
        let repository = repository_in(temp_dir.path());
        let first = main_bg_of(repository.find_palette("zed"));
        fs::write(themes.join("zed.json"), r##"{"shade0":"#202020"}"##).unwrap();

        let second = main_bg_of(repository.find_palette("zed"));

        assert_eq!((first, second), (Some((16, 16, 16)), Some((32, 32, 32))));
    }
}
