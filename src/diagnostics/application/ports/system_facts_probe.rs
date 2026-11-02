#[cfg(test)]
use crate::diagnostics::application::test_fixtures::{logged_path, EventLog};
use crate::diagnostics::domain::value_objects::operating_system::OperatingSystem;
use crate::diagnostics::domain::value_objects::tool_version::ToolVersion;
#[cfg(test)]
use std::cell::{Cell, RefCell};
#[cfg(test)]
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// What doctor asks the machine it runs on: the operating system, the environment, the file system, other programs.
/// Every question is asked only when the check that needs it is evaluated.
pub(crate) trait SystemFactsProbe {
    /// The program that started this process, as the process table names it.
    fn parent_executable(&self) -> Option<String>;

    fn is_on_path(&self, program: &str) -> bool;

    /// The value of an environment variable: unset and not valid UTF-8 are `None`, an empty value is kept.
    fn env_text(&self, name: &str) -> Option<String>;

    /// The variable exists, even with an empty value.
    fn env_is_set(&self, name: &str) -> bool;

    /// The variable is set to a value that is not empty (`var_os(name)` is `Some` and not empty).
    fn env_is_non_empty(&self, name: &str) -> bool;

    fn tabsmith_version(&self) -> String;

    fn operating_system(&self) -> OperatingSystem;

    /// The path is a directory, following symbolic links (`Path::is_dir`); a file of that name is not one.
    fn directory_exists(&self, path: &Path) -> bool;

    /// The path is a regular file, following symbolic links (`Path::is_file`); a directory of that name is not one.
    fn file_exists(&self, path: &Path) -> bool;

    /// The settings file of the user; `data_root/settings.json` when the data directory cannot say.
    fn settings_file(&self, data_root: &Path) -> PathBuf;

    /// The settings document store loads the file without an error.
    fn settings_file_loads(&self, path: &Path) -> bool;

    /// The version a program prints; `None` when it fails, takes too long or prints no version.
    fn tool_version(&self, program: &str, args: &[&str]) -> Option<ToolVersion>;

    /// Whether a console is attached; the Windows-only part of that question lives in the adapter.
    fn console_attached(&self) -> bool;

    /// The Windows build number; `None` off Windows. Every call asks again.
    fn windows_build(&self) -> Option<u32>;
}

/// Answers the cheap facts from its configuration (with defaults) and the expensive ones (the file system, other
/// programs, Windows) only from what a test scripted; asked about anything else it panics, which is how a test
/// asserts that doctor stays lazy. Every question is logged before it is answered.
#[cfg(test)]
pub(crate) struct InMemorySystemFactsProbe {
    parent_executable: Option<String>,
    programs_on_path: HashSet<String>,
    environment: HashMap<String, String>,
    tabsmith_version: String,
    operating_system: OperatingSystem,
    settings_file: Option<PathBuf>,
    directories: RefCell<HashMap<PathBuf, bool>>,
    files: RefCell<HashMap<PathBuf, bool>>,
    settings_files_that_load: RefCell<HashMap<PathBuf, bool>>,
    tool_versions: RefCell<HashMap<String, Option<ToolVersion>>>,
    windows_build: Cell<Option<Option<u32>>>,
    console_attached: Cell<Option<bool>>,
    log: EventLog,
}

#[cfg(test)]
impl Default for InMemorySystemFactsProbe {
    fn default() -> Self {
        InMemorySystemFactsProbe {
            parent_executable: None,
            programs_on_path: HashSet::new(),
            environment: HashMap::new(),
            tabsmith_version: "0.0.0-test".to_string(),
            operating_system: OperatingSystem::Linux,
            settings_file: None,
            directories: RefCell::default(),
            files: RefCell::default(),
            settings_files_that_load: RefCell::default(),
            tool_versions: RefCell::default(),
            windows_build: Cell::new(None),
            console_attached: Cell::new(None),
            log: EventLog::default(),
        }
    }
}

#[cfg(test)]
impl InMemorySystemFactsProbe {
    pub(crate) fn logging_to(mut self, log: EventLog) -> Self {
        self.log = log;
        self
    }

