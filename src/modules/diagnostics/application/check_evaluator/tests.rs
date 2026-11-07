use crate::modules::diagnostics::application::check_evaluator::CheckEvaluator;
use crate::modules::diagnostics::application::ports::bundled_specs_probe::InMemoryBundledSpecsProbe;
use crate::modules::diagnostics::application::ports::integration_probe::InMemoryIntegrationProbe;
use crate::modules::diagnostics::application::ports::system_facts_probe::InMemorySystemFactsProbe;
use crate::modules::diagnostics::application::test_fixtures::{sample_paths, EventLog};
use crate::modules::diagnostics::domain::value_objects::check::Check;
use crate::modules::diagnostics::domain::value_objects::check_fix::CheckFix;
use crate::modules::diagnostics::domain::value_objects::check_outcome::CheckOutcome;
use crate::modules::diagnostics::domain::value_objects::check_severity::CheckSeverity;
use crate::modules::diagnostics::domain::value_objects::check_subject::CheckSubject;
use crate::modules::diagnostics::domain::value_objects::environment_check::EnvironmentCheck;
use crate::modules::diagnostics::domain::value_objects::operating_system::OperatingSystem;
use crate::modules::diagnostics::domain::value_objects::system_check::SystemCheck;
use crate::modules::diagnostics::domain::value_objects::system_facts::SystemFacts;
use crate::modules::diagnostics::domain::value_objects::terminal_environment::{
    TerminalEnvironment, TerminalEnvironmentParts,
};
use crate::modules::diagnostics::domain::value_objects::tool_version::ToolVersion;
use crate::modules::settings::domain::services::settings_catalog::{
    AUTOCOMPLETE_DEVELOPER_MODE, AUTOCOMPLETE_DISABLE,
};
use crate::modules::settings::domain::value_objects::setting_value::SettingValue;
use crate::modules::settings::domain::value_objects::settings::Settings;
use crate::modules::settings::infrastructure::adapters::setting_value_json::settings_from_json;
use crate::modules::shared::domain::value_objects::shell::Shell;
use crate::modules::shell_integration::domain::entities::integration::Integration;
use crate::modules::shell_integration::domain::shell_integration_error::ShellIntegrationError;
use serde_json::json;
use std::path::PathBuf;
use std::rc::Rc;

const DATA_ROOT: &str = "/home/u/.tabsmith";
const SETTINGS_FILE: &str = "/home/u/.tabsmith/settings.json";

struct World {
    log: EventLog,
    system: Rc<InMemorySystemFactsProbe>,
    integrations: Rc<InMemoryIntegrationProbe>,
    specs: Rc<InMemoryBundledSpecsProbe>,
}

impl World {
    fn new() -> Self {
        let log = EventLog::default();
        World {
            system: Rc::new(InMemorySystemFactsProbe::default().logging_to(log.clone())),
            integrations: Rc::new(InMemoryIntegrationProbe::default().logging_to(log.clone())),
            specs: Rc::new(InMemoryBundledSpecsProbe::extracted(true).logging_to(log.clone())),
            log,
        }
    }

    fn with_system(mut self, system: InMemorySystemFactsProbe) -> Self {
        self.system = Rc::new(system.logging_to(self.log.clone()));
        self
    }

    fn with_integrations(mut self, integrations: InMemoryIntegrationProbe) -> Self {
        self.integrations = Rc::new(integrations.logging_to(self.log.clone()));
        self
    }

    fn with_specs_extracted(mut self, extracted: bool) -> Self {
        self.specs = Rc::new(
            InMemoryBundledSpecsProbe::extracted(extracted).logging_to(self.log.clone()),
        );
        self
    }

    fn evaluate(&self, subject: CheckSubject) -> CheckOutcome {
        let check = Check::new("check".to_string(), CheckSeverity::Normal, subject);
        self.evaluator().evaluate(&check, &sample_paths())
    }

    fn evaluator(&self) -> CheckEvaluator {
        CheckEvaluator::new(
            self.system.clone(),
            self.integrations.clone(),
            self.specs.clone(),
        )
    }
}

fn bashrc() -> Integration {
    Integration::dotfile(Shell::Bash, PathBuf::from("/home/u/.bashrc"))
}

fn version(text: &str) -> Option<ToolVersion> {
    ToolVersion::parse(text)
}

