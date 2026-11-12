use crate::modules::shared::domain::value_objects::shell::Shell;
use crate::modules::shared::infrastructure::os::executable_lookup::which;
use crate::modules::shared::infrastructure::output_format::OutputFormat;
use crate::modules::shared::infrastructure::terminal_style::{sgr, stdout_tty};
use crate::modules::shell_integration::application::integration_failure::IntegrationFailure;
use crate::modules::shell_integration::application::integration_status_report::IntegrationStatusReport;
use crate::modules::shell_integration::domain::value_objects::integration_target::IntegrationTarget;
use crate::modules::shell_integration::infrastructure::cli::init_command::ShellArg;
use crate::modules::shell_integration::infrastructure::shell_integration_module::ShellIntegrationModule;
use clap::Subcommand;

#[derive(Subcommand, Clone, Debug)]
pub(crate) enum IntegrationTargetArg {
    /// Shell dotfile integrations
    Dotfiles { shell: Option<ShellArg> },
    /// All integrations
    All,
}

impl From<IntegrationTargetArg> for IntegrationTarget {
    fn from(t: IntegrationTargetArg) -> IntegrationTarget {
        match t {
            IntegrationTargetArg::Dotfiles { shell } => IntegrationTarget::Dotfiles(shell.map(Shell::from)),
            IntegrationTargetArg::All => IntegrationTarget::All,
        }
    }
}

#[derive(Subcommand)]
pub(crate) enum IntegrationsCmd {
    /// Install an integration
    Install {
        #[command(subcommand)]
        target: IntegrationTargetArg,
        /// Suppress status messages
        #[arg(long, short)]
        silent: bool,
    },
    /// Uninstall an integration
    Uninstall {
        #[command(subcommand)]
        target: IntegrationTargetArg,
        #[arg(long, short)]
        silent: bool,
    },
    /// Reinstall an integration
    Reinstall {
        #[command(subcommand)]
        target: IntegrationTargetArg,
        #[arg(long, short)]
        silent: bool,
    },
    /// Check the status of an integration
    Status {
        #[command(subcommand)]
        target: IntegrationTargetArg,
        #[arg(long, short, value_enum, default_value_t)]
        format: OutputFormat,
    },
}

/// `tabsmith integrations` (SPEC §2.6): the paths first, whatever the subcommand, then its target's shells. A
/// failure is the error main prints, never a print of its own.
pub(crate) fn run_integrations_command(
    cmd: IntegrationsCmd,
    module: &ShellIntegrationModule,
) -> anyhow::Result<i32> {
    let paths = module.locate_integration_paths()?;
    match cmd {
        IntegrationsCmd::Install { target, silent } => {
            let installed = module
                .install_integrations
                .execute(&paths, &target_shells(target))
                .map_err(integration_failures_error)?;
            announce(silent, if installed { "Installed!" } else { "Already installed" });
        }
        IntegrationsCmd::Uninstall { target, silent } => {
            let uninstalled = module
                .uninstall_integrations
                .execute(&paths, &target_shells(target))
                .map_err(integration_failures_error)?;
            announce(silent, if uninstalled { "Uninstalled!" } else { "Not installed" });
        }
        IntegrationsCmd::Reinstall { target, silent } => {
            let installed = module
                .reinstall_integrations
                .execute(&paths, &target_shells(target))
                .map_err(integration_failures_error)?;
            announce(silent, if installed { "Installed!" } else { "Already installed" });
        }
        IntegrationsCmd::Status { target, format } => {
            let report = module
                .get_integration_status
                .execute(&paths, &target_shells(target));
            for line in status_lines(&report, format, stdout_tty())? {
                println!("{line}");
            }
        }
    }
    Ok(0)
}

/// The shells `target` names; pwsh is looked up on PATH only when it names no shell (SPEC §2.6).
fn target_shells(target: IntegrationTargetArg) -> Vec<Shell> {
    IntegrationTarget::from(target).shells(|| which("pwsh").is_some())
}

/// `message` on stdout unless `--silent`.
fn announce(silent: bool, message: &str) {
    if !silent {
        println!("{message}");
    }
}

/// The failures of a run over integrations as one error, the text main prints after `error: ` (the legacy
/// `bail!`); a failed integration's description is bold when stdout is a terminal, as `paint` does.
pub(crate) fn integration_failures_error(failures: Vec<IntegrationFailure>) -> anyhow::Error {
    anyhow::Error::msg(failures_text(&failures, stdout_tty()))
}