    pub(crate) fn with_parent_executable(mut self, executable: &str) -> Self {
        self.parent_executable = Some(executable.to_string());
        self
    }

    pub(crate) fn with_program_on_path(mut self, program: &str) -> Self {
        self.programs_on_path.insert(program.to_string());
        self
    }

    pub(crate) fn with_env(mut self, name: &str, value: &str) -> Self {
        self.environment.insert(name.to_string(), value.to_string());
        self
    }

    pub(crate) fn with_operating_system(mut self, operating_system: OperatingSystem) -> Self {
        self.operating_system = operating_system;
        self
    }

    pub(crate) fn with_tabsmith_version(mut self, version: &str) -> Self {
        self.tabsmith_version = version.to_string();
        self
    }

    pub(crate) fn with_settings_file(mut self, path: &str) -> Self {
        self.settings_file = Some(PathBuf::from(path));
        self
    }

    pub(crate) fn script_directory(&self, path: &str, exists: bool) {
        self.directories
            .borrow_mut()
            .insert(PathBuf::from(path), exists);
    }

    pub(crate) fn script_file(&self, path: &str, exists: bool) {
        self.files.borrow_mut().insert(PathBuf::from(path), exists);
    }

    pub(crate) fn script_settings_file_loads(&self, path: &str, loads: bool) {
        self.settings_files_that_load
            .borrow_mut()
            .insert(PathBuf::from(path), loads);
    }

    pub(crate) fn script_tool_version(
        &self,
        program: &str,
        args: &[&str],
        version: Option<ToolVersion>,
    ) {
        self.tool_versions
            .borrow_mut()
            .insert(command_line(program, args), version);
    }

    pub(crate) fn script_windows_build(&self, build: Option<u32>) {
        self.windows_build.set(Some(build));
    }

    pub(crate) fn script_console_attached(&self, attached: bool) {
        self.console_attached.set(Some(attached));
    }

    fn scripted_flag(
        &self,
        facts: &RefCell<HashMap<PathBuf, bool>>,
        question: &str,
        path: &Path,
    ) -> bool {
        self.log
            .push(format!("system.{question}({})", logged_path(path)));
        match facts.borrow().get(path) {
            Some(answer) => *answer,
            None => panic!("unscripted system.{question}({})", logged_path(path)),
        }
    }
}

