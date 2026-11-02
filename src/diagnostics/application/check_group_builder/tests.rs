use crate::diagnostics::application::check_group_builder::CheckGroupBuilder;
use crate::diagnostics::application::ports::integration_probe::InMemoryIntegrationProbe;
use crate::diagnostics::application::ports::settings_snapshot::InMemorySettingsSnapshot;
use crate::diagnostics::application::ports::system_facts_probe::InMemorySystemFactsProbe;
use crate::diagnostics::application::test_fixtures::{sample_paths, EventLog};
use crate::diagnostics::domain::value_objects::check_group::CheckGroup;
use crate::diagnostics::domain::value_objects::check_severity::CheckSeverity;
use crate::diagnostics::domain::value_objects::check_subject::CheckSubject;
use crate::diagnostics::domain::value_objects::operating_system::OperatingSystem;
use crate::diagnostics::domain::value_objects::system_check::SystemCheck;
use crate::diagnostics::domain::value_objects::system_facts::SystemFacts;
use crate::settings::domain::value_objects::setting_value::SettingValue;
use crate::settings::domain::value_objects::settings::Settings;
use crate::shared::domain::value_objects::shell::Shell;
use crate::shell_integration::domain::entities::integration::Integration;
use std::path::PathBuf;
use std::rc::Rc;

const TIMED_OUT: &str = "could not ask pwsh for $PROFILE: timed out after 5s";

fn builder_of(
    system: InMemorySystemFactsProbe,
    integrations: InMemoryIntegrationProbe,
) -> CheckGroupBuilder {
    let settings = InMemorySettingsSnapshot::returning(Settings::default());
    CheckGroupBuilder::new(Rc::new(system), Rc::new(integrations), Rc::new(settings))
}

fn builder_with_settings(
    system: InMemorySystemFactsProbe,
    settings: Rc<InMemorySettingsSnapshot>,
) -> CheckGroupBuilder {
    CheckGroupBuilder::new(
        Rc::new(system),
        Rc::new(InMemoryIntegrationProbe::default()),
        settings,
    )
}

fn dotfile_of(shell: Shell, path: &str) -> Integration {
    Integration::dotfile(shell, PathBuf::from(path))
}

fn severities(group: &CheckGroup) -> Vec<CheckSeverity> {
    group.checks().iter().map(|check| check.severity()).collect()
}

fn names(group: &CheckGroup) -> Vec<&str> {
    group.checks().iter().map(|check| check.name()).collect()
}

fn facts_of(group: &CheckGroup) -> Rc<SystemFacts> {
    match group.checks()[0].subject() {
        CheckSubject::Environment(_, facts) => Rc::clone(facts),
        other => panic!("not an environment check: {other:?}"),
    }
}

fn severity_of(group: &CheckGroup, name: &str) -> CheckSeverity {
    group
        .checks()
        .iter()
        .find(|check| check.name() == name)
        .unwrap_or_else(|| panic!("no check named {name}"))
        .severity()
}

#[test]
fn should_check_the_current_shell_without_asking_whether_it_is_on_path() {
    let log = EventLog::default();
    let system = InMemorySystemFactsProbe::default().logging_to(log.clone());
    let integrations = InMemoryIntegrationProbe::default()
        .logging_to(log.clone())
        .listing(Shell::Bash, vec![dotfile_of(Shell::Bash, "/home/u/.bashrc")]);
    let builder = builder_of(system, integrations);

    builder.dotfiles(&sample_paths(), Some(Shell::Bash));

    assert_eq!(
        log.entries(),
        [
            "integrations.list(bash)",
            "system.is_on_path(zsh)",
            "system.is_on_path(fish)",
            "system.is_on_path(pwsh)",
        ]
    );
}

#[test]
fn should_check_the_current_shell_as_normal_and_other_shells_on_path_as_soft() {
    let system = InMemorySystemFactsProbe::default().with_program_on_path("zsh");
    let integrations = InMemoryIntegrationProbe::default()
        .listing(Shell::Bash, vec![dotfile_of(Shell::Bash, "/home/u/.bashrc")])
        .listing(Shell::Zsh, vec![dotfile_of(Shell::Zsh, "/home/u/.zshrc")]);
    let builder = builder_of(system, integrations);

    let group = builder.dotfiles(&sample_paths(), Some(Shell::Bash));

    assert_eq!(severities(&group), [CheckSeverity::Normal, CheckSeverity::Soft]);
}

