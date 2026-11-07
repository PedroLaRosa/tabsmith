//! tabsmith — IDE-style autocomplete for your terminal (dropdown menu + inline ghost text),
//! driven by the Fig completion specs. See docs/SPEC.md.

#[cfg(test)]
mod architecture_tests;
mod bootstrap;
mod modules;

fn main() {
    let cli: bootstrap::cli::Cli = clap::Parser::parse();
    let is_pty = cli.runs_pty();
    modules::shared::infrastructure::logger::init(
        if is_pty { "pty" } else { "cli" },
        cli.verbose(),
        !is_pty,
        &modules::shared::infrastructure::data_directory_from_env::data_directory_from_env(),
    );

    let result = bootstrap::cli::run(cli);

    match result {
        Ok(code) => std::process::exit(code),
        Err(err) => {
            modules::shared::infrastructure::logger::error(&format!("{err:#}"));
            bootstrap::cli::print_error(&err);
            std::process::exit(1);
        }
    }
}
