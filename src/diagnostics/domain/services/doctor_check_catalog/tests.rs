use super::*;
use crate::diagnostics::domain::value_objects::terminal_environment::TerminalEnvironmentParts;
use crate::settings::domain::value_objects::setting_value::SettingValue;
use crate::settings::infrastructure::adapters::setting_value_json::settings_from_json;
use crate::shell_integration::domain::value_objects::integration_phase::IntegrationPhase;
use serde_json::{json, Value};

fn home() -> PathBuf {
    PathBuf::from("/home/bob")
}

fn path_under_home(parts: &[&str]) -> PathBuf {
    parts.iter().fold(home(), |path, part| path.join(part))
}

fn display_relative(parts: &[&str]) -> String {
    format!("~/{}", parts.iter().collect::<PathBuf>().display())
}

fn bashrc() -> Integration {
    Integration::dotfile(Shell::Bash, PathBuf::from("/h/.bashrc"))
}

fn error_of(result: Result<(), ShellIntegrationError>) -> (String, Vec<InfoLine>, Option<CheckFix>) {
    match dotfile_outcome(&bashrc(), result) {
        CheckOutcome::Err { reason, info, fix } => (reason, info, fix),
        other => panic!("expected an error, got {other:?}"),
    }
}

fn reinstall_hint_for(shell: &str) -> InfoLine {
    InfoLine::Command {
        before: "Run ".into(),
        command: format!("tabsmith integrations install dotfiles {shell}"),
        after: format!(" to reinstall shell integrations for {shell}"),
    }
}

fn script_binary(shell: Shell, binary: &str) -> (Shell, String) {
    (shell, binary.to_string())
}

fn scripts_fix_shells(binary: &str, scripts: &[(Shell, String)]) -> Vec<Shell> {
    match scripts_outcome(binary, scripts) {
        CheckOutcome::Err { fix: Some(CheckFix::InstallMissingIntegrations(shells)), .. } => shells,
        other => panic!("expected an install-missing fix, got {other:?}"),
    }
}

fn check_names(checks: &[Check]) -> Vec<&str> {
    checks.iter().map(Check::name).collect()
}

#[test]
fn should_greet_the_dotfile_and_setup_groups_with_their_headers() {
    assert_eq!(
        (DOTFILES_HEADER, SETUP_HEADER),
        ("Let's check your dotfiles...", "Let's make sure tabsmith is set up correctly...")
    );
}

#[test]
fn should_check_the_current_shell_as_normal_without_asking_the_path() {
    let severity = dotfile_severity(Shell::Bash, Some(Shell::Bash), || panic!("the PATH must not be asked"));

    assert_eq!(severity, Some(CheckSeverity::Normal));
}

#[test]
fn should_check_another_shell_on_the_path_as_soft() {
    let severity = dotfile_severity(Shell::Zsh, Some(Shell::Bash), || true);

    assert_eq!(severity, Some(CheckSeverity::Soft));
}

#[test]
fn should_not_check_another_shell_off_the_path() {
    let severity = dotfile_severity(Shell::Pwsh, Some(Shell::Bash), || false);

    assert_eq!(severity, None);
}

#[test]
fn should_check_every_shell_on_the_path_as_soft_when_the_parent_is_no_shell() {
    let severities = Shell::ALL.map(|shell| dotfile_severity(shell, None, || true));

    assert_eq!(severities, [Some(CheckSeverity::Soft); 4]);
}

#[test]
fn should_name_a_bash_dotfile_check_after_its_short_path() {
    let integration = Integration::dotfile(Shell::Bash, path_under_home(&[".bashrc"]));

    let check = dotfile_check(&home(), integration, CheckSeverity::Normal);

    assert_eq!(check.name(), "bash ~/.bashrc integration check");
}

#[test]
fn should_name_the_profile_check_of_bash() {
    let integration = Integration::dotfile(Shell::Bash, path_under_home(&[".profile"]));

    let check = dotfile_check(&home(), integration, CheckSeverity::Normal);

    assert_eq!(check.name(), "bash ~/.profile integration check");
}

#[test]
fn should_name_the_login_checks_of_bash() {
    let names: Vec<String> = [".bash_login", ".bash_profile"]
        .iter()
        .map(|file| dotfile_check(&home(), Integration::dotfile(Shell::Bash, path_under_home(&[file])), CheckSeverity::Normal))
        .map(|check| check.name().to_string())
        .collect();

    assert_eq!(names, ["bash ~/.bash_login integration check", "bash ~/.bash_profile integration check"]);
}

#[test]
fn should_name_a_zsh_dotfile_inside_home_after_its_short_path() {
    let integration = Integration::dotfile(Shell::Zsh, path_under_home(&["zsh", ".zshrc"]));

    let check = dotfile_check(&home(), integration, CheckSeverity::Soft);

    assert_eq!(check.name(), format!("zsh {} integration check", display_relative(&["zsh", ".zshrc"])));
}

#[test]
fn should_name_a_zsh_dotfile_outside_home_after_its_absolute_path() {
    let outside = PathBuf::from("/opt/z/.zshrc");

    let check = dotfile_check(&home(), Integration::dotfile(Shell::Zsh, outside.clone()), CheckSeverity::Soft);

    assert_eq!(check.name(), format!("zsh {} integration check", outside.display()));
}

#[test]
fn should_name_the_fish_conf_d_checks_after_their_short_paths() {
    let parts = |file: &'static str| [".config", "fish", "conf.d", file];
    let pre = Integration::whole_file(Shell::Fish, path_under_home(&parts("00_tabsmith_pre.fish")), IntegrationPhase::Pre);
    let post = Integration::whole_file(Shell::Fish, path_under_home(&parts("99_tabsmith_post.fish")), IntegrationPhase::Post);

    let names = [pre, post].map(|i| dotfile_check(&home(), i, CheckSeverity::Soft).name().to_string());

    assert_eq!(
        names,
        [
            format!("fish {} integration check", display_relative(&parts("00_tabsmith_pre.fish"))),
            format!("fish {} integration check", display_relative(&parts("99_tabsmith_post.fish"))),
        ]
    );
}