fn facts_of(session_id: Option<&str>, settings: Settings) -> Rc<SystemFacts> {
    let terminal = TerminalEnvironment::new(TerminalEnvironmentParts::default());
    Rc::new(SystemFacts::new(
        terminal,
        session_id.map(String::from),
        settings,
    ))
}

fn environment(which: EnvironmentCheck, facts: Rc<SystemFacts>) -> CheckSubject {
    CheckSubject::Environment(which, facts)
}

fn remote_profile_settings() -> Settings {
    settings_from_json(
        json!({
            "inline.source": "cloud",
            "inline.profiles.cloud": { "provider": "openai", "model": "gpt" }
        })
        .as_object()
        .unwrap()
        .clone(),
    )
}

fn local_profile_settings() -> Settings {
    settings_from_json(
        json!({
            "inline.source": "mine",
            "inline.profiles.mine": {
                "provider": "openai",
                "model": "llama",
                "base_url": "http://localhost:11434/v1"
            }
        })
        .as_object()
        .unwrap()
        .clone(),
    )
}

fn fix_of(outcome: CheckOutcome) -> Option<CheckFix> {
    match outcome {
        CheckOutcome::Err { fix, .. } => fix,
        other => panic!("expected an error, got {other:?}"),
    }
}

fn reason_of(outcome: CheckOutcome) -> String {
    match outcome {
        CheckOutcome::Err { reason, .. } | CheckOutcome::Warn(reason) => reason,
        CheckOutcome::Ok => panic!("expected a failure, got Ok"),
    }
}

fn no_questions() -> Vec<String> {
    Vec::new()
}

#[test]
fn should_offer_to_install_a_dotfile_integration_that_is_not_installed() {
    let world = World::new().with_integrations(
        InMemoryIntegrationProbe::default().checking(bashrc(), || {
            Err(ShellIntegrationError::NotInstalled("not installed".into()))
        }),
    );

    let outcome = world.evaluate(CheckSubject::Dotfile(bashrc()));

    assert_eq!(
        (fix_of(outcome), world.log.entries()),
        (
            Some(CheckFix::InstallIntegration(bashrc())),
            vec!["integrations.check(bash pre post into .bashrc)".to_string()]
        )
    );
}

#[test]
fn should_pass_a_dotfile_integration_that_checks_out() {
    let world = World::new()
        .with_integrations(InMemoryIntegrationProbe::default().checking(bashrc(), || Ok(())));

    let outcome = world.evaluate(CheckSubject::Dotfile(bashrc()));

    assert_eq!(outcome, CheckOutcome::Ok);
}

#[test]
fn should_fail_a_listing_failure_with_its_text_and_ask_no_probe() {
    let world = World::new();

    let outcome = world.evaluate(CheckSubject::ListingFailure("could not list".into()));

    assert_eq!(
        (outcome, world.log.entries()),
        (CheckOutcome::err("could not list"), no_questions())
    );
}

#[test]
fn should_offer_to_create_the_data_directory_the_probe_says_is_missing() {
    let world = World::new();
    world.system.script_directory(DATA_ROOT, false);

    let outcome = world.evaluate(CheckSubject::DataDirectory);

    assert_eq!(
        (fix_of(outcome), world.log.entries()),
        (
            Some(CheckFix::CreateDataDirectory(PathBuf::from(DATA_ROOT))),
            vec!["system.directory_exists(/home/u/.tabsmith)".to_string()]
        )
    );
}

#[test]
fn should_pass_an_existing_data_directory() {
    let world = World::new();
    world.system.script_directory(DATA_ROOT, true);

    let outcome = world.evaluate(CheckSubject::DataDirectory);

    assert_eq!(outcome, CheckOutcome::Ok);
}

#[test]
fn should_offer_to_install_the_missing_integrations_when_a_script_points_at_another_binary() {
    let world = World::new().with_integrations(
        InMemoryIntegrationProbe::default().with_script_binaries(vec![
            (Shell::Bash, "/usr/local/bin/tabsmith".to_string()),
            (Shell::Zsh, "/old/tabsmith".to_string()),
        ]),
    );

    let outcome = world.evaluate(CheckSubject::ScriptsPointAtBinary);

    assert_eq!(
        (fix_of(outcome), world.log.entries()),
        (
            Some(CheckFix::InstallMissingIntegrations(vec![Shell::Zsh])),
            vec!["integrations.script_binaries".to_string()]
        )
    );
}