#[test]
fn should_leave_out_a_shell_that_is_off_path_and_never_list_its_integrations() {
    let log = EventLog::default();
    let system = InMemorySystemFactsProbe::default().logging_to(log.clone());
    let integrations = InMemoryIntegrationProbe::default()
        .logging_to(log.clone())
        .listing(Shell::Bash, vec![dotfile_of(Shell::Bash, "/home/u/.bashrc")]);
    let builder = builder_of(system, integrations);

    let group = builder.dotfiles(&sample_paths(), Some(Shell::Bash));

    assert_eq!(names(&group), ["bash ~/.bashrc integration check"]);
    assert!(!log.entries().iter().any(|entry| entry.starts_with("integrations.list(zsh")));
}

#[test]
fn should_check_every_shell_on_path_as_soft_when_there_is_no_current_shell() {
    let system = InMemorySystemFactsProbe::default()
        .with_program_on_path("bash")
        .with_program_on_path("zsh")
        .with_program_on_path("fish")
        .with_program_on_path("pwsh");
    let integrations = InMemoryIntegrationProbe::default()
        .listing(Shell::Bash, vec![dotfile_of(Shell::Bash, "/home/u/.bashrc")])
        .listing(Shell::Zsh, vec![dotfile_of(Shell::Zsh, "/home/u/.zshrc")])
        .listing(Shell::Fish, vec![dotfile_of(Shell::Fish, "/home/u/.config/fish/config.fish")])
        .listing(Shell::Pwsh, vec![dotfile_of(Shell::Pwsh, "/home/u/profile.ps1")]);
    let builder = builder_of(system, integrations);

    let group = builder.dotfiles(&sample_paths(), None);

    assert_eq!(severities(&group), [CheckSeverity::Soft; 4]);
}

#[test]
fn should_name_one_check_per_integration_of_bash_in_listing_order() {
    let integrations = InMemoryIntegrationProbe::default().listing(
        Shell::Bash,
        vec![
            dotfile_of(Shell::Bash, "/home/u/.bashrc"),
            dotfile_of(Shell::Bash, "/home/u/.profile"),
        ],
    );
    let builder = builder_of(InMemorySystemFactsProbe::default(), integrations);

    let group = builder.dotfiles(&sample_paths(), Some(Shell::Bash));

    assert_eq!(
        names(&group),
        ["bash ~/.bashrc integration check", "bash ~/.profile integration check"]
    );
}

#[test]
fn should_make_the_dotfile_checks_evaluate_the_listed_integrations() {
    let bashrc = dotfile_of(Shell::Bash, "/home/u/.bashrc");
    let integrations = InMemoryIntegrationProbe::default().listing(Shell::Bash, vec![bashrc.clone()]);
    let builder = builder_of(InMemorySystemFactsProbe::default(), integrations);

    let group = builder.dotfiles(&sample_paths(), Some(Shell::Bash));

    assert_eq!(group.checks()[0].subject(), &CheckSubject::Dotfile(bashrc));
}

#[test]
fn should_turn_a_listing_failure_of_the_current_shell_into_one_normal_check() {
    let integrations = InMemoryIntegrationProbe::default().failing_to_list(Shell::Pwsh, TIMED_OUT);
    let builder = builder_of(InMemorySystemFactsProbe::default(), integrations);

    let group = builder.dotfiles(&sample_paths(), Some(Shell::Pwsh));

    assert_eq!(names(&group), ["pwsh integration check"]);
    assert_eq!(severities(&group), [CheckSeverity::Normal]);
    assert_eq!(group.checks()[0].subject(), &CheckSubject::ListingFailure(TIMED_OUT.to_string()));
}

#[test]
fn should_turn_a_listing_failure_of_another_shell_into_one_soft_check() {
    let system = InMemorySystemFactsProbe::default().with_program_on_path("pwsh");
    let integrations = InMemoryIntegrationProbe::default().failing_to_list(Shell::Pwsh, TIMED_OUT);
    let builder = builder_of(system, integrations);

    let group = builder.dotfiles(&sample_paths(), None);

    assert_eq!(names(&group), ["pwsh integration check"]);
    assert_eq!(severities(&group), [CheckSeverity::Soft]);
}

#[test]
fn should_list_the_integrations_of_a_shell_only_once() {
    let log = EventLog::default();
    let integrations = InMemoryIntegrationProbe::default()
        .logging_to(log.clone())
        .listing(Shell::Bash, vec![dotfile_of(Shell::Bash, "/home/u/.bashrc")]);
    let builder = builder_of(InMemorySystemFactsProbe::default(), integrations);

    builder.dotfiles(&sample_paths(), Some(Shell::Bash));

    assert_eq!(log.entries().iter().filter(|e| *e == "integrations.list(bash)").count(), 1);
}

