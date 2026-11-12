use crate::modules::diagnostics::application::doctor_request::DoctorRequest;
use crate::modules::diagnostics::application::doctor_verdict::DoctorVerdict;
use crate::modules::diagnostics::infrastructure::diagnostics_module::DiagnosticsModule;
use crate::modules::shared::infrastructure::os::process_info;
use crate::modules::shared::infrastructure::terminal_style::epaint;
use clap::Args;

#[derive(Args)]
pub(crate) struct DoctorArgs {
    /// Run all checks, print every result, never fix, never abort
    #[arg(long, short)]
    pub(crate) all: bool,
    /// Error on warnings
    #[arg(long, short)]
    pub(crate) strict: bool,
}

/// `doctor`: exit 1 when a check failed. It takes the module builder, not a built module, so a root run without
/// `--all` returns before anything is constructed.
pub(crate) fn run_doctor_command(
    args: DoctorArgs,
    build_module: impl FnOnce() -> DiagnosticsModule,
) -> anyhow::Result<i32> {
    if process_info::is_root() {
        eprintln!("{}", epaint("1;31", "Running doctor as root is not supported."));
        if !args.all {
            eprintln!(
                "{}",
                epaint("31", "If you know what you're doing, run the command again with --all.")
            );
            return Ok(1);
        }
    }
    let module = build_module();
    let verdict = module.run_doctor.execute(DoctorRequest::new(args.all, args.strict))?;
    Ok(i32::from(verdict == DoctorVerdict::Failed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Parser)]
    struct DoctorOnly {
        #[command(flatten)]
        args: DoctorArgs,
    }

    #[test]
    fn should_read_the_all_and_strict_flags_from_their_short_and_long_names() {
        let parsed = DoctorOnly::parse_from(["doctor", "-a", "--strict"]);

        assert_eq!((parsed.args.all, parsed.args.strict), (true, true));
    }

    #[test]
    fn should_leave_both_flags_off_by_default() {
        let parsed = DoctorOnly::parse_from(["doctor"]);

        assert_eq!((parsed.args.all, parsed.args.strict), (false, false));
    }
}