#[cfg(test)]
fn command_line(program: &str, args: &[&str]) -> String {
    std::iter::once(program)
        .chain(args.iter().copied())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
impl SystemFactsProbe for InMemorySystemFactsProbe {
    fn parent_executable(&self) -> Option<String> {
        self.log.push("system.parent_executable");
        self.parent_executable.clone()
    }

    fn is_on_path(&self, program: &str) -> bool {
        self.log.push(format!("system.is_on_path({program})"));
        self.programs_on_path.contains(program)
    }

    fn env_text(&self, name: &str) -> Option<String> {
        self.log.push(format!("system.env_text({name})"));
        self.environment.get(name).cloned()
    }

    fn env_is_set(&self, name: &str) -> bool {
        self.log.push(format!("system.env_is_set({name})"));
        self.environment.contains_key(name)
    }

    fn env_is_non_empty(&self, name: &str) -> bool {
        self.log.push(format!("system.env_is_non_empty({name})"));
        self.environment
            .get(name)
            .is_some_and(|value| !value.is_empty())
    }

    fn tabsmith_version(&self) -> String {
        self.log.push("system.tabsmith_version");
        self.tabsmith_version.clone()
    }

    fn operating_system(&self) -> OperatingSystem {
        self.log.push("system.operating_system");
        self.operating_system.clone()
    }

    fn directory_exists(&self, path: &Path) -> bool {
        self.scripted_flag(&self.directories, "directory_exists", path)
    }

    fn file_exists(&self, path: &Path) -> bool {
        self.scripted_flag(&self.files, "file_exists", path)
    }

    fn settings_file(&self, data_root: &Path) -> PathBuf {
        self.log
            .push(format!("system.settings_file({})", logged_path(data_root)));
        self.settings_file
            .clone()
            .unwrap_or_else(|| data_root.join("settings.json"))
    }

    fn settings_file_loads(&self, path: &Path) -> bool {
        self.scripted_flag(&self.settings_files_that_load, "settings_file_loads", path)
    }

    fn tool_version(&self, program: &str, args: &[&str]) -> Option<ToolVersion> {
        let command = command_line(program, args);
        self.log.push(format!("system.tool_version({command})"));
        match self.tool_versions.borrow().get(&command) {
            Some(version) => version.clone(),
            None => panic!("unscripted system.tool_version({command})"),
        }
    }

    fn console_attached(&self) -> bool {
        self.log.push("system.console_attached");
        self.console_attached
            .get()
            .expect("unscripted system.console_attached")
    }

    fn windows_build(&self) -> Option<u32> {
        self.log.push("system.windows_build");
        self.windows_build
            .get()
            .expect("unscripted system.windows_build")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::application::test_fixtures::EventLog;

    fn version(text: &str) -> ToolVersion {
        ToolVersion::parse(text).unwrap()
    }

    #[test]
    fn should_know_nothing_about_the_machine_until_it_is_told() {
        let probe = InMemorySystemFactsProbe::default();

        let facts = (
            probe.parent_executable(),
            probe.is_on_path("bash"),
            probe.env_text("HOME"),
            probe.env_is_set("HOME"),
            probe.env_is_non_empty("HOME"),
            probe.operating_system(),
        );

        assert_eq!(
            facts,
            (None, false, None, false, false, OperatingSystem::Linux)
        );
    }

    #[test]
    fn should_answer_the_cheap_facts_it_was_configured_with() {
        let probe = InMemorySystemFactsProbe::default()
            .with_parent_executable("/bin/zsh")
            .with_program_on_path("fish")
            .with_env("TERM_PROGRAM", "")
            .with_env("OPENAI_API_KEY", "sk")
            .with_operating_system(OperatingSystem::Macos)
            .with_tabsmith_version("9.9.9");

        let facts = (
            probe.parent_executable(),
            probe.is_on_path("fish"),
            probe.env_text("TERM_PROGRAM"),
            probe.env_is_set("TERM_PROGRAM"),
            probe.env_is_non_empty("TERM_PROGRAM"),
            probe.env_is_non_empty("OPENAI_API_KEY"),
            probe.operating_system(),
            probe.tabsmith_version(),
        );

        assert_eq!(
            facts,
            (
                Some("/bin/zsh".to_string()),
                true,
                Some(String::new()),
                true,
                false,
                true,
                OperatingSystem::Macos,
                "9.9.9".to_string()
            )
        );
    }

    #[test]
    fn should_put_the_settings_file_in_the_data_root_unless_told_otherwise() {
        let probe = InMemorySystemFactsProbe::default();
        let elsewhere =
            InMemorySystemFactsProbe::default().with_settings_file("/etc/settings.json");

        let files = (
            probe.settings_file(Path::new("/data")),
            elsewhere.settings_file(Path::new("/data")),
        );

        assert_eq!(
            files,
            (
                PathBuf::from("/data/settings.json"),
                PathBuf::from("/etc/settings.json")
            )
        );
    }

    #[test]
    fn should_answer_the_file_system_facts_it_was_scripted_with() {
        let probe = InMemorySystemFactsProbe::default();
        probe.script_directory("/data", true);
        probe.script_file("/data/run/s.json", false);
        probe.script_settings_file_loads("/data/settings.json", true);

        let facts = (
            probe.directory_exists(Path::new("/data")),
            probe.file_exists(Path::new("/data/run/s.json")),
            probe.settings_file_loads(Path::new("/data/settings.json")),
        );

        assert_eq!(facts, (true, false, true));
    }

    #[test]
    fn should_answer_the_new_value_after_a_fact_is_scripted_again() {
        let probe = InMemorySystemFactsProbe::default();
        probe.script_directory("/data", false);
        let before = probe.directory_exists(Path::new("/data"));

        probe.script_directory("/data", true);

        assert_eq!(
            (before, probe.directory_exists(Path::new("/data"))),
            (false, true)
        );
    }

    #[test]
    fn should_answer_the_tool_version_scripted_for_the_program_and_its_arguments() {
        let probe = InMemorySystemFactsProbe::default();
        probe.script_tool_version("bash", &["--version"], Some(version("3.2.57")));
        probe.script_tool_version("sw_vers", &["-productVersion"], None);

        let versions = (
            probe.tool_version("bash", &["--version"]),
            probe.tool_version("sw_vers", &["-productVersion"]),
        );

        assert_eq!(versions, (Some(version("3.2.57")), None));
    }

    #[test]
    fn should_answer_the_windows_facts_it_was_scripted_with() {
        let probe = InMemorySystemFactsProbe::default();
        probe.script_windows_build(Some(19045));
        probe.script_console_attached(false);

        let facts = (probe.windows_build(), probe.console_attached());

        assert_eq!(facts, (Some(19045), false));
    }

    #[test]
    #[should_panic(expected = "unscripted system.directory_exists(/data)")]
    fn should_panic_when_asked_about_a_directory_it_was_not_scripted_with() {
        let probe = InMemorySystemFactsProbe::default();

        probe.directory_exists(Path::new("/data"));
    }

    #[test]
    #[should_panic(expected = "unscripted system.file_exists(/data/run/s.json)")]
    fn should_panic_when_asked_about_a_file_it_was_not_scripted_with() {
        let probe = InMemorySystemFactsProbe::default();

        probe.file_exists(Path::new("/data/run/s.json"));
    }

    #[test]
    #[should_panic(expected = "unscripted system.settings_file_loads(/data/settings.json)")]
    fn should_panic_when_asked_whether_a_settings_file_loads_it_was_not_scripted_with() {
        let probe = InMemorySystemFactsProbe::default();

        probe.settings_file_loads(Path::new("/data/settings.json"));
    }

    #[test]
    #[should_panic(expected = "unscripted system.tool_version(fish --version)")]
    fn should_panic_when_asked_for_a_tool_version_it_was_not_scripted_with() {
        let probe = InMemorySystemFactsProbe::default();

        probe.tool_version("fish", &["--version"]);
    }

    #[test]
    #[should_panic(expected = "unscripted system.windows_build")]
    fn should_panic_when_asked_for_a_windows_build_it_was_not_scripted_with() {
        let probe = InMemorySystemFactsProbe::default();

        probe.windows_build();
    }

    #[test]
    #[should_panic(expected = "unscripted system.console_attached")]
    fn should_panic_when_asked_about_the_console_it_was_not_scripted_with() {
        let probe = InMemorySystemFactsProbe::default();

        probe.console_attached();
    }

    #[test]
    fn should_log_each_system_question_in_order() {
        let log = EventLog::default();
        let probe = InMemorySystemFactsProbe::default().logging_to(log.clone());
        probe.script_directory("/data", true);
        probe.script_tool_version("bash", &["--version"], None);

        probe.is_on_path("bash");
        probe.directory_exists(Path::new("/data"));
        probe.tool_version("bash", &["--version"]);
        probe.env_is_set("HOME");

        assert_eq!(
            log.entries(),
            [
                "system.is_on_path(bash)",
                "system.directory_exists(/data)",
                "system.tool_version(bash --version)",
                "system.env_is_set(HOME)",
            ]
        );
    }

    #[test]
    fn should_log_the_questions_it_answers_from_its_configuration() {
        let log = EventLog::default();
        let probe = InMemorySystemFactsProbe::default().logging_to(log.clone());

        probe.parent_executable();
        probe.env_text("TERM");
        probe.env_is_non_empty("KEY");
        probe.tabsmith_version();
        probe.operating_system();
        probe.settings_file(Path::new("/data"));

        assert_eq!(
            log.entries(),
            [
                "system.parent_executable",
                "system.env_text(TERM)",
                "system.env_is_non_empty(KEY)",
                "system.tabsmith_version",
                "system.operating_system",
                "system.settings_file(/data)",
            ]
        );
    }
}
