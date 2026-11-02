use super::*;
use crate::shell_integration::domain::value_objects::integration_phase::IntegrationPhase;

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