#[test]
fn should_pass_scripts_that_point_at_the_paths_binary() {
    let world = World::new().with_integrations(
        InMemoryIntegrationProbe::default()
            .with_script_binaries(vec![(Shell::Bash, "/usr/local/bin/tabsmith".to_string())]),
    );

    let outcome = world.evaluate(CheckSubject::ScriptsPointAtBinary);

    assert_eq!(outcome, CheckOutcome::Ok);
}

#[test]
fn should_offer_to_reset_a_settings_file_that_does_not_load() {
    let world = World::new();
    world.system.script_settings_file_loads(SETTINGS_FILE, false);

    let outcome = world.evaluate(CheckSubject::SettingsCorruption(PathBuf::from(SETTINGS_FILE)));

    assert_eq!(
        (fix_of(outcome), world.log.entries()),
        (
            Some(CheckFix::ResetSettings(PathBuf::from(SETTINGS_FILE))),
            vec!["system.settings_file_loads(/home/u/.tabsmith/settings.json)".to_string()]
        )
    );
}

#[test]
fn should_pass_a_settings_file_that_loads() {
    let world = World::new();
    world.system.script_settings_file_loads(SETTINGS_FILE, true);

    let outcome = world.evaluate(CheckSubject::SettingsCorruption(PathBuf::from(SETTINGS_FILE)));

    assert_eq!(outcome, CheckOutcome::Ok);
}

#[test]
fn should_offer_to_install_the_spec_bundle_when_it_is_not_extracted() {
    let world = World::new().with_specs_extracted(false);

    let outcome = world.evaluate(CheckSubject::SpecsExtracted);

    assert_eq!(
        (fix_of(outcome), world.log.entries()),
        (
            Some(CheckFix::InstallSpecBundle),
            vec!["bundled_specs.is_extracted(/home/u/.tabsmith)".to_string()]
        )
    );
}

#[test]
fn should_pass_an_extracted_spec_bundle() {
    let world = World::new().with_specs_extracted(true);

    let outcome = world.evaluate(CheckSubject::SpecsExtracted);

    assert_eq!(outcome, CheckOutcome::Ok);
}

#[test]
fn should_fail_the_windows_console_check_when_no_console_is_attached() {
    let world = World::new();
    world.system.script_console_attached(false);

    let outcome = world.evaluate(CheckSubject::WindowsConsole);

    assert_eq!(
        (reason_of(outcome), world.log.entries()),
        (
            "Windows Console APIs are not supported in this terminal".to_string(),
            vec!["system.console_attached".to_string()]
        )
    );
}

#[test]
fn should_pass_the_windows_console_check_when_a_console_is_attached() {
    let world = World::new();
    world.system.script_console_attached(true);

    let outcome = world.evaluate(CheckSubject::WindowsConsole);

    assert_eq!(outcome, CheckOutcome::Ok);
}

#[test]
fn should_fail_the_windows_version_check_below_the_conpty_build() {
    let world = World::new();
    world.system.script_windows_build(Some(17134));

    let outcome = world.evaluate(CheckSubject::WindowsVersion);

    assert_eq!(
        (reason_of(outcome), world.log.entries()),
        (
            "Windows 10 1809 or newer is required (ConPTY)".to_string(),
            vec!["system.windows_build".to_string()]
        )
    );
}

#[test]
fn should_pass_the_windows_version_check_from_the_conpty_build() {
    let world = World::new();
    world.system.script_windows_build(Some(17763));

    let outcome = world.evaluate(CheckSubject::WindowsVersion);

    assert_eq!(outcome, CheckOutcome::Ok);
}

#[test]
fn should_fail_the_terminal_check_for_an_unsupported_terminal_and_ask_no_probe() {
    let world = World::new();
    let terminal = TerminalEnvironment::new(TerminalEnvironmentParts {
        term_program: Some("WarpTerminal".to_string()),
        ..TerminalEnvironmentParts::default()
    });
    let facts = Rc::new(SystemFacts::new(terminal, None, Settings::default()));

    let outcome = world.evaluate(environment(EnvironmentCheck::TerminalIntegrations, facts));

    assert_eq!(
        (reason_of(outcome), world.log.entries()),
        ("WarpTerminal is not supported".to_string(), no_questions())
    );
}

