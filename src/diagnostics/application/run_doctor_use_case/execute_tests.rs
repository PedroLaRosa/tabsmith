use crate::diagnostics::application::doctor_request::DoctorRequest;
use crate::diagnostics::application::doctor_verdict::DoctorVerdict;
use crate::diagnostics::application::ports::check_fixer::RecordingCheckFixer;
use crate::diagnostics::application::ports::integration_probe::InMemoryIntegrationProbe;
use crate::diagnostics::application::ports::system_facts_probe::InMemorySystemFactsProbe;
use crate::diagnostics::application::test_fixtures::{sample_paths, DoctorHarness, LoggingRestartReminder};
use crate::diagnostics::domain::diagnostics_error::DiagnosticsError;
use crate::diagnostics::domain::value_objects::operating_system::OperatingSystem;
use crate::diagnostics::domain::value_objects::tool_version::ToolVersion;
use crate::shared::domain::value_objects::shell::Shell;
use crate::shell_integration::domain::entities::integration::Integration;
use crate::shell_integration::domain::repositories::restart_reminder_repository::RestartReminderRepository;
use crate::shell_integration::domain::shell_integration_error::ShellIntegrationError;
use std::path::PathBuf;

const SETTINGS_FILE: &str = "/home/u/.tabsmith/settings.json";
const DATA_ROOT: &str = "/home/u/.tabsmith";
const SESSION_RUN_FILE: &str = "/home/u/.tabsmith/run/abc.json";

fn bashrc() -> Integration {
    Integration::dotfile(Shell::Bash, PathBuf::from("/home/u/.bashrc"))
}

fn event(name: &str) -> String {
    format!("reporter.{name}")
}

fn events(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| name.to_string()).collect()
}

fn located_integrations() -> InMemoryIntegrationProbe {
    InMemoryIntegrationProbe::default().locating(sample_paths()).with_script_binaries(Vec::new())
}

fn healthy_system() -> InMemorySystemFactsProbe {
    let system = InMemorySystemFactsProbe::default().with_env("TABSMITH_TERM", "0.0.0-test");
    system.script_directory(DATA_ROOT, true);
    system.script_settings_file_loads(SETTINGS_FILE, true);
    system
}

fn healthy() -> DoctorHarness {
    DoctorHarness::new().with_system(healthy_system()).with_integrations(located_integrations())
}

fn healthy_with_bash_running() -> DoctorHarness {
    let integrations = located_integrations().listing(Shell::Bash, vec![bashrc()]).checking(bashrc(), || Ok(()));
    DoctorHarness::new()
        .with_system(healthy_system().with_parent_executable("bash"))
        .with_integrations(integrations)
}

fn with_a_broken_dotfile_that_cannot_be_repaired() -> DoctorHarness {
    let broken = || Err(ShellIntegrationError::NotInstalled("no block".to_string()));
    let integrations = located_integrations().listing(Shell::Bash, vec![bashrc()]).checking(bashrc(), broken);
    DoctorHarness::new()
        .with_system(healthy_system().with_parent_executable("bash"))
        .with_integrations(integrations)
        .with_fixer(RecordingCheckFixer::failing("read-only"))
}

fn healthy_with_a_warning_in_the_environment_group() -> DoctorHarness {
    let system = healthy_system().with_env("TABSMITH_SESSION_ID", "abc");
    system.script_file(SESSION_RUN_FILE, false);
    DoctorHarness::new().with_system(system).with_integrations(located_integrations())
}

fn without_the_data_directory() -> DoctorHarness {
    let system = healthy_system();
    system.script_directory(DATA_ROOT, false);
    DoctorHarness::new().with_system(system).with_integrations(located_integrations())
}

fn without_the_data_directory_and_unable_to_create_it() -> DoctorHarness {
    without_the_data_directory().with_fixer(RecordingCheckFixer::failing("no space"))
}

fn without_the_data_directory_and_saving_a_reminder_when_creating_it() -> DoctorHarness {
    let harness = without_the_data_directory();
    let system = harness.system.clone();
    let reminder = LoggingRestartReminder::new(harness.reminder.clone(), harness.log.clone());
    let creating = RecordingCheckFixer::succeeding().with_effect(move || {
        system.script_directory(DATA_ROOT, true);
        reminder.save_pending();
    });
    harness.with_fixer(creating)
}

fn on_freebsd() -> DoctorHarness {
    DoctorHarness::new()
        .with_system(healthy_system().with_operating_system(OperatingSystem::Other("freebsd".to_string())))
        .with_integrations(located_integrations())
}

fn with_an_old_bash_running_and_fish_installed() -> DoctorHarness {
    let integrations = located_integrations()
        .listing(Shell::Bash, vec![bashrc()])
        .checking(bashrc(), || Ok(()))
        .listing(Shell::Fish, Vec::new());
    let system = healthy_system().with_parent_executable("bash").with_program_on_path("fish");
    system.script_tool_version("bash", &["--version"], ToolVersion::parse("4.0"));
    DoctorHarness::new().with_system(system).with_integrations(integrations)
}

