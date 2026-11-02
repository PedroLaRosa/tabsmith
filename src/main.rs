//! tabsmith — IDE-style autocomplete for your terminal (dropdown menu + inline ghost text),
//! driven by the Fig completion specs. See docs/SPEC.md.
#![allow(dead_code)]

#[cfg(test)]
mod architecture_tests;
mod bootstrap;
mod cli;
mod completion;
mod diagnostics;
mod doctor;
mod dropdown;
mod ghost_text;
mod history;
mod settings;
mod shared;
mod shell_integration;
mod specs;
mod terminal_session;
mod types;

fn main() {
    let cli: bootstrap::cli::Cli = clap::Parser::parse();
    let is_pty = cli.runs_pty();
    shared::infrastructure::logger::init(
        if is_pty { "pty" } else { "cli" },
        cli.verbose(),
        !is_pty,
        &shared::infrastructure::data_directory_from_env::data_directory_from_env(),
    );

    let result = bootstrap::cli::run(cli);

    match result {
        Ok(code) => std::process::exit(code),
        Err(err) => {
            shared::infrastructure::logger::error(&format!("{err:#}"));
            bootstrap::cli::print_error(&err);
            std::process::exit(1);
        }
    }
}