#[test]
fn should_name_the_pwsh_profile_check_after_its_short_path() {
    let profile = ["Documents", "PowerShell", "Microsoft.PowerShell_profile.ps1"];
    let integration = Integration::dotfile(Shell::Pwsh, path_under_home(&profile));

    let check = dotfile_check(&home(), integration, CheckSeverity::Soft);

    assert_eq!(check.name(), format!("pwsh {} integration check", display_relative(&profile)));
}

#[test]
fn should_evaluate_a_dotfile_check_as_its_integration() {
    let integration = Integration::dotfile(Shell::Bash, path_under_home(&[".bashrc"]));

    let check = dotfile_check(&home(), integration.clone(), CheckSeverity::Normal);

    assert_eq!((check.severity(), check.subject()), (CheckSeverity::Normal, &CheckSubject::Dotfile(integration)));
}

#[test]
fn should_shorten_home_itself_to_a_tilde_and_a_slash() {
    assert_eq!(short_path(&home(), &home()), "~/");
}

#[test]
fn should_not_shorten_a_sibling_directory_that_only_shares_the_prefix_text() {
    let sibling = PathBuf::from("/home/bobby/x");

    assert_eq!(short_path(&PathBuf::from("/home/bob"), &sibling), sibling.display().to_string());
}

#[test]
fn should_name_a_listing_failure_check_after_the_shell_only() {
    let check = listing_failure_check(Shell::Pwsh, CheckSeverity::Soft, "pwsh printed an empty $PROFILE".into());

    assert_eq!(check.name(), "pwsh integration check");
}

#[test]
fn should_carry_the_listing_failure_reason_in_the_subject() {
    let check = listing_failure_check(Shell::Pwsh, CheckSeverity::Soft, "timed out after 5s".into());

    assert_eq!(
        (check.severity(), check.subject()),
        (CheckSeverity::Soft, &CheckSubject::ListingFailure("timed out after 5s".into()))
    );
}

#[test]
fn should_pass_an_integration_that_checks_out() {
    assert_eq!(dotfile_outcome(&bashrc(), Ok(())), CheckOutcome::Ok);
}

#[test]
fn should_strip_the_prefix_of_a_not_installed_integration() {
    let (reason, _, _) = error_of(Err(ShellIntegrationError::NotInstalled("/h/.bashrc does not source pre integration".into())));

    assert_eq!(reason, "/h/.bashrc does not source pre integration");
}

#[test]
fn should_offer_to_reinstall_a_not_installed_integration() {
    let (_, info, fix) = error_of(Err(ShellIntegrationError::NotInstalled("missing".into())));

    assert_eq!((info, fix), (vec![reinstall_hint_for("bash")], Some(CheckFix::InstallIntegration(bashrc()))));
}

#[test]
fn should_keep_the_message_of_an_improper_integration() {
    let (reason, _, _) = error_of(Err(ShellIntegrationError::Improper("/h/.bashrc does not source post integration last".into())));

    assert_eq!(reason, "/h/.bashrc does not source post integration last");
}

#[test]
fn should_keep_a_multi_line_improper_message_verbatim() {
    let message = "/h/.tabsmith/shell/bashrc.pre.sh should contain:\nexport A=1\nexport B=2";

    let (reason, _, _) = error_of(Err(ShellIntegrationError::Improper(message.into())));

    assert_eq!(reason, message);
}

#[test]
fn should_offer_to_reinstall_an_improper_integration() {
    let (_, info, fix) = error_of(Err(ShellIntegrationError::Improper("wrong".into())));

    assert_eq!((info, fix), (vec![reinstall_hint_for("bash")], Some(CheckFix::InstallIntegration(bashrc()))));
}

#[test]
fn should_describe_a_dotfile_that_does_not_exist() {
    let (reason, _, fix) = error_of(Err(ShellIntegrationError::FileDoesNotExist(PathBuf::from("/h/.bashrc"))));

    assert_eq!((reason.as_str(), fix), ("File does not exist: /h/.bashrc", Some(CheckFix::InstallIntegration(bashrc()))));
}

#[test]
fn should_describe_a_dotfile_that_cannot_be_read() {
    let denied = ShellIntegrationError::PermissionDenied(PathBuf::from("/h/.bashrc"));

    let (reason, _, _) = error_of(Err(denied));

    assert_eq!(reason, "Permission denied: /h/.bashrc");
}

#[test]
fn should_keep_the_hint_but_offer_no_fix_for_a_permission_denied() {
    let denied = ShellIntegrationError::PermissionDenied(PathBuf::from("/h/.bashrc"));

    let (_, info, fix) = error_of(Err(denied));

    assert_eq!((info, fix), (vec![reinstall_hint_for("bash")], None));
}

#[test]
fn should_describe_an_io_failure_after_the_path_and_the_cause() {
    let io = ShellIntegrationError::Io(PathBuf::from("/h/.bashrc"), std::io::Error::other("disk full"));

    let (reason, _, fix) = error_of(Err(io));

    assert_eq!((reason.as_str(), fix), ("/h/.bashrc: disk full", Some(CheckFix::InstallIntegration(bashrc()))));
}

#[test]
fn should_describe_an_unknown_environment_with_its_message() {
    let (reason, _, fix) = error_of(Err(ShellIntegrationError::Environment("HOME is not set".into())));

    assert_eq!((reason.as_str(), fix), ("HOME is not set", Some(CheckFix::InstallIntegration(bashrc()))));
}

#[test]
fn should_describe_a_spec_bundle_failure_with_its_text() {
    let (reason, _, fix) = error_of(Err(ShellIntegrationError::SpecBundle("disk full".into())));

    assert_eq!(
        (reason.as_str(), fix),
        ("could not extract the completion specs: disk full", Some(CheckFix::InstallIntegration(bashrc())))
    );
}