#[test]
fn should_fail_the_autocomplete_check_when_autocomplete_is_disabled_and_ask_no_probe() {
    let world = World::new();
    let settings = Settings::default().with(AUTOCOMPLETE_DISABLE, SettingValue::Bool(true));

    let outcome = world.evaluate(environment(
        EnvironmentCheck::AutocompleteEnabled,
        facts_of(None, settings),
    ));

    assert_eq!(
        (reason_of(outcome), world.log.entries()),
        ("Autocomplete disabled.".to_string(), no_questions())
    );
}

#[test]
fn should_warn_about_developer_mode_when_it_is_enabled_and_ask_no_probe() {
    let world = World::new();
    let settings =
        Settings::default().with(AUTOCOMPLETE_DEVELOPER_MODE, SettingValue::Bool(true));

    let outcome = world.evaluate(environment(
        EnvironmentCheck::DeveloperMode,
        facts_of(None, settings),
    ));

    assert_eq!(
        (reason_of(outcome), world.log.entries()),
        (
            "autocomplete.developerMode is enabled".to_string(),
            no_questions()
        )
    );
}

#[test]
fn should_warn_when_the_run_file_of_the_session_does_not_exist() {
    let world = World::new();
    world.system.script_file("/home/u/.tabsmith/run/s-1.json", false);

    let outcome = world.evaluate(environment(
        EnvironmentCheck::Session,
        facts_of(Some("s-1"), Settings::default()),
    ));

    assert_eq!(
        (reason_of(outcome), world.log.entries()),
        (
            "Shell has not reported its environment yet (tabsmith _ pre-cmd did not run)"
                .to_string(),
            vec!["system.file_exists(/home/u/.tabsmith/run/s-1.json)".to_string()]
        )
    );
}

#[test]
fn should_pass_the_session_check_when_the_probe_finds_the_run_file() {
    let world = World::new();
    world.system.script_file("/home/u/.tabsmith/run/s-1.json", true);

    let outcome = world.evaluate(environment(
        EnvironmentCheck::Session,
        facts_of(Some("s-1"), Settings::default()),
    ));

    assert_eq!(outcome, CheckOutcome::Ok);
}

#[test]
fn should_look_for_a_nameless_run_file_when_the_session_id_is_empty() {
    let world = World::new();
    world.system.script_file("/home/u/.tabsmith/run/.json", true);

    world.evaluate(environment(
        EnvironmentCheck::Session,
        facts_of(Some(""), Settings::default()),
    ));

    assert_eq!(
        world.log.entries(),
        vec!["system.file_exists(/home/u/.tabsmith/run/.json)".to_string()]
    );
}

#[test]
fn should_look_for_a_nameless_run_file_when_there_is_no_session_id() {
    let world = World::new();
    world.system.script_file("/home/u/.tabsmith/run/.json", true);

    world.evaluate(environment(
        EnvironmentCheck::Session,
        facts_of(None, Settings::default()),
    ));

    assert_eq!(
        world.log.entries(),
        vec!["system.file_exists(/home/u/.tabsmith/run/.json)".to_string()]
    );
}

#[test]
fn should_ask_the_environment_for_the_key_of_a_remote_inline_profile() {
    let world = World::new();

    let outcome = world.evaluate(environment(
        EnvironmentCheck::Inline,
        facts_of(None, remote_profile_settings()),
    ));

    assert_eq!(
        (reason_of(outcome), world.log.entries()),
        (
            "OPENAI_API_KEY is not set".to_string(),
            vec!["system.env_is_non_empty(OPENAI_API_KEY)".to_string()]
        )
    );
}

#[test]
fn should_pass_a_remote_inline_profile_whose_key_is_set() {
    let world =
        World::new().with_system(InMemorySystemFactsProbe::default().with_env("OPENAI_API_KEY", "sk"));

    let outcome = world.evaluate(environment(
        EnvironmentCheck::Inline,
        facts_of(None, remote_profile_settings()),
    ));

    assert_eq!(outcome, CheckOutcome::Ok);
}

#[test]
fn should_not_ask_the_environment_for_a_local_inline_profile() {
    let world = World::new();

    let outcome = world.evaluate(environment(
        EnvironmentCheck::Inline,
        facts_of(None, local_profile_settings()),
    ));

    assert_eq!((outcome, world.log.entries()), (CheckOutcome::Ok, no_questions()));
}

