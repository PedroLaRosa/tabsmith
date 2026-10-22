//! `tabsmith doctor` (SPEC §2.8). OWNER: integrations agent.
use clap::Args;

#[derive(Args)]
pub struct DoctorArgs {
    /// Run all checks, print every result, never fix, never abort
    #[arg(long, short)]
    pub all: bool,
    /// Error on warnings
    #[arg(long, short)]
    pub strict: bool,
}

pub fn run(args: DoctorArgs) -> anyhow::Result<i32> {
    let _ = args;
    todo!("integrations agent")
}