#[test]
fn should_name_fish_in_the_hint_of_a_fish_integration() {
    let conf = Integration::whole_file(Shell::Fish, PathBuf::from("/h/00_tabsmith_pre.fish"), IntegrationPhase::Pre);

    let outcome = dotfile_outcome(&conf, Err(ShellIntegrationError::NotInstalled("missing".into())));

    assert_eq!(
        outcome,
        CheckOutcome::Err {
            reason: "missing".into(),
            info: vec![reinstall_hint_for("fish")],
            fix: Some(CheckFix::InstallIntegration(conf)),
        }
    );
}

#[test]
fn should_pass_a_data_directory_that_exists() {
    assert_eq!(data_directory_outcome(&PathBuf::from("/d/.tabsmith"), true), CheckOutcome::Ok);
}

#[test]
fn should_offer_to_create_a_missing_data_directory() {
    let data_root = PathBuf::from("/d/.tabsmith");

    let outcome = data_directory_outcome(&data_root, false);

    assert_eq!(
        outcome,
        CheckOutcome::Err {
            reason: format!("No directory at path {}", data_root.display()),
            info: vec![],
            fix: Some(CheckFix::CreateDataDirectory(data_root)),
        }
    );
}

#[test]
fn should_pass_when_no_script_is_found() {
    assert_eq!(scripts_outcome("/new", &[]), CheckOutcome::Ok);
}

#[test]
fn should_pass_when_every_script_points_at_this_binary() {
    let scripts = [script_binary(Shell::Bash, "/new"), script_binary(Shell::Zsh, "/new")];

    assert_eq!(scripts_outcome("/new", &scripts), CheckOutcome::Ok);
}

#[test]
fn should_name_the_first_stale_binary_in_the_reason() {
    let scripts = [
        script_binary(Shell::Bash, "/old"),
        script_binary(Shell::Bash, "/old"),
        script_binary(Shell::Zsh, "/new"),
        script_binary(Shell::Zsh, "/old2"),
    ];

    let outcome = scripts_outcome("/new", &scripts);

    assert!(matches!(outcome, CheckOutcome::Err { reason, info, .. } if reason == "Integration scripts reference /old" && info.is_empty()));
}

#[test]
fn should_install_the_missing_integrations_of_every_stale_shell() {
    let scripts = [
        script_binary(Shell::Bash, "/old"),
        script_binary(Shell::Bash, "/old"),
        script_binary(Shell::Zsh, "/new"),
        script_binary(Shell::Zsh, "/old2"),
    ];

    let shells = scripts_fix_shells("/new", &scripts);

    assert_eq!(shells, [Shell::Bash, Shell::Zsh]);
}

#[test]
fn should_remove_only_consecutive_duplicate_shells_from_the_fix() {
    let scripts = [script_binary(Shell::Bash, "/a"), script_binary(Shell::Zsh, "/b"), script_binary(Shell::Bash, "/c")];

    let shells = scripts_fix_shells("/new", &scripts);

    assert_eq!(shells, [Shell::Bash, Shell::Zsh, Shell::Bash]);
}

#[test]
fn should_pass_settings_that_load() {
    assert_eq!(settings_outcome(&PathBuf::from("/d/settings.json"), true), CheckOutcome::Ok);
}

#[test]
fn should_offer_to_reset_settings_that_do_not_load() {
    let settings_file = PathBuf::from("/d/settings.json");

    let outcome = settings_outcome(&settings_file, false);

    assert_eq!(
        outcome,
        CheckOutcome::Err {
            reason: "tabsmith settings file is corrupted".into(),
            info: vec![],
            fix: Some(CheckFix::ResetSettings(settings_file)),
        }
    );
}

#[test]
fn should_pass_when_the_completion_specs_are_extracted() {
    assert_eq!(specs_outcome(true), CheckOutcome::Ok);
}

#[test]
fn should_offer_to_install_the_missing_completion_specs() {
    assert_eq!(
        specs_outcome(false),
        CheckOutcome::Err {
            reason: "Completion specs are not extracted".into(),
            info: vec![],
            fix: Some(CheckFix::InstallSpecBundle),
        }
    );
}

#[test]
fn should_pass_when_the_console_is_attached() {
    assert_eq!(windows_console_outcome(true), CheckOutcome::Ok);
}

#[test]
fn should_reject_a_terminal_without_the_windows_console_apis() {
    assert!(matches!(
        windows_console_outcome(false),
        CheckOutcome::Err { reason, fix: None, .. } if reason == "Windows Console APIs are not supported in this terminal"
    ));
}

#[test]
fn should_explain_the_windows_console_failure_in_eight_lines() {
    let expected: Vec<InfoLine> = [
        "The pseudoterminal layer only supports the new Windows Console API.",
        "MinTTY and other TTY implementations may not work properly.",
        "",
        "You can try the following fixes to get completions working:",
        "- If using Git for Windows, reinstall and choose \"Use default console window\" instead of MinTTY",
        "- If using Git for Windows and you really want to use MinTTY, reinstall and check \"Enable experimental support for pseudo consoles\"",
        "- Use your shell with a supported terminal emulator like Windows Terminal.",
        "- Launch your terminal emulator with winpty (e.g. winpty mintty). NOTE: this can lead to some UI bugs.",
    ]
    .map(|line| InfoLine::Text(line.to_string()))
    .to_vec();

    let info = match windows_console_outcome(false) {
        CheckOutcome::Err { info, .. } => info,
        other => panic!("expected an error, got {other:?}"),
    };

    assert_eq!(info, expected);
}

#[test]
fn should_reject_a_windows_build_older_than_conpty() {
    assert_eq!(
        windows_version_setup_outcome(Some(17762)),
        CheckOutcome::err("Windows 10 1809 or newer is required (ConPTY)")
    );
}

