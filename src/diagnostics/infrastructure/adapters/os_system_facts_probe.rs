use crate::diagnostics::application::ports::system_facts_probe::SystemFactsProbe;
use crate::diagnostics::domain::value_objects::operating_system::OperatingSystem;
use crate::diagnostics::domain::value_objects::tool_version::ToolVersion;
use crate::shared::domain::value_objects::data_directory::DataDirectory;
use crate::shared::infrastructure::json_document_store::JsonDocumentStore;
use crate::shared::infrastructure::os::executable_lookup::which;
use crate::shared::infrastructure::os::process_info;
use crate::shared::infrastructure::os::process_runner::output_with_timeout;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// Answers doctor's questions from the machine itself: process table, environment, file system, other programs.
pub(crate) struct OsSystemFactsProbe {
    data_directory: DataDirectory,
}

impl OsSystemFactsProbe {
    pub(crate) fn new(data_directory: DataDirectory) -> Self {
        OsSystemFactsProbe { data_directory }
    }
}

impl SystemFactsProbe for OsSystemFactsProbe {
    fn parent_executable(&self) -> Option<String> {
        process_info::parent_exe()
    }

    fn is_on_path(&self, program: &str) -> bool {
        which(program).is_some()
    }

    fn env_text(&self, name: &str) -> Option<String> {
        std::env::var(name).ok()
    }

    fn env_is_set(&self, name: &str) -> bool {
        std::env::var_os(name).is_some()
    }

    fn env_is_non_empty(&self, name: &str) -> bool {
        std::env::var_os(name).is_some_and(|value| !value.is_empty())
    }

    fn tabsmith_version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    fn operating_system(&self) -> OperatingSystem {
        OperatingSystem::from_name(std::env::consts::OS)
    }

    fn directory_exists(&self, path: &Path) -> bool {
        path.is_dir()
    }

    fn file_exists(&self, path: &Path) -> bool {
        path.is_file()
    }

    fn settings_file(&self, data_root: &Path) -> PathBuf {
        self.data_directory
            .settings_file()
            .unwrap_or_else(|_| data_root.join("settings.json"))
    }

    fn settings_file_loads(&self, path: &Path) -> bool {
        JsonDocumentStore::load(path).is_ok()
    }

    fn tool_version(&self, program: &str, args: &[&str]) -> Option<ToolVersion> {
        version_from(program, args)
    }

    #[cfg(windows)]
    fn console_attached(&self) -> bool {
        use std::os::windows::io::AsRawHandle;
        process_info::is_console(std::io::stdin().as_raw_handle())
            && process_info::is_console(std::io::stdout().as_raw_handle())
    }

    #[cfg(not(windows))]
    fn console_attached(&self) -> bool {
        true
    }

    #[cfg(windows)]
    fn windows_build(&self) -> Option<u32> {
        // ponytail: GetVersionExW lies without an app manifest and RtlGetVersion needs another windows-sys feature.
        version_from("cmd", &["/C", "ver"])?.components().get(2).copied()
    }

    #[cfg(not(windows))]
    fn windows_build(&self) -> Option<u32> {
        None
    }
}

