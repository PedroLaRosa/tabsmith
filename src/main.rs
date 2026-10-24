//! tabsmith — IDE-style autocomplete for your terminal (dropdown menu + inline ghost text),
//! driven by the Fig completion specs. See docs/SPEC.md.
#![allow(dead_code)]

#[cfg(test)]
mod architecture_tests;
mod bootstrap;
mod cli;
mod doctor;
mod engine;
mod icons;
mod inline;
mod integrations;
mod js;
mod keys;
mod menu;
mod parser;
mod pty;
mod render;
mod settings;
mod shared;
mod shell_parser;
mod spec;
mod specs;
mod suggest;
mod tracker;
mod types;

use bootstrap::cli::Cli;
use clap::Parser;
use shared::infrastructure::data_directory_from_env::data_directory_from_env;
use shared::infrastructure::logger;

fn main() {
    let cli = Cli::parse();
    let is_pty = cli.runs_pty();
    logger::init(
        if is_pty { "pty" } else { "cli" },
        cli.verbose(),
        !is_pty,
        &data_directory_from_env(),
    );

    let result = bootstrap::cli::run(cli);

    match result {
        Ok(code) => std::process::exit(code),
        Err(err) => {
            logger::error(&format!("{err:#}"));
            bootstrap::cli::print_error(&err);
            std::process::exit(1);
        }
    }
}