#[test]
fn should_accept_the_first_windows_build_with_conpty() {
    assert_eq!(windows_version_setup_outcome(Some(17763)), CheckOutcome::Ok);
}

#[test]
fn should_accept_an_unknown_windows_build() {
    assert_eq!(windows_version_setup_outcome(None), CheckOutcome::Ok);
}

#[test]
fn should_list_the_four_setup_checks_off_windows() {
    let checks = setup_checks(PathBuf::from("/d/settings.json"), false);

    assert_eq!(
        check_names(&checks),
        [
            "tabsmith data dir exists",
            "Integration scripts point at this binary",
            "tabsmith settings corruption",
            "Completion specs installed",
        ]
    );
}

#[test]
fn should_add_the_two_windows_checks_last_on_windows() {
    let checks = setup_checks(PathBuf::from("/d/settings.json"), true);

    assert_eq!(&check_names(&checks)[4..], ["Windows Console Check", "Windows version"]);
}

#[test]
fn should_evaluate_each_setup_check_by_its_subject() {
    let settings_file = PathBuf::from("/d/settings.json");

    let checks = setup_checks(settings_file.clone(), true);

    assert_eq!(
        checks.iter().map(Check::subject).cloned().collect::<Vec<_>>(),
        [
            CheckSubject::DataDirectory,
            CheckSubject::ScriptsPointAtBinary,
            CheckSubject::SettingsCorruption(settings_file),
            CheckSubject::SpecsExtracted,
            CheckSubject::WindowsConsole,
            CheckSubject::WindowsVersion,
        ]
    );
}

#[test]
fn should_make_every_setup_check_normal() {
    let checks = setup_checks(PathBuf::from("/d/settings.json"), true);

    assert!(checks.iter().all(|check| check.severity() == CheckSeverity::Normal));
}

const VERSION: &str = "1.2.3";

fn terminal_with(parts: TerminalEnvironmentParts) -> TerminalEnvironment {
    TerminalEnvironment::new(TerminalEnvironmentParts { tabsmith_version: VERSION.into(), ..parts })
}

fn supported_terminal() -> TerminalEnvironmentParts {
    TerminalEnvironmentParts { tabsmith_term: Some(VERSION.into()), ..TerminalEnvironmentParts::default() }
}

fn terminal_reason(terminal: &TerminalEnvironment) -> String {
    match terminal_outcome(terminal) {
        CheckOutcome::Err { reason, .. } => reason,
        other => panic!("expected an error, got {other:?}"),
    }
}

fn settings_of(value: Value) -> Settings {
    settings_from_json(value.as_object().unwrap().clone())
}

fn facts_with(session_id: Option<&str>, settings: Settings) -> SystemFacts {
    let terminal = terminal_with(TerminalEnvironmentParts::default());
    SystemFacts::new(terminal, session_id.map(String::from), settings)
}

fn session_kind_of(session_id: Option<&str>, parent: Option<&str>, tabsmith_shell: Option<&str>) -> CheckSeverity {
    session_kind(session_id, parent, tabsmith_shell)
}

fn inline_error(settings: &Settings, env_is_set: impl Fn(&str) -> bool) -> String {
    match inline_outcome(settings, env_is_set) {
        CheckOutcome::Err { reason, .. } => reason,
        other => panic!("expected an error, got {other:?}"),
    }
}

fn profile_settings(source: &str, profile: Value) -> Settings {
    settings_of(json!({ "inline.source": source, format!("inline.profiles.{source}"): profile }))
}

fn remote_profile(name: &str) -> Settings {
    profile_settings(name, json!({ "provider": "openai", "model": "gpt" }))
}

fn local_profile(base_url: &str) -> Settings {
    profile_settings("mine", json!({ "provider": "openai", "model": "llama", "base_url": base_url }))
}

fn version(text: &str) -> Option<ToolVersion> {
    ToolVersion::parse(text)
}

fn unused_macos_version() -> Option<ToolVersion> {
    panic!("the macOS version must not be asked")
}

fn unused_windows_build() -> Option<u32> {
    panic!("the Windows build must not be asked")
}

fn os_warning(os: OperatingSystem, macos: Option<ToolVersion>, windows: Option<u32>) -> CheckOutcome {
    os_outcome(&os, || macos, || windows)
}

fn error_reason(outcome: CheckOutcome) -> String {
    match outcome {
        CheckOutcome::Err { reason, .. } => reason,
        other => panic!("expected an error, got {other:?}"),
    }
}

#[test]
fn should_greet_the_environment_and_system_groups_with_their_headers() {
    assert_eq!(
        (ENVIRONMENT_HEADER, SYSTEM_HEADER),
        ("Let's check if your environment is supported...", "Let's check if your system is compatible...")
    );
}

#[test]
fn should_recognise_cmd_as_the_parent_by_its_base_name() {
    let parents = ["C:\\Windows\\System32\\CMD.EXE", "cmd", "/x/cmd.exe", "CMD"];

    let hits: Vec<bool> = parents.iter().map(|parent| parent_is_cmd(Some(parent))).collect();

    assert_eq!(hits, [true, true, true, true]);
}

#[test]
fn should_not_recognise_other_parents_as_cmd() {
    let parents = [Some("/x/cmd.exe.bak"), Some("/bin/bash"), Some("/x/mycmd"), Some(""), None];

    let hits: Vec<bool> = parents.iter().map(|parent| parent_is_cmd(*parent)).collect();

    assert_eq!(hits, [false; 5]);
}

#[test]
fn should_run_the_session_check_in_a_wrapped_zsh_session() {
    let severity = session_kind_of(Some("abc"), Some("/bin/zsh"), None);

    assert_eq!(severity, CheckSeverity::Normal);
}

#[test]
fn should_skip_the_session_check_without_a_session_id() {
    let severity = session_kind_of(None, Some("/bin/zsh"), None);

    assert_eq!(severity, CheckSeverity::Skip);
}