#[test]
fn should_not_ask_the_environment_for_the_default_inline_source() {
    let world = World::new();

    let outcome = world.evaluate(environment(
        EnvironmentCheck::Inline,
        facts_of(None, Settings::default()),
    ));

    assert_eq!((outcome, world.log.entries()), (CheckOutcome::Ok, no_questions()));
}

#[test]
fn should_ask_no_version_of_a_linux_system() {
    let world = World::new();

    let outcome = world.evaluate(CheckSubject::System(SystemCheck::Os));

    assert_eq!(
        (outcome, world.log.entries()),
        (CheckOutcome::Ok, vec!["system.operating_system".to_string()])
    );
}

#[test]
fn should_ask_sw_vers_once_on_macos() {
    let system = InMemorySystemFactsProbe::default().with_operating_system(OperatingSystem::Macos);
    let world = World::new().with_system(system);
    world
        .system
        .script_tool_version("sw_vers", &["-productVersion"], version("10.15.7"));

    let outcome = world.evaluate(CheckSubject::System(SystemCheck::Os));

    assert_eq!(
        (reason_of(outcome), world.log.entries()),
        (
            "macOS 10.15.7 is not supported (needs macOS 11 or newer)".to_string(),
            vec![
                "system.operating_system".to_string(),
                "system.tool_version(sw_vers -productVersion)".to_string()
            ]
        )
    );
}

#[test]
fn should_ask_the_windows_build_once_on_windows() {
    let system =
        InMemorySystemFactsProbe::default().with_operating_system(OperatingSystem::Windows);
    let world = World::new().with_system(system);
    world.system.script_windows_build(Some(10240));

    let outcome = world.evaluate(CheckSubject::System(SystemCheck::Os));

    assert_eq!(
        (reason_of(outcome), world.log.entries()),
        (
            "Windows 10 1809 or newer is required (ConPTY)".to_string(),
            vec![
                "system.operating_system".to_string(),
                "system.windows_build".to_string()
            ]
        )
    );
}

#[test]
fn should_ask_nothing_more_about_an_unknown_operating_system() {
    let system = InMemorySystemFactsProbe::default()
        .with_operating_system(OperatingSystem::Other("freebsd".to_string()));
    let world = World::new().with_system(system);

    let outcome = world.evaluate(CheckSubject::System(SystemCheck::Os));

    assert_eq!(
        (reason_of(outcome), world.log.entries()),
        (
            "freebsd is not supported".to_string(),
            vec!["system.operating_system".to_string()]
        )
    );
}

#[test]
fn should_ask_the_program_of_the_bash_check_for_its_version() {
    let world = World::new();
    world
        .system
        .script_tool_version("/opt/bash", &["--version"], version("3.2.57"));

    let outcome = world.evaluate(CheckSubject::System(SystemCheck::Bash {
        program: "/opt/bash".to_string(),
    }));

    assert_eq!(
        (reason_of(outcome), world.log.entries()),
        (
            "Bash 3.2 is not supported (needs >= 4.4 for PS0); install a newer bash or switch to zsh"
                .to_string(),
            vec!["system.tool_version(/opt/bash --version)".to_string()]
        )
    );
}

#[test]
fn should_ask_the_fish_on_path_for_its_version() {
    let world = World::new();
    world
        .system
        .script_tool_version("fish", &["--version"], version("3.1.0"));

    let outcome = world.evaluate(CheckSubject::System(SystemCheck::Fish));

    assert_eq!(
        (reason_of(outcome), world.log.entries()),
        (
            "your fish version is outdated (need at least 3.3.0, found 3.1.0)".to_string(),
            vec!["system.tool_version(fish --version)".to_string()]
        )
    );
}

#[test]
fn should_ask_the_probe_again_each_time_the_same_check_is_evaluated() {
    let world = World::new();
    world.system.script_directory(DATA_ROOT, false);
    let check = Check::new(
        "data".to_string(),
        CheckSeverity::Normal,
        CheckSubject::DataDirectory,
    );
    let evaluator = world.evaluator();
    let first = evaluator.evaluate(&check, &sample_paths());
    world.system.script_directory(DATA_ROOT, true);

    let second = evaluator.evaluate(&check, &sample_paths());

    assert_eq!(
        (first.is_err(), second, world.log.entries().len()),
        (true, CheckOutcome::Ok, 2)
    );
}