fn execute(harness: &DoctorHarness, all: bool, strict: bool) -> DoctorVerdict {
    harness.use_case().execute(DoctorRequest::new(all, strict)).expect("the paths are located")
}

fn tail(harness: &DoctorHarness, count: usize) -> Vec<String> {
    let log = harness.log.entries();
    log[log.len() - count..].to_vec()
}

fn position(log: &[String], entry: &str) -> usize {
    log.iter().position(|logged| logged == entry).unwrap_or_else(|| panic!("{entry} is not in {log:?}"))
}

fn summary_after_the_last_stop_of_the_progress(harness: &DoctorHarness) -> Vec<String> {
    let events = harness.reporter_events();
    let last_stop = events.iter().rposition(|logged| logged == &event("stop_progress")).expect("the progress was stopped");
    events[last_stop + 1..].to_vec()
}

#[test]
fn should_run_a_whole_quiet_run_in_the_order_of_the_design() {
    let harness = healthy();

    execute(&harness, false, false);

    assert_eq!(
        harness.log.entries(),
        events(&[
            "integrations.locate_paths",
            "system.parent_executable",
            "reporter.start_progress",
            "system.is_on_path(bash)",
            "system.is_on_path(zsh)",
            "system.is_on_path(fish)",
            "system.is_on_path(pwsh)",
            "system.settings_file(/home/u/.tabsmith)",
            "system.operating_system",
            "system.directory_exists(/home/u/.tabsmith)",
            "integrations.script_binaries",
            "system.settings_file_loads(/home/u/.tabsmith/settings.json)",
            "bundled_specs.is_extracted(/home/u/.tabsmith)",
            "system.env_text(TERM_PROGRAM)",
            "system.env_text(__CFBundleIdentifier)",
            "system.env_text(TABSMITH_TERM)",
            "system.env_is_set(INSIDE_EMACS)",
            "system.env_is_set(PROCESS_LAUNCHED_BY_TABSMITH)",
            "system.env_is_set(CI)",
            "system.tabsmith_version",
            "system.env_text(TABSMITH_SESSION_ID)",
            "system.env_text(TABSMITH_SHELL)",
            "settings.load",
            "system.is_on_path(fish)",
            "system.operating_system",
            "reporter.stop_progress",
            "reporter.everything_looks_good",
            "reporter.blank_line",
            "reporter.support_hint",
            "reporter.blank_line",
            "reminder.is_pending",
        ])
    );
}

#[test]
fn should_build_each_group_before_printing_its_header() {
    let harness = healthy();

    execute(&harness, true, false);

    let log = harness.log.entries();
    let before_each_header: Vec<&str> = [
        "reporter.group_header(Let's check your dotfiles...)",
        "reporter.group_header(Let's make sure tabsmith is set up correctly...)",
        "reporter.group_header(Let's check if your environment is supported...)",
        "reporter.group_header(Let's check if your system is compatible...)",
    ]
    .iter()
    .map(|header| log[position(&log, header) - 1].as_str())
    .collect();
    assert_eq!(
        before_each_header,
        ["system.is_on_path(pwsh)", "system.operating_system", "settings.load", "system.is_on_path(fish)"]
    );
}

#[test]
fn should_probe_nothing_of_the_later_groups_when_the_first_group_aborts() {
    let harness = with_a_broken_dotfile_that_cannot_be_repaired();

    let verdict = execute(&harness, false, false);

    assert_eq!(
        (verdict, harness.probe_questions()),
        (
            DoctorVerdict::Failed,
            events(&[
                "integrations.locate_paths",
                "system.parent_executable",
                "integrations.list(bash)",
                "system.is_on_path(zsh)",
                "system.is_on_path(fish)",
                "system.is_on_path(pwsh)",
                "integrations.check(bash pre post into .bashrc)",
            ])
        )
    );
}

#[test]
fn should_not_start_the_progress_with_all() {
    let harness = healthy();

    execute(&harness, true, false);

    assert!(!harness.reporter_events().contains(&event("start_progress")));
}

#[test]
fn should_start_the_progress_before_the_first_group_without_all() {
    let harness = healthy();

    execute(&harness, false, false);

    assert_eq!(harness.reporter_events()[0], event("start_progress"));
}

#[test]
fn should_print_the_error_of_the_system_group_and_skip_the_rest_of_that_group() {
    let harness = with_an_old_bash_running_and_fish_installed();

    execute(&harness, false, true);

    let events = harness.reporter_events();
    assert_eq!(
        events[1..5],
        [
            event("stop_progress"),
            "reporter.outcome(Bash is new enough, Err { reason: \"Bash 4.0 is not supported (needs >= 4.4 for PS0); install a newer bash or switch to zsh\", info: [], fix: None })".to_string(),
            event("blank_line"),
            event("stop_progress"),
        ]
    );
}

#[test]
fn should_keep_the_verdict_passed_when_only_the_system_group_fails() {
    let harness = with_an_old_bash_running_and_fish_installed();

    let verdict = execute(&harness, false, true);

    assert_eq!(verdict, DoctorVerdict::Passed);
}