#[test]
fn should_skip_the_session_check_when_the_parent_is_pwsh() {
    let severity = session_kind_of(Some("abc"), Some("/usr/bin/pwsh"), Some("bash"));

    assert_eq!(severity, CheckSeverity::Skip);
}

#[test]
fn should_skip_the_session_check_when_only_tabsmith_shell_says_pwsh() {
    let severity = session_kind_of(Some("abc"), None, Some("pwsh"));

    assert_eq!(severity, CheckSeverity::Skip);
}

#[test]
fn should_prefer_the_parent_shell_over_tabsmith_shell_for_the_session_check() {
    let severity = session_kind_of(Some("abc"), Some("/bin/bash"), Some("pwsh"));

    assert_eq!(severity, CheckSeverity::Normal);
}

#[test]
fn should_fall_back_to_tabsmith_shell_when_the_parent_is_not_a_shell() {
    let severity = session_kind_of(Some("abc"), Some("/usr/bin/sudo"), Some("pwsh"));

    assert_eq!(severity, CheckSeverity::Skip);
}

#[test]
fn should_run_the_session_check_for_an_empty_session_id() {
    let severity = session_kind_of(Some(""), None, None);

    assert_eq!(severity, CheckSeverity::Normal);
}

#[test]
fn should_accept_a_terminal_running_the_latest_integration() {
    let terminal = terminal_with(supported_terminal());

    assert_eq!(terminal_outcome(&terminal), CheckOutcome::Ok);
}

#[test]
fn should_refuse_warp() {
    let terminal = terminal_with(TerminalEnvironmentParts {
        term_program: Some("WarpTerminal".into()),
        ..supported_terminal()
    });

    assert_eq!(terminal_reason(&terminal), "WarpTerminal is not supported");
}

#[test]
fn should_accept_another_term_program() {
    let terminal = terminal_with(TerminalEnvironmentParts {
        term_program: Some("iTerm.app".into()),
        ..supported_terminal()
    });

    assert_eq!(terminal_outcome(&terminal), CheckOutcome::Ok);
}

#[test]
fn should_refuse_cmd_as_the_parent() {
    let terminal = terminal_with(TerminalEnvironmentParts { parent_is_cmd: true, ..supported_terminal() });

    assert_eq!(terminal_reason(&terminal), "CMD isn't supported, use PowerShell 7 (pwsh) instead");
}

#[test]
fn should_refuse_emacs() {
    let terminal = terminal_with(TerminalEnvironmentParts { inside_emacs: true, ..supported_terminal() });

    assert_eq!(terminal_reason(&terminal), "Emacs is not supported");
}

#[test]
fn should_refuse_securecrt() {
    let terminal = terminal_with(TerminalEnvironmentParts {
        bundle_identifier: Some("com.vandyke.SecureCRT".into()),
        ..supported_terminal()
    });

    assert_eq!(terminal_reason(&terminal), "SecureCRT is not supported");
}

#[test]
fn should_accept_another_bundle_identifier() {
    let terminal = terminal_with(TerminalEnvironmentParts {
        bundle_identifier: Some("com.apple.Terminal".into()),
        ..supported_terminal()
    });

    assert_eq!(terminal_outcome(&terminal), CheckOutcome::Ok);
}

#[test]
fn should_refuse_a_process_launched_by_tabsmith() {
    let terminal = terminal_with(TerminalEnvironmentParts { launched_by_tabsmith: true, ..supported_terminal() });

    assert_eq!(terminal_reason(&terminal), "tabsmith can not run in a process it launched");
}

#[test]
fn should_refuse_a_ci_run() {
    let terminal = terminal_with(TerminalEnvironmentParts { in_ci: true, ..supported_terminal() });

    assert_eq!(terminal_reason(&terminal), "Doctor can not run in CI");
}

#[test]
fn should_report_warp_before_emacs_and_ci() {
    let terminal = terminal_with(TerminalEnvironmentParts {
        term_program: Some("WarpTerminal".into()),
        inside_emacs: true,
        in_ci: true,
        ..supported_terminal()
    });

    assert_eq!(terminal_reason(&terminal), "WarpTerminal is not supported");
}

#[test]
fn should_report_emacs_before_ci() {
    let terminal = terminal_with(TerminalEnvironmentParts {
        inside_emacs: true,
        in_ci: true,
        ..supported_terminal()
    });

    assert_eq!(terminal_reason(&terminal), "Emacs is not supported");
}

#[test]
fn should_report_emacs_even_when_tabsmith_term_is_correct() {
    let terminal = terminal_with(TerminalEnvironmentParts { inside_emacs: true, ..supported_terminal() });

    assert_eq!(terminal_outcome(&terminal), CheckOutcome::err("Emacs is not supported"));
}

#[test]
fn should_report_cmd_before_emacs_securecrt_launch_and_ci() {
    let terminal = terminal_with(TerminalEnvironmentParts {
        parent_is_cmd: true,
        inside_emacs: true,
        bundle_identifier: Some("com.vandyke.SecureCRT".into()),
        launched_by_tabsmith: true,
        in_ci: true,
        ..supported_terminal()
    });

    assert_eq!(terminal_reason(&terminal), "CMD isn't supported, use PowerShell 7 (pwsh) instead");
}

#[test]
fn should_report_securecrt_before_a_launch_and_ci() {
    let terminal = terminal_with(TerminalEnvironmentParts {
        bundle_identifier: Some("com.vandyke.SecureCRT".into()),
        launched_by_tabsmith: true,
        in_ci: true,
        ..supported_terminal()
    });

    assert_eq!(terminal_reason(&terminal), "SecureCRT is not supported");
}

#[test]
fn should_report_a_launch_before_ci() {
    let terminal = terminal_with(TerminalEnvironmentParts {
        launched_by_tabsmith: true,
        in_ci: true,
        ..supported_terminal()
    });

    assert_eq!(terminal_reason(&terminal), "tabsmith can not run in a process it launched");
}