#[test]
fn should_head_the_dotfiles_group_with_the_dotfiles_header() {
    let integrations = InMemoryIntegrationProbe::default().listing(Shell::Bash, vec![]);
    let builder = builder_of(InMemorySystemFactsProbe::default(), integrations);

    let group = builder.dotfiles(&sample_paths(), Some(Shell::Bash));

    assert_eq!(group.header(), "Let's check your dotfiles...");
}

#[test]
fn should_point_the_settings_corruption_check_at_the_file_the_probe_names() {
    let system = InMemorySystemFactsProbe::default().with_settings_file("/custom/settings.json");
    let builder = builder_of(system, InMemoryIntegrationProbe::default());

    let group = builder.setup(&sample_paths());

    assert_eq!(
        group.checks()[2].subject(),
        &CheckSubject::SettingsCorruption(PathBuf::from("/custom/settings.json"))
    );
}

#[test]
fn should_ask_the_probe_for_the_settings_file_under_the_data_root() {
    let builder = builder_of(InMemorySystemFactsProbe::default(), InMemoryIntegrationProbe::default());

    let group = builder.setup(&sample_paths());

    assert_eq!(
        group.checks()[2].subject(),
        &CheckSubject::SettingsCorruption(PathBuf::from("/home/u/.tabsmith/settings.json"))
    );
}

#[test]
fn should_set_up_four_checks_on_linux() {
    let system = InMemorySystemFactsProbe::default().with_operating_system(OperatingSystem::Linux);
    let builder = builder_of(system, InMemoryIntegrationProbe::default());

    let group = builder.setup(&sample_paths());

    assert_eq!(group.checks().len(), 4);
}

#[test]
fn should_add_the_two_windows_checks_only_on_windows() {
    let system = InMemorySystemFactsProbe::default().with_operating_system(OperatingSystem::Windows);
    let builder = builder_of(system, InMemoryIntegrationProbe::default());

    let group = builder.setup(&sample_paths());

    assert_eq!(group.checks().len(), 6);
    assert_eq!(&names(&group)[4..], ["Windows Console Check", "Windows version"]);
}

#[test]
fn should_head_the_setup_group_with_the_setup_header() {
    let builder = builder_of(InMemorySystemFactsProbe::default(), InMemoryIntegrationProbe::default());

    let group = builder.setup(&sample_paths());

    assert_eq!(group.header(), "Let's make sure tabsmith is set up correctly...");
}

#[test]
fn should_load_the_settings_exactly_once_when_building_the_environment_group() {
    let settings = Rc::new(InMemorySettingsSnapshot::returning(Settings::default()));
    let builder = builder_with_settings(InMemorySystemFactsProbe::default(), Rc::clone(&settings));

    builder.environment(None);

    assert_eq!(settings.loads(), 1);
}

#[test]
fn should_not_load_the_settings_for_the_other_groups() {
    let settings = Rc::new(InMemorySettingsSnapshot::returning(Settings::default()));
    let builder = builder_with_settings(InMemorySystemFactsProbe::default(), Rc::clone(&settings));

    builder.setup(&sample_paths());
    builder.system(None, None);

    assert_eq!(settings.loads(), 0);
}

#[test]
fn should_load_the_settings_after_reading_the_environment() {
    let log = EventLog::default();
    let system = InMemorySystemFactsProbe::default().logging_to(log.clone());
    let settings = Rc::new(InMemorySettingsSnapshot::returning(Settings::default()).logging_to(log.clone()));
    let builder = builder_with_settings(system, settings);

    builder.environment(None);

    assert_eq!(log.entries().last().map(String::as_str), Some("settings.load"));
}

#[test]
fn should_count_a_variable_set_to_an_empty_value_as_present() {
    let system = InMemorySystemFactsProbe::default().with_env("CI", "");
    let builder = builder_of(system, InMemoryIntegrationProbe::default());

    let facts = facts_of(&builder.environment(None));

    assert!(facts.terminal().in_ci());
}

#[test]
fn should_count_an_unset_variable_as_absent() {
    let builder = builder_of(InMemorySystemFactsProbe::default(), InMemoryIntegrationProbe::default());

    let facts = facts_of(&builder.environment(None));

    assert!(!facts.terminal().in_ci());
    assert!(!facts.terminal().inside_emacs());
    assert!(!facts.terminal().launched_by_tabsmith());
}

