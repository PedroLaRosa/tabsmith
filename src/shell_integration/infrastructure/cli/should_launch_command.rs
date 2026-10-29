use crate::shell_integration::infrastructure::shell_integration_module::ShellIntegrationModule;

/// `_ should-launch`: the reason line, then exit 0 when tabsmith launches in this shell and 1 when not (SPEC §5.2).
pub(crate) fn run_should_launch_command(module: &ShellIntegrationModule) -> anyhow::Result<i32> {
    let decision = module.decide_launch.execute();
    println!("{}", decision.reason());
    Ok(if decision.launches() { 0 } else { 1 })
}