#[test]
fn should_report_a_missing_integration_when_tabsmith_term_is_unset() {
    let terminal = terminal_with(TerminalEnvironmentParts::default());

    assert_eq!(
        terminal_outcome(&terminal),
        CheckOutcome::err("tabsmith is not running in this terminal, please try restarting your terminal")
    );
}

#[test]
fn should_report_an_old_integration_with_the_value_it_found() {
    let terminal = terminal_with(TerminalEnvironmentParts {
        tabsmith_term: Some("0.0.9".into()),
        ..TerminalEnvironmentParts::default()
    });

    assert_eq!(
        terminal_outcome(&terminal),
        CheckOutcome::Err {
            reason: "This terminal is not running with the latest integration, please restart your terminal".into(),
            info: vec![InfoLine::Text("TABSMITH_TERM=0.0.9".into())],
            fix: None,
        }
    );
}

#[test]
fn should_report_an_empty_tabsmith_term_as_an_old_integration() {
    let terminal = terminal_with(TerminalEnvironmentParts {
        tabsmith_term: Some(String::new()),
        ..TerminalEnvironmentParts::default()
    });

    assert_eq!(
        terminal_outcome(&terminal),
        CheckOutcome::Err {
            reason: "This terminal is not running with the latest integration, please restart your terminal".into(),
            info: vec![InfoLine::Text("TABSMITH_TERM=".into())],
            fix: None,
        }
    );
}

#[test]
fn should_accept_a_terminal_with_no_presence_facts_and_the_right_version() {
    let terminal = terminal_with(TerminalEnvironmentParts {
        tabsmith_term: Some(VERSION.into()),
        inside_emacs: false,
        launched_by_tabsmith: false,
        in_ci: false,
        parent_is_cmd: false,
        ..TerminalEnvironmentParts::default()
    });

    assert!(terminal_outcome(&terminal).is_ok());
}

#[test]
fn should_pass_the_session_check_when_the_run_file_exists() {
    assert_eq!(session_outcome(true), CheckOutcome::Ok);
}

#[test]
fn should_warn_the_session_check_when_the_run_file_is_missing() {
    assert_eq!(
        session_outcome(false),
        CheckOutcome::Warn("Shell has not reported its environment yet (tabsmith _ pre-cmd did not run)".into())
    );
}

#[test]
fn should_leave_autocomplete_enabled_by_default() {
    assert_eq!(autocomplete_enabled_outcome(&Settings::default()), CheckOutcome::Ok);
}

#[test]
fn should_fail_the_autocomplete_check_when_it_is_disabled() {
    let settings = Settings::default().with(AUTOCOMPLETE_DISABLE, SettingValue::Bool(true));

    assert_eq!(
        autocomplete_enabled_outcome(&settings),
        CheckOutcome::Err {
            reason: "Autocomplete disabled.".into(),
            info: vec![InfoLine::Text("To fix run: tabsmith settings autocomplete.disable false".into())],
            fix: None,
        }
    );
}

#[test]
fn should_pass_the_autocomplete_check_when_disable_is_false() {
    let settings = Settings::default().with(AUTOCOMPLETE_DISABLE, SettingValue::Bool(false));

    assert_eq!(autocomplete_enabled_outcome(&settings), CheckOutcome::Ok);
}

#[test]
fn should_not_warn_about_developer_mode_by_default() {
    assert_eq!(developer_mode_outcome(&Settings::default()), CheckOutcome::Ok);
}

#[test]
fn should_warn_when_developer_mode_is_enabled() {
    let settings = Settings::default().with(AUTOCOMPLETE_DEVELOPER_MODE, SettingValue::Bool(true));

    assert_eq!(
        developer_mode_outcome(&settings),
        CheckOutcome::Warn("autocomplete.developerMode is enabled".into())
    );
}

#[test]
fn should_warn_when_inline_is_disabled() {
    let settings = Settings::default().with(INLINE_ENABLED, SettingValue::Bool(false));

    assert_eq!(
        inline_outcome(&settings, |_| panic!("the environment must not be asked")),
        CheckOutcome::Warn("Inline is disabled, to re-enable run: tabsmith inline enable".into())
    );
}

#[test]
fn should_pass_inline_with_the_default_source() {
    assert_eq!(inline_outcome(&Settings::default(), |_| false), CheckOutcome::Ok);
}

#[test]
fn should_pass_inline_for_each_built_in_source() {
    let outcomes: Vec<CheckOutcome> = ["history+specs", "history", "specs"]
        .iter()
        .map(|source| inline_outcome(&settings_of(json!({ "inline.source": source })), |_| false))
        .collect();

    assert_eq!(outcomes, [CheckOutcome::Ok, CheckOutcome::Ok, CheckOutcome::Ok]);
}

#[test]
fn should_fail_inline_for_an_unknown_source() {
    let settings = settings_of(json!({ "inline.source": "nope" }));

    assert_eq!(inline_error(&settings, |_| false), "Inline source 'nope' is not a valid profile");
}

#[test]
fn should_fail_inline_for_a_profile_without_a_model_naming_it() {
    let settings = profile_settings("broken", json!({ "provider": "openai" }));

    assert_eq!(inline_error(&settings, |_| true), "Inline source 'broken' is not a valid profile");
}

#[test]
fn should_fall_back_to_the_default_source_when_the_source_is_not_a_string() {
    let settings = settings_of(json!({ "inline.source": 7 }));

    assert_eq!(inline_outcome(&settings, |_| false), CheckOutcome::Ok);
}

#[test]
fn should_fail_inline_for_a_remote_profile_whose_key_is_unset() {
    let settings = remote_profile("cloud");

    assert_eq!(inline_error(&settings, |_| false), "OPENAI_API_KEY is not set");
}

#[test]
fn should_ask_the_environment_for_the_key_of_a_remote_profile() {
    let settings = remote_profile("cloud");
    let asked = std::cell::RefCell::new(Vec::new());

    inline_outcome(&settings, |name| {
        asked.borrow_mut().push(name.to_string());
        true
    });

    assert_eq!(asked.into_inner(), ["OPENAI_API_KEY"]);
}