/// The failures each in a paragraph of its own after a blank line, a failed integration's description in bold when
/// `styled` (SPEC §2.6).
fn failures_text(failures: &[IntegrationFailure], styled: bool) -> String {
    let parts: Vec<String> = failures
        .iter()
        .map(|failure| failure_line(failure, styled))
        .collect();
    format!("\n\n{}", parts.join("\n\n"))
}

/// The shell it could not list, or the failed integration's description in bold when `styled`, then the reason.
fn failure_line(failure: &IntegrationFailure, styled: bool) -> String {
    match failure {
        IntegrationFailure::Listing { shell, reason } => format!("{shell}: {reason}"),
        IntegrationFailure::Operation {
            description,
            reason,
        } => format!("{}: {reason}", sgr(description, "1", styled)),
    }
}

/// What `integrations status` prints in `format`, one `println!` each (SPEC §2.6): the plain rows and failures, or
/// one JSON document, its keys sorted (serde_json without `preserve_order`), compact or pretty.
fn status_lines(
    report: &IntegrationStatusReport,
    format: OutputFormat,
    styled: bool,
) -> serde_json::Result<Vec<String>> {
    let document = match format {
        OutputFormat::Plain => return Ok(plain_status_lines(report, styled)),
        OutputFormat::Json => status_json(report).to_string(),
        OutputFormat::JsonPretty => serde_json::to_string_pretty(&status_json(report))?,
    };
    Ok(vec![document])
}

fn status_json(report: &IntegrationStatusReport) -> serde_json::Value {
    serde_json::json!({
        "integrations": report.integrations.iter().map(|row| serde_json::json!({
            "installed": row.installed, "description": row.description, "shell": row.shell, "file_name": row.file_name,
        })).collect::<Vec<_>>(),
        "errors": report.failures.iter().map(|failure| {
            let (shell, reason) = match failure {
                IntegrationFailure::Listing { shell, reason } => (shell, reason),
                // Never built: status reports only the shells it cannot list (the legacy `status` had no other error).
                IntegrationFailure::Operation { description, reason } => (description, reason),
            };
            serde_json::json!({"shell": shell, "error": {"title": reason, "message": null}})
        }).collect::<Vec<_>>(),
    })
}

