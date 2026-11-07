use crate::modules::shared::domain::value_objects::shell::Shell;
use crate::modules::shared::infrastructure::os::executable_lookup::which;
use crate::modules::shared::infrastructure::os::process_info::is_root;
use crate::modules::shared::infrastructure::terminal_style::{epaint, paint};
use crate::modules::shell_integration::application::setup_progress::SetupProgress;
use crate::modules::shell_integration::infrastructure::cli::integrations_command::integration_failures_error;
use crate::modules::shell_integration::infrastructure::cli::user_confirmation::confirm;
use crate::modules::shell_integration::infrastructure::shell_integration_module::ShellIntegrationModule;
use clap::Args;

#[derive(Args)]
pub(crate) struct SetupArgs {
    /// Do not prompt for confirmation
    #[arg(long)]
    pub(crate) no_confirm: bool,
    /// Force installation (e.g. as root)
    #[arg(long)]
    pub(crate) force: bool,
}

/// `tabsmith setup` (SPEC §2.9): refused as root unless `--force`, then asks unless `--no-confirm`, and locates the
/// paths after the answer, whatever it was. Each step is printed as it happens; the restart line only when nothing
/// is left to do by hand.
pub(crate) fn run_setup_command(args: SetupArgs, module: &ShellIntegrationModule) -> anyhow::Result<i32> {
    if is_root() && !args.force {
        eprintln!("{}", epaint("1;31", "Installing as root is not supported."));
        eprintln!(
            "{}",
            epaint(
                "31",
                "If you know what you're doing, run the command again with --force."
            )
        );
        return Ok(1);
    }
    let auto = args.no_confirm
        || confirm("Do you want tabsmith to modify your shell config (you will have to manually do this otherwise)? [Y/n] ")?;
    let paths = module.locate_integration_paths()?;
    let shells: Vec<Shell> = Shell::ALL
        .into_iter()
        .filter(|s| which(s.name()).is_some())
        .collect();
    let manual = module
        .set_up_tabsmith
        .execute(&paths, &shells, auto, print_setup_progress)?;
    if !manual {
        println!("Restart your terminal (or open a new tab) to start using tabsmith.");
    }
    Ok(0)
}

/// One step of setup on stdout, at the moment it happens.
fn print_setup_progress(progress: SetupProgress) {
    match progress {
        SetupProgress::Installed(installed) => {
            println!(
                "{}",
                if installed {
                    "Installed!"
                } else {
                    "Already installed"
                }
            );
        }
        SetupProgress::AutomaticInstallFailed(failures) => {
            println!("{}", paint("1", "Could not automatically install:"));
            println!("{:#}", integration_failures_error(failures));
        }
        SetupProgress::ManualInstructions(text) => print!("{text}"),
        SetupProgress::ScriptFilesFailed(failures) => {
            println!("Could not install required files:");
            println!("{:#}", integration_failures_error(failures));
        }
    }
}