#[test]
fn should_pass_inline_for_a_remote_profile_whose_key_is_set() {
    let settings = remote_profile("cloud");

    assert_eq!(inline_outcome(&settings, |name| name == "OPENAI_API_KEY"), CheckOutcome::Ok);
}

#[test]
fn should_name_a_custom_key_variable_in_the_inline_error() {
    let settings = profile_settings(
        "cloud",
        json!({ "provider": "anthropic", "model": "m", "api_key_env": "MY_KEY" }),
    );

    assert_eq!(inline_error(&settings, |name| name == "ANTHROPIC_API_KEY"), "MY_KEY is not set");
}

#[test]
fn should_pass_inline_for_local_profiles_without_a_key() {
    let urls = ["http://localhost:11434/v1", "http://127.0.0.1:8080", "http://[::1]:8080/v1"];

    let outcomes: Vec<CheckOutcome> = urls.iter().map(|url| inline_outcome(&local_profile(url), |_| false)).collect();

    assert_eq!(outcomes, [CheckOutcome::Ok, CheckOutcome::Ok, CheckOutcome::Ok]);
}

#[test]
fn should_treat_a_localhost_substring_anywhere_in_the_url_as_local() {
    let settings = local_profile("https://example.com/?u=://localhost");

    assert_eq!(inline_outcome(&settings, |_| false), CheckOutcome::Ok);
}

#[test]
fn should_treat_a_host_that_merely_starts_with_localhost_as_local() {
    let settings = local_profile("https://localhost.example.com/v1");

    assert_eq!(inline_outcome(&settings, |_| false), CheckOutcome::Ok);
}

#[test]
fn should_not_treat_a_localhost_path_as_local() {
    let settings = local_profile("https://example.com/localhost");

    assert_eq!(inline_error(&settings, |_| false), "OPENAI_API_KEY is not set");
}

#[test]
fn should_list_the_environment_checks_in_order() {
    let checks = environment_checks(facts_with(None, Settings::default()), None, None);

    assert_eq!(
        check_names(&checks),
        [
            "tabsmith terminal integrations",
            "Session check",
            "Autocomplete is enabled",
            "Autocomplete dev mode",
            "Inline"
        ]
    );
}

#[test]
fn should_decide_each_environment_check_by_its_kind() {
    let facts = Rc::new(facts_with(Some("s"), Settings::default()));

    let checks = environment_checks((*facts).clone(), Some("/bin/zsh"), None);

    assert_eq!(
        checks.iter().map(Check::subject).cloned().collect::<Vec<_>>(),
        [
            EnvironmentCheck::TerminalIntegrations,
            EnvironmentCheck::Session,
            EnvironmentCheck::AutocompleteEnabled,
            EnvironmentCheck::DeveloperMode,
            EnvironmentCheck::Inline,
        ]
        .map(|which| CheckSubject::Environment(which, Rc::clone(&facts)))
    );
}

#[test]
fn should_make_every_environment_check_normal_when_a_session_runs_and_inline_is_on() {
    let checks = environment_checks(facts_with(Some("s"), Settings::default()), Some("/bin/zsh"), None);

    assert!(checks.iter().all(|check| check.severity() == CheckSeverity::Normal));
}

#[test]
fn should_skip_the_session_check_without_a_session_id_in_the_list() {
    let checks = environment_checks(facts_with(None, Settings::default()), Some("/bin/zsh"), None);

    assert_eq!(checks[1].severity(), CheckSeverity::Skip);
}

#[test]
fn should_skip_the_session_check_of_a_pwsh_session_in_the_list() {
    let checks = environment_checks(facts_with(Some("s"), Settings::default()), None, Some("pwsh"));

    assert_eq!(checks[1].severity(), CheckSeverity::Skip);
}

#[test]
fn should_make_the_inline_check_soft_when_inline_is_disabled() {
    let settings = Settings::default().with(INLINE_ENABLED, SettingValue::Bool(false));

    let checks = environment_checks(facts_with(None, settings), None, None);

    assert_eq!(checks[4].severity(), CheckSeverity::Soft);
}

#[test]
fn should_share_one_set_of_facts_between_the_environment_checks() {
    let checks = environment_checks(facts_with(None, Settings::default()), None, None);

    let shared: Vec<&Rc<SystemFacts>> = checks
        .iter()
        .map(|check| match check.subject() {
            CheckSubject::Environment(_, facts) => facts,
            other => panic!("expected an environment subject, got {other:?}"),
        })
        .collect();

    assert!(shared.iter().all(|facts| Rc::ptr_eq(facts, shared[0])));
}

#[test]
fn should_pass_the_operating_system_check_on_linux_without_asking_anything() {
    let outcome = os_outcome(&OperatingSystem::Linux, unused_macos_version, unused_windows_build);

    assert_eq!(outcome, CheckOutcome::Ok);
}

#[test]
fn should_warn_about_a_macos_below_11() {
    let outcome = os_warning(OperatingSystem::Macos, version("10.15.7"), None);

    assert_eq!(
        outcome,
        CheckOutcome::Warn("macOS 10.15.7 is not supported (needs macOS 11 or newer)".into())
    );
}

#[test]
fn should_pass_macos_11_and_newer() {
    let outcomes = [version("11"), version("11.0"), version("14.5")].map(|v| os_warning(OperatingSystem::Macos, v, None));

    assert_eq!(outcomes, [CheckOutcome::Ok, CheckOutcome::Ok, CheckOutcome::Ok]);
}

#[test]
fn should_pass_macos_when_its_version_is_unknown() {
    assert_eq!(os_warning(OperatingSystem::Macos, None, None), CheckOutcome::Ok);
}

#[test]
fn should_not_ask_the_windows_build_on_macos() {
    let outcome = os_outcome(&OperatingSystem::Macos, || version("14.5"), unused_windows_build);

    assert_eq!(outcome, CheckOutcome::Ok);
}