/// `integrations status` in plain text (SPEC §2.6): every row, its mark green when installed and red when not if
/// `styled`, then every shell whose integrations could not be listed.
fn plain_status_lines(report: &IntegrationStatusReport, styled: bool) -> Vec<String> {
    let rows = report.integrations.iter().map(|row| {
        let mark = if row.installed {
            sgr("✔", "32", styled)
        } else {
            sgr("✘", "31", styled)
        };
        format!("{mark} {}", row.description)
    });
    let failures = report.failures.iter().map(|failure| failure_line(failure, styled));
    rows.chain(failures).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::shared::infrastructure::testing::temp_dir::TempDir;
    use crate::modules::shell_integration::application::integration_status::IntegrationStatus;
    use crate::modules::shell_integration::infrastructure::testing::integration_fixtures::{ctx, install_dotfiles};

    #[test]
    fn should_word_a_shell_it_could_not_list_as_the_shell_and_reason_after_a_blank_line() {
        let failures = [IntegrationFailure::Listing {
            shell: "pwsh".into(),
            reason: "pwsh printed an empty $PROFILE".into(),
        }];

        let text = failures_text(&failures, false);

        assert_eq!(text, "\n\npwsh: pwsh printed an empty $PROFILE");
    }

    fn failed_zshrc() -> IntegrationFailure {
        IntegrationFailure::Operation {
            description: "zsh pre post into .zshrc".into(),
            reason: "Permission denied: /home/u/.zshrc".into(),
        }
    }

    #[test]
    fn should_put_a_blank_line_between_failures() {
        let failures = [
            IntegrationFailure::Listing {
                shell: "pwsh".into(),
                reason: "pwsh printed an empty $PROFILE".into(),
            },
            failed_zshrc(),
        ];

        let text = failures_text(&failures, false);

        assert_eq!(
            text,
            "\n\npwsh: pwsh printed an empty $PROFILE\n\nzsh pre post into .zshrc: Permission denied: /home/u/.zshrc"
        );
    }

    #[test]
    fn should_bold_the_description_of_a_failed_operation_when_styled() {
        let text = failures_text(&[failed_zshrc()], true);

        assert_eq!(text, "\n\n\x1b[1mzsh pre post into .zshrc\x1b[0m: Permission denied: /home/u/.zshrc");
    }

    #[test]
    fn should_word_a_failed_operation_plainly_when_not_styled() {
        let text = failures_text(&[failed_zshrc()], false);

        assert_eq!(text, "\n\nzsh pre post into .zshrc: Permission denied: /home/u/.zshrc");
    }

    /// A listing failure is never styled, so the text does not depend on whether stdout is a terminal.
    #[test]
    fn should_give_the_failures_text_as_the_error_main_prints() {
        let failures = vec![IntegrationFailure::Listing {
            shell: "pwsh".into(),
            reason: "pwsh printed an empty $PROFILE".into(),
        }];

        let error = integration_failures_error(failures);

        assert_eq!(format!("{error:#}"), "\n\npwsh: pwsh printed an empty $PROFILE");
    }

    /// zsh's `.zshrc` is installed, fish's pre file is not, and pwsh could not be listed.
    fn status_report() -> IntegrationStatusReport {
        IntegrationStatusReport {
            integrations: vec![
                IntegrationStatus {
                    installed: true,
                    description: "zsh pre post into .zshrc".into(),
                    shell: "zsh".into(),
                    file_name: ".zshrc".into(),
                },
                IntegrationStatus {
                    installed: false,
                    description: "fish pre".into(),
                    shell: "fish".into(),
                    file_name: "00_tabsmith_pre.fish".into(),
                },
            ],
            failures: vec![IntegrationFailure::Listing {
                shell: "pwsh".into(),
                reason: "pwsh printed an empty $PROFILE".into(),
            }],
        }
    }

    #[test]
    fn should_mark_an_installed_row_green_and_one_not_installed_red_when_styled() {
        let report = status_report();

        let lines = plain_status_lines(&report, true);

        assert_eq!(
            lines,
            [
                "\x1b[32m✔\x1b[0m zsh pre post into .zshrc",
                "\x1b[31m✘\x1b[0m fish pre",
                "pwsh: pwsh printed an empty $PROFILE"
            ]
        );
    }

    #[test]
    fn should_list_every_row_plainly_before_the_shells_it_could_not_list_when_not_styled() {
        let report = status_report();

        let lines = plain_status_lines(&report, false);

        assert_eq!(
            lines,
            [
                "✔ zsh pre post into .zshrc",
                "✘ fish pre",
                "pwsh: pwsh printed an empty $PROFILE"
            ]
        );
    }

    #[test]
    fn should_give_the_status_as_one_line_of_json_with_sorted_keys_and_a_null_message() {
        let report = status_report();

        let lines = status_lines(&report, OutputFormat::Json, false).unwrap();

        assert_eq!(
            lines,
            [concat!(
                r#"{"errors":[{"error":{"message":null,"title":"pwsh printed an empty $PROFILE"},"shell":"pwsh"}],"#,
                r#""integrations":[{"description":"zsh pre post into .zshrc","file_name":".zshrc","installed":true,"#,
                r#""shell":"zsh"},{"description":"fish pre","file_name":"00_tabsmith_pre.fish","installed":false,"#,
                r#""shell":"fish"}]}"#
            )]
        );
    }

    #[test]
    fn should_give_the_status_as_json_indented_by_two_spaces_when_pretty() {
        let report = status_report();

        let lines = status_lines(&report, OutputFormat::JsonPretty, false).unwrap();

        assert_eq!(
            lines,
            [r#"{
  "errors": [
    {
      "error": {
        "message": null,
        "title": "pwsh printed an empty $PROFILE"
      },
      "shell": "pwsh"
    }
  ],
  "integrations": [
    {
      "description": "zsh pre post into .zshrc",
      "file_name": ".zshrc",
      "installed": true,
      "shell": "zsh"
    },
    {
      "description": "fish pre",
      "file_name": "00_tabsmith_pre.fish",
      "installed": false,
      "shell": "fish"
    }
  ]
}"#]
        );
    }

    #[test]
    fn integrations_install_dotfiles_reports() {
        let temp_dir = TempDir::new("apply");
        let dir = temp_dir.path().to_path_buf();
        let c = ctx(&dir);
        assert!(install_dotfiles(&c, &[Shell::Zsh, Shell::Fish]).unwrap());
        assert!(!install_dotfiles(&c, &[Shell::Zsh, Shell::Fish]).unwrap());
        // A dotfile that cannot be written is reported with its description.
        std::fs::create_dir_all(c.home().join(".bashrc")).unwrap(); // a directory where the file should be
        let e = format!("{:#}", install_dotfiles(&c, &[Shell::Bash]).unwrap_err());
        assert!(
            e.starts_with("\n\n")
                && e.contains("bash pre post into .bashrc")
                && !e.contains(".profile"),
            "{e}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