/// Version printed by `prog args` (5 s timeout).
fn version_from(prog: &str, args: &[&str]) -> Option<ToolVersion> {
    let printed =
        output_with_timeout(Command::new(prog).args(args), Duration::from_secs(5)).ok()?;
    ToolVersion::parse(&printed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::application::ports::system_facts_probe::SystemFactsProbe;
    use crate::diagnostics::domain::value_objects::operating_system::OperatingSystem;
    use crate::shared::infrastructure::testing::temp_dir::TempDir;
    use std::path::{Path, PathBuf};

    const UNSET_VARIABLE: &str = "TABSMITH_DOCTOR_PROBE_CERTAINLY_UNSET_VARIABLE";

    fn probe() -> OsSystemFactsProbe {
        OsSystemFactsProbe::new(DataDirectory::new(None, None))
    }

    fn probe_with(root: Option<PathBuf>, settings: Option<PathBuf>) -> OsSystemFactsProbe {
        OsSystemFactsProbe::new(DataDirectory::new(root, settings))
    }

    fn loads(content: &str) -> bool {
        let temp_dir = TempDir::new("probe-settings-loads");
        let path = temp_dir.path().join("settings.json");
        std::fs::write(&path, content).unwrap();
        probe().settings_file_loads(&path)
    }

    #[test]
    fn should_see_a_directory_as_a_directory_and_not_as_a_file() {
        let temp_dir = TempDir::new("probe-directory");
        let sub = temp_dir.path().join("abc.json");
        std::fs::create_dir(&sub).unwrap();

        let answers = (probe().directory_exists(&sub), probe().file_exists(&sub));

        assert_eq!(answers, (true, false));
    }

    #[test]
    fn should_see_a_regular_file_as_a_file_and_not_as_a_directory() {
        let temp_dir = TempDir::new("probe-file");
        let data_root = temp_dir.path().join("data");
        std::fs::write(&data_root, "x").unwrap();

        let answers = (
            probe().file_exists(&data_root),
            probe().directory_exists(&data_root),
        );

        assert_eq!(answers, (true, false));
    }

    #[test]
    fn should_see_nothing_at_a_missing_path() {
        let temp_dir = TempDir::new("probe-missing");
        let missing = temp_dir.path().join("missing");

        let answers = (
            probe().file_exists(&missing),
            probe().directory_exists(&missing),
        );

        assert_eq!(answers, (false, false));
    }

    #[test]
    fn should_use_the_settings_override_of_the_data_directory() {
        let elsewhere = PathBuf::from("/elsewhere/prefs.json");
        let probe = probe_with(Some(PathBuf::from("/data")), Some(elsewhere.clone()));

        let file = probe.settings_file(Path::new("/data"));

        assert_eq!(file, elsewhere);
    }

    #[test]
    fn should_put_the_settings_file_in_the_data_root_when_the_data_directory_has_no_root() {
        let file = probe().settings_file(Path::new("/data"));

        assert_eq!(file, Path::new("/data").join("settings.json"));
    }

    #[test]
    fn should_load_a_settings_file_that_does_not_exist() {
        let temp_dir = TempDir::new("probe-settings-missing");

        let answer = probe().settings_file_loads(&temp_dir.path().join("settings.json"));

        assert!(answer);
    }

    #[test]
    fn should_load_a_blank_settings_file() {
        assert!(loads("  \n"));
    }

    #[test]
    fn should_load_a_settings_file_holding_an_object() {
        assert!(loads(r#"{"theme": "dark"}"#));
    }

    #[test]
    fn should_not_load_a_settings_file_with_invalid_json() {
        assert!(!loads("{oops"));
    }

    #[test]
    fn should_not_load_a_settings_file_holding_an_array() {
        assert!(!loads("[1, 2]"));
    }

    #[test]
    fn should_not_load_a_settings_path_that_is_a_directory() {
        let temp_dir = TempDir::new("probe-settings-directory");
        let path = temp_dir.path().join("settings.json");
        std::fs::create_dir(&path).unwrap();

        assert!(!probe().settings_file_loads(&path));
    }

    #[test]
    fn should_find_no_version_for_a_program_that_does_not_exist() {
        let version = probe().tool_version("tabsmith-no-such-program-anywhere", &["--version"]);

        assert_eq!(version, None);
    }

    #[test]
    fn should_name_the_operating_system_this_binary_runs_on() {
        let system = probe().operating_system();

        assert_eq!(system, OperatingSystem::from_name(std::env::consts::OS));
    }

    #[test]
    fn should_report_the_crate_version_as_the_tabsmith_version() {
        let version = probe().tabsmith_version();

        assert_eq!(version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn should_find_an_unset_variable_neither_set_nor_non_empty_nor_text() {
        let answers = (
            probe().env_text(UNSET_VARIABLE),
            probe().env_is_set(UNSET_VARIABLE),
            probe().env_is_non_empty(UNSET_VARIABLE),
        );

        assert_eq!(answers, (None, false, false));
    }

    #[test]
    fn should_not_find_a_program_that_does_not_exist_on_the_path() {
        assert!(!probe().is_on_path("tabsmith-no-such-program-anywhere"));
    }
}