#[test]
fn should_warn_about_a_windows_build_below_conpty() {
    let outcome = os_warning(OperatingSystem::Windows, None, Some(17762));

    assert_eq!(outcome, CheckOutcome::Warn("Windows 10 1809 or newer is required (ConPTY)".into()));
}

#[test]
fn should_pass_windows_from_the_conpty_build_on_and_when_unknown() {
    let outcomes = [Some(17763), Some(19045), None].map(|build| os_warning(OperatingSystem::Windows, None, build));

    assert_eq!(outcomes, [CheckOutcome::Ok, CheckOutcome::Ok, CheckOutcome::Ok]);
}

#[test]
fn should_not_ask_the_macos_version_on_windows() {
    let outcome = os_outcome(&OperatingSystem::Windows, unused_macos_version, || Some(19045));

    assert_eq!(outcome, CheckOutcome::Ok);
}

#[test]
fn should_warn_about_an_unsupported_operating_system_without_asking_anything() {
    let outcome = os_outcome(&OperatingSystem::Other("freebsd".into()), unused_macos_version, unused_windows_build);

    assert_eq!(outcome, CheckOutcome::Warn("freebsd is not supported".into()));
}

#[test]
fn should_fail_a_bash_below_4_4_naming_its_version() {
    let reasons: Vec<String> = ["3.2.57", "4.0", "4.3.9"]
        .iter()
        .map(|text| error_reason(bash_outcome(version(text))))
        .collect();

    assert_eq!(
        reasons,
        [
            "Bash 3.2 is not supported (needs >= 4.4 for PS0); install a newer bash or switch to zsh",
            "Bash 4.0 is not supported (needs >= 4.4 for PS0); install a newer bash or switch to zsh",
            "Bash 4.3 is not supported (needs >= 4.4 for PS0); install a newer bash or switch to zsh",
        ]
    );
}

#[test]
fn should_name_a_bash_with_only_a_major_number_with_minor_zero() {
    let reason = error_reason(bash_outcome(Some(ToolVersion::parse("4.").unwrap())));

    assert_eq!(reason, "Bash 4.0 is not supported (needs >= 4.4 for PS0); install a newer bash or switch to zsh");
}

#[test]
fn should_pass_a_bash_from_4_4_on_and_when_unknown() {
    let outcomes = [version("4.4"), version("4.4.0"), version("5.2.37"), None].map(bash_outcome);

    assert_eq!(outcomes, [CheckOutcome::Ok, CheckOutcome::Ok, CheckOutcome::Ok, CheckOutcome::Ok]);
}

#[test]
fn should_fail_a_fish_below_3_3_0_naming_what_it_found() {
    let reason = error_reason(fish_outcome(version("3.2.2")));

    assert_eq!(reason, "your fish version is outdated (need at least 3.3.0, found 3.2.2)");
}

#[test]
fn should_fail_fish_3_3_because_it_compares_without_padding() {
    let reason = error_reason(fish_outcome(version("3.3")));

    assert_eq!(reason, "your fish version is outdated (need at least 3.3.0, found 3.3)");
}

#[test]
fn should_pass_a_fish_from_3_3_0_on_and_when_unknown() {
    let outcomes = [version("3.3.0"), version("4.0.0"), None].map(fish_outcome);

    assert_eq!(outcomes, [CheckOutcome::Ok, CheckOutcome::Ok, CheckOutcome::Ok]);
}

#[test]
fn should_list_the_system_checks_in_order() {
    let checks = system_checks(None, None, false);

    assert_eq!(check_names(&checks), ["OS is supported", "Bash is new enough", "Fish is up to date"]);
}

#[test]
fn should_always_check_the_operating_system_as_normal() {
    let checks = system_checks(Some(Shell::Zsh), Some("/bin/zsh"), false);

    assert_eq!((checks[0].severity(), checks[0].subject()), (CheckSeverity::Normal, &CheckSubject::System(SystemCheck::Os)));
}

#[test]
fn should_check_bash_softly_through_the_parent_path_when_the_current_shell_is_bash() {
    let checks = system_checks(Some(Shell::Bash), Some("/opt/homebrew/bin/bash"), false);

    assert_eq!(
        (checks[1].severity(), checks[1].subject()),
        (CheckSeverity::Soft, &CheckSubject::System(SystemCheck::Bash { program: "/opt/homebrew/bin/bash".into() }))
    );
}

#[test]
fn should_check_bash_by_name_when_the_current_shell_is_bash_without_a_parent_path() {
    let checks = system_checks(Some(Shell::Bash), None, false);

    assert_eq!(checks[1].subject(), &CheckSubject::System(SystemCheck::Bash { program: "bash".into() }));
}

#[test]
fn should_skip_bash_and_keep_the_plain_program_when_another_shell_runs() {
    let checks = system_checks(Some(Shell::Zsh), Some("/bin/zsh"), false);

    assert_eq!(
        (checks[1].severity(), checks[1].subject()),
        (CheckSeverity::Skip, &CheckSubject::System(SystemCheck::Bash { program: "bash".into() }))
    );
}

#[test]
fn should_skip_bash_when_no_shell_is_known() {
    let checks = system_checks(None, Some("/bin/bash"), false);

    assert_eq!(checks[1].severity(), CheckSeverity::Skip);
}

#[test]
fn should_check_fish_as_normal_only_when_it_is_on_the_path() {
    let severities: Vec<CheckSeverity> = [true, false]
        .iter()
        .map(|on_path| system_checks(None, None, *on_path)[2].severity())
        .collect();

    assert_eq!(severities, [CheckSeverity::Normal, CheckSeverity::Skip]);
}

#[test]
fn should_evaluate_the_fish_check_by_its_subject() {
    let checks = system_checks(None, None, true);

    assert_eq!(checks[2].subject(), &CheckSubject::System(SystemCheck::Fish));
}