#[test]
fn should_read_the_presence_of_emacs_and_tabsmith_launch_variables() {
    let system = InMemorySystemFactsProbe::default()
        .with_env("INSIDE_EMACS", "29.1")
        .with_env("PROCESS_LAUNCHED_BY_TABSMITH", "1");
    let builder = builder_of(system, InMemoryIntegrationProbe::default());

    let facts = facts_of(&builder.environment(None));

    assert!(facts.terminal().inside_emacs());
    assert!(facts.terminal().launched_by_tabsmith());
}

#[test]
fn should_take_the_text_variables_as_none_when_the_probe_has_no_text_for_them() {
    let builder = builder_of(InMemorySystemFactsProbe::default(), InMemoryIntegrationProbe::default());

    let facts = facts_of(&builder.environment(None));

    assert_eq!(facts.terminal().term_program(), None);
    assert_eq!(facts.terminal().bundle_identifier(), None);
    assert_eq!(facts.terminal().tabsmith_term(), None);
    assert_eq!(facts.session_id(), None);
}

#[test]
fn should_carry_the_text_variables_the_probe_returned() {
    let system = InMemorySystemFactsProbe::default()
        .with_env("TERM_PROGRAM", "WarpTerminal")
        .with_env("__CFBundleIdentifier", "com.example.term")
        .with_env("TABSMITH_TERM", "v1")
        .with_env("TABSMITH_SESSION_ID", "abc");
    let builder = builder_of(system, InMemoryIntegrationProbe::default());

    let facts = facts_of(&builder.environment(None));

    assert_eq!(facts.terminal().term_program(), Some("WarpTerminal"));
    assert_eq!(facts.terminal().bundle_identifier(), Some("com.example.term"));
    assert_eq!(facts.terminal().tabsmith_term(), Some("v1"));
    assert_eq!(facts.session_id(), Some("abc"));
}

#[test]
fn should_carry_the_version_the_probe_reports() {
    let system = InMemorySystemFactsProbe::default().with_tabsmith_version("9.9.9");
    let builder = builder_of(system, InMemoryIntegrationProbe::default());

    let facts = facts_of(&builder.environment(None));

    assert_eq!(facts.terminal().tabsmith_version(), "9.9.9");
}

#[test]
fn should_carry_the_settings_the_snapshot_returned() {
    let given = Settings::default().with("inline.enabled", SettingValue::Bool(false));
    let settings = Rc::new(InMemorySettingsSnapshot::returning(given.clone()));
    let builder = builder_with_settings(InMemorySystemFactsProbe::default(), settings);

    let facts = facts_of(&builder.environment(None));

    assert_eq!(facts.settings(), &given);
}

#[test]
fn should_recognise_a_cmd_parent_whatever_the_separator() {
    let builder = builder_of(InMemorySystemFactsProbe::default(), InMemoryIntegrationProbe::default());

    let facts = facts_of(&builder.environment(Some("C:\\Windows\\System32\\CMD.EXE")));

    assert!(facts.terminal().parent_is_cmd());
}

#[test]
fn should_not_take_another_parent_for_cmd() {
    let builder = builder_of(InMemorySystemFactsProbe::default(), InMemoryIntegrationProbe::default());

    let facts = facts_of(&builder.environment(Some("/bin/bash")));

    assert!(!facts.terminal().parent_is_cmd());
}

#[test]
fn should_check_the_session_as_normal_when_the_parent_shell_runs_it() {
    let system = InMemorySystemFactsProbe::default().with_env("TABSMITH_SESSION_ID", "s1");
    let builder = builder_of(system, InMemoryIntegrationProbe::default());

    let group = builder.environment(Some("/bin/zsh"));

    assert_eq!(severity_of(&group, "Session check"), CheckSeverity::Normal);
}

#[test]
fn should_fall_back_to_tabsmith_shell_when_the_parent_is_unknown() {
    let system = InMemorySystemFactsProbe::default()
        .with_env("TABSMITH_SESSION_ID", "s1")
        .with_env("TABSMITH_SHELL", "pwsh");
    let builder = builder_of(system, InMemoryIntegrationProbe::default());

    let group = builder.environment(None);

    assert_eq!(severity_of(&group, "Session check"), CheckSeverity::Skip);
}

#[test]
fn should_prefer_the_parent_over_tabsmith_shell() {
    let system = InMemorySystemFactsProbe::default()
        .with_env("TABSMITH_SESSION_ID", "s1")
        .with_env("TABSMITH_SHELL", "pwsh");
    let builder = builder_of(system, InMemoryIntegrationProbe::default());

    let group = builder.environment(Some("bash"));

    assert_eq!(severity_of(&group, "Session check"), CheckSeverity::Normal);
}