#[test]
fn should_still_say_everything_looks_good_when_only_the_system_group_fails() {
    let harness = with_an_old_bash_running_and_fish_installed();

    execute(&harness, false, true);

    assert_eq!(
        summary_after_the_last_stop_of_the_progress(&harness),
        events(&["reporter.everything_looks_good", "reporter.blank_line", "reporter.support_hint", "reporter.blank_line"])
    );
}

#[test]
fn should_not_let_a_system_warning_fail_a_strict_run() {
    let harness = on_freebsd();

    let verdict = execute(&harness, false, true);

    assert_eq!(verdict, DoctorVerdict::Passed);
}

#[test]
fn should_print_the_summary_of_a_failed_run() {
    let harness = without_the_data_directory_and_unable_to_create_it();

    execute(&harness, false, false);

    assert_eq!(
        summary_after_the_last_stop_of_the_progress(&harness),
        events(&["reporter.blank_line", "reporter.doctor_found_errors", "reporter.blank_line"])
    );
}

#[test]
fn should_print_the_summary_of_a_passed_run() {
    let harness = healthy();

    execute(&harness, false, false);

    assert_eq!(
        summary_after_the_last_stop_of_the_progress(&harness),
        events(&["reporter.everything_looks_good", "reporter.blank_line", "reporter.support_hint", "reporter.blank_line"])
    );
}

#[test]
fn should_print_the_summary_of_a_passed_run_with_all_without_saying_everything_looks_good() {
    let harness = healthy();

    execute(&harness, true, false);

    assert_eq!(
        summary_after_the_last_stop_of_the_progress(&harness),
        events(&["reporter.blank_line", "reporter.support_hint", "reporter.blank_line"])
    );
}

#[test]
fn should_fail_a_run_with_an_error() {
    let harness = without_the_data_directory_and_unable_to_create_it();

    let verdict = execute(&harness, false, false);

    assert_eq!(verdict, DoctorVerdict::Failed);
}

#[test]
fn should_fail_a_strict_run_with_a_warning() {
    let harness = healthy_with_a_warning_in_the_environment_group();

    let verdict = execute(&harness, false, true);

    assert_eq!(verdict, DoctorVerdict::Failed);
}

#[test]
fn should_pass_a_strict_run_without_warnings() {
    let harness = healthy();

    let verdict = execute(&harness, false, true);

    assert_eq!(verdict, DoctorVerdict::Passed);
}

#[test]
fn should_pass_a_run_with_a_warning_that_is_not_strict() {
    let harness = healthy_with_a_warning_in_the_environment_group();

    let verdict = execute(&harness, false, false);

    assert_eq!(verdict, DoctorVerdict::Passed);
}

#[test]
fn should_return_the_paths_error_before_any_event() {
    let harness = DoctorHarness::new().with_integrations(InMemoryIntegrationProbe::default().failing_to_locate("no home"));

    let result = harness.use_case().execute(DoctorRequest::new(false, false));

    assert!(matches!(&result, Err(DiagnosticsError::IntegrationPaths(error)) if error.to_string() == "no home"));
    assert_eq!(harness.log.entries(), events(&["integrations.locate_paths"]));
}

#[test]
fn should_print_the_restart_notice_and_clear_the_reminder_after_the_summary() {
    let harness = healthy();
    harness.reminder.save_pending();

    execute(&harness, false, false);

    assert_eq!(
        tail(&harness, 4),
        events(&["reporter.blank_line", "reminder.is_pending", "reporter.restart_notice", "reminder.clear_pending"])
    );
    assert!(!harness.reminder.is_pending());
}

#[test]
fn should_only_ask_for_the_reminder_when_none_is_pending() {
    let harness = healthy();

    execute(&harness, false, false);

    assert_eq!(tail(&harness, 2), events(&["reporter.blank_line", "reminder.is_pending"]));
}

#[test]
fn should_print_the_notice_of_a_reminder_saved_by_a_fix_of_the_same_run() {
    let harness = without_the_data_directory_and_saving_a_reminder_when_creating_it();

    execute(&harness, false, false);

    let log = harness.log.entries();
    let saved = position(&log, "reminder.save_pending");
    let asked = position(&log, "reminder.is_pending");
    let printed = position(&log, "reporter.restart_notice");
    assert_eq!((saved < asked, asked < printed, harness.reminder.is_pending()), (true, true, false));
}

#[test]
fn should_handle_the_reminder_after_a_failed_run() {
    let harness = without_the_data_directory_and_unable_to_create_it();
    harness.reminder.save_pending();

    execute(&harness, false, false);

    assert_eq!(
        tail(&harness, 4),
        events(&["reporter.blank_line", "reminder.is_pending", "reporter.restart_notice", "reminder.clear_pending"])
    );
}

#[test]
fn should_handle_the_reminder_after_a_run_with_all() {
    let harness = healthy();
    harness.reminder.save_pending();

    execute(&harness, true, false);

    assert_eq!(
        tail(&harness, 4),
        events(&["reporter.blank_line", "reminder.is_pending", "reporter.restart_notice", "reminder.clear_pending"])
    );
}