#[test]
fn should_skip_the_session_check_outside_a_session() {
    let builder = builder_of(InMemorySystemFactsProbe::default(), InMemoryIntegrationProbe::default());

    let group = builder.environment(Some("bash"));

    assert_eq!(severity_of(&group, "Session check"), CheckSeverity::Skip);
}

#[test]
fn should_check_inline_as_soft_when_the_settings_turn_it_off() {
    let off = Settings::default().with("inline.enabled", SettingValue::Bool(false));
    let settings = Rc::new(InMemorySettingsSnapshot::returning(off));
    let builder = builder_with_settings(InMemorySystemFactsProbe::default(), settings);

    let group = builder.environment(None);

    assert_eq!(severity_of(&group, "Inline"), CheckSeverity::Soft);
}

#[test]
fn should_check_inline_as_normal_when_the_settings_turn_it_on() {
    let on = Settings::default().with("inline.enabled", SettingValue::Bool(true));
    let settings = Rc::new(InMemorySettingsSnapshot::returning(on));
    let builder = builder_with_settings(InMemorySystemFactsProbe::default(), settings);

    let group = builder.environment(None);

    assert_eq!(severity_of(&group, "Inline"), CheckSeverity::Normal);
}

#[test]
fn should_build_the_environment_checks_over_one_shared_set_of_facts() {
    let builder = builder_of(InMemorySystemFactsProbe::default(), InMemoryIntegrationProbe::default());

    let group = builder.environment(None);

    let all_share_facts = group.checks().iter().all(|check| match check.subject() {
        CheckSubject::Environment(_, facts) => Rc::ptr_eq(facts, &facts_of(&group)),
        _ => false,
    });
    assert!(all_share_facts);
}

#[test]
fn should_head_the_environment_group_with_the_environment_header() {
    let builder = builder_of(InMemorySystemFactsProbe::default(), InMemoryIntegrationProbe::default());

    let group = builder.environment(None);

    assert_eq!(group.header(), "Let's check if your environment is supported...");
}

#[test]
fn should_run_bash_from_the_parent_path_when_bash_is_the_current_shell() {
    let builder = builder_of(InMemorySystemFactsProbe::default(), InMemoryIntegrationProbe::default());

    let group = builder.system(Some(Shell::Bash), Some("/opt/bin/bash"));

    assert_eq!(
        group.checks()[1].subject(),
        &CheckSubject::System(SystemCheck::Bash { program: "/opt/bin/bash".to_string() })
    );
}

#[test]
fn should_run_plain_bash_when_another_shell_is_the_current_one() {
    let builder = builder_of(InMemorySystemFactsProbe::default(), InMemoryIntegrationProbe::default());

    let group = builder.system(Some(Shell::Zsh), Some("/bin/zsh"));

    assert_eq!(
        group.checks()[1].subject(),
        &CheckSubject::System(SystemCheck::Bash { program: "bash".to_string() })
    );
}

#[test]
fn should_check_fish_as_normal_when_fish_is_on_path() {
    let system = InMemorySystemFactsProbe::default().with_program_on_path("fish");
    let builder = builder_of(system, InMemoryIntegrationProbe::default());

    let group = builder.system(None, None);

    assert_eq!(severity_of(&group, "Fish is up to date"), CheckSeverity::Normal);
}

#[test]
fn should_skip_the_fish_check_when_fish_is_off_path() {
    let builder = builder_of(InMemorySystemFactsProbe::default(), InMemoryIntegrationProbe::default());

    let group = builder.system(None, None);

    assert_eq!(severity_of(&group, "Fish is up to date"), CheckSeverity::Skip);
}

#[test]
fn should_ask_whether_fish_is_on_path_whatever_the_current_shell() {
    let log = EventLog::default();
    let system = InMemorySystemFactsProbe::default().logging_to(log.clone());
    let builder = builder_of(system, InMemoryIntegrationProbe::default());

    builder.system(Some(Shell::Fish), Some("/usr/bin/fish"));

    assert_eq!(log.entries(), ["system.is_on_path(fish)"]);
}

#[test]
fn should_head_the_system_group_with_the_system_header() {
    let builder = builder_of(InMemorySystemFactsProbe::default(), InMemoryIntegrationProbe::default());

    let group = builder.system(None, None);

    assert_eq!(group.header(), "Let's check if your system is compatible...");
}
