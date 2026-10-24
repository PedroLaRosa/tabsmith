//! Architecture fitness test (plan section 1): every registered module keeps the hexagonal layer rules.

use std::fs;
use std::path::{Path, PathBuf};

/// Modules that follow the hexagonal layout; each phase registers the module it finishes. `bootstrap` is the
/// composition root and is never registered.
const REGISTERED_MODULES: &[&str] = &[];

/// External crates the inner layers may use. std, core and alloc are not in Cargo.toml, so they are never flagged.
const ALLOWED_EXTERNAL_CRATES: &[&str] = &["unicode_width"];

#[derive(Debug, Clone, Copy, PartialEq)]
enum Rule {
    DomainLayer,
    ApplicationLayer,
    ModuleBoundary,
    SharedKernel,
}

impl Rule {
    fn description(self) -> &'static str {
        match self {
            Rule::DomainLayer => {
                "domain uses application, infrastructure, bootstrap, an external crate, println!/eprintln! or the logger"
            }
            Rule::ApplicationLayer => "application uses infrastructure, bootstrap or an external crate",
            Rule::ModuleBoundary => "uses another module's use case or infrastructure",
            Rule::SharedKernel => "shared uses a crate module other than shared",
        }
    }
}

#[derive(Debug, PartialEq)]
struct Violation {
    line: usize,
    rule: Rule,
}

/// One `src/<file>:<line>: <rule>: <code>` entry per violation in the registered modules under `src`.
fn registered_module_violations(src: &Path, external_crates: &[String]) -> Vec<String> {
    let mut report = Vec::new();
    for module in REGISTERED_MODULES {
        for file in rust_files(&src.join(module)) {
            let path = relative_path(src, &file);
            let source = fs::read_to_string(&file).unwrap_or_else(|err| panic!("read {path}: {err}"));
            let lines: Vec<&str> = source.lines().collect();
            for Violation { line, rule } in violations(&path, &source, external_crates) {
                let code = lines[line - 1].trim();
                report.push(format!("src/{path}:{line}: {}: {code}", rule.description()));
            }
        }
    }
    report
}

fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let entries = fs::read_dir(dir).unwrap_or_else(|err| panic!("read {}: {err}", dir.display()));
    let mut files = Vec::new();
    for entry in entries {
        let path = entry.expect("read directory entry").path();
        if path.is_dir() {
            files.extend(rust_files(&path));
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
    files.sort();
    files
}

/// `file` relative to `src`, `/`-separated on every platform.
fn relative_path(src: &Path, file: &Path) -> String {
    let relative = file.strip_prefix(src).expect("file under src");
    let components: Vec<_> = relative.components().map(|part| part.as_os_str().to_string_lossy()).collect();
    components.join("/")
}

/// The rules `source` breaks, line by line; `path` is relative to `src` and `/`-separated (`<module>/<layer>/..`).
fn violations(path: &str, source: &str, external_crates: &[String]) -> Vec<Violation> {
    let mut components = path.split('/');
    let module = components.next().unwrap_or_default();
    let layer = components.next();
    let mut open_braces = Vec::new();
    let mut found = Vec::new();
    for (index, line) in source.lines().enumerate() {
        let code = line.find("//").map_or(line, |comment| &line[..comment]);
        let paths = paths_in(code, &mut open_braces);
        let checks = [
            (
                Rule::DomainLayer,
                layer == Some("domain") && breaks_domain_layer(code, &paths, external_crates),
            ),
            (
                Rule::ApplicationLayer,
                layer == Some("application") && breaks_application_layer(&paths, external_crates),
            ),
            (Rule::ModuleBoundary, crosses_module_boundary(module, &paths)),
            (Rule::SharedKernel, module == "shared" && leaves_the_shared_kernel(&paths)),
        ];
        let broken = checks.into_iter().filter(|(_, is_broken)| *is_broken);
        found.extend(broken.map(|(rule, _)| Violation { line: index + 1, rule }));
    }
    found
}

/// The segments of every `a::b::c` path in `code`; `::serde::X` and `serde::{` both start at `serde`. A member of
/// a `use a::{..}` group gets the `a` prefix. `open_braces` holds each open brace's prefix across lines (empty for
/// a block).
// ponytail: a line scanner, not a Rust parser. String literals are read as code, only `//` comments are skipped,
// and `super::`-relative paths are not resolved. Switch to a real parser if a false hit or a miss ever shows up.
fn paths_in<'a>(code: &'a str, open_braces: &mut Vec<Vec<&'a str>>) -> Vec<Vec<&'a str>> {
    let mut paths = Vec::new();
    for piece in code.split_inclusive(|c: char| !is_path_char(c)) {
        let chunk = piece.trim_end_matches(|c: char| !is_path_char(c));
        let segments: Vec<&str> = chunk.split("::").filter(|segment| !segment.is_empty()).collect();
        if piece.ends_with("::{") {
            open_braces.push(segments);
            continue;
        }
        let in_use_group = open_braces.iter().any(|prefix| !prefix.is_empty());
        if !segments.is_empty() && (chunk.contains("::") || in_use_group) {
            paths.push(open_braces.concat().into_iter().chain(segments).collect());
        }
        if piece.ends_with('{') {
            open_braces.push(Vec::new());
        } else if piece.ends_with('}') {
            open_braces.pop();
        }
    }
    paths
}

fn is_path_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == ':'
}

/// The domain is std plus unicode-width: no other layer, no composition root, no external crate, no output.
fn breaks_domain_layer(code: &str, paths: &[Vec<&str>], external_crates: &[String]) -> bool {
    code.contains("println!")
        || breaks_application_layer(paths, external_crates)
        || paths.iter().any(|path| path.contains(&"application") || path.contains(&"logger"))
}

fn breaks_application_layer(paths: &[Vec<&str>], external_crates: &[String]) -> bool {
    paths.iter().any(|path| {
        path.contains(&"infrastructure")
            || path.starts_with(&["crate", "bootstrap"])
            || is_forbidden_external_crate(path[0], external_crates)
    })
}

fn is_forbidden_external_crate(root: &str, external_crates: &[String]) -> bool {
    !ALLOWED_EXTERNAL_CRATES.contains(&root) && external_crates.iter().any(|name| name == root)
}

/// Another module's use cases and adapters are reached only through bootstrap.
fn crosses_module_boundary(own_module: &str, paths: &[Vec<&str>]) -> bool {
    paths.iter().any(|path| match path.as_slice() {
        ["crate", module, "infrastructure", ..] => *module != own_module,
        ["crate", module, "application", rest @ ..] => {
            *module != own_module && rest.iter().any(|segment| segment.ends_with("_use_case"))
        }
        _ => false,
    })
}

/// The kernel never imports a feature or legacy module.
fn leaves_the_shared_kernel(paths: &[Vec<&str>]) -> bool {
    paths.iter().any(|path| matches!(path.as_slice(), ["crate", module, ..] if *module != "shared"))
}

/// Crate names (`-` read as `_`) from `[dependencies]` and every `[target.*.dependencies]` table.
fn external_crates(cargo_toml: &str) -> Vec<String> {
    let mut in_dependency_table = false;
    let mut names = Vec::new();
    for line in cargo_toml.lines().map(str::trim) {
        if line.starts_with('[') {
            in_dependency_table = line == "[dependencies]"
                || (line.starts_with("[target.") && line.ends_with(".dependencies]"));
        } else if let Some((name, _)) = line.split_once('=').filter(|_| in_dependency_table) {
            names.push(name.trim().replace('-', "_"));
        }
    }
    names
}

mod tests {
    use super::*;

    fn violations_in(path: &str, source: &str) -> Vec<Violation> {
        let external_crates = ["anyhow", "serde", "serde_json", "unicode_width"].map(String::from);
        violations(path, source, &external_crates)
    }

    #[test]
    fn should_keep_every_registered_module_inside_its_layer_rules() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let cargo_toml = fs::read_to_string(root.join("Cargo.toml")).expect("read Cargo.toml");

        let report = registered_module_violations(&root.join("src"), &external_crates(&cargo_toml));

        assert!(report.is_empty(), "architecture rules broken:\n{}", report.join("\n"));
    }

    #[test]
    fn should_flag_a_domain_file_that_uses_another_layer() {
        let source = "use super::setting_key::SettingKey;\n\
                      use crate::shared::application::ports::clock::Clock;\n\
                      use crate::settings::infrastructure::cli::SettingsArgs;\n";

        let found = violations_in("settings/domain/settings.rs", source);

        assert_eq!(
            found,
            vec![
                Violation { line: 2, rule: Rule::DomainLayer },
                Violation { line: 3, rule: Rule::DomainLayer },
            ]
        );
    }

    #[test]
    fn should_flag_a_domain_file_that_uses_the_composition_root() {
        let source = "use crate::bootstrap::cli::Cli;\n";

        let found = violations_in("settings/domain/settings.rs", source);

        assert_eq!(found, vec![Violation { line: 1, rule: Rule::DomainLayer }]);
    }

    #[test]
    fn should_flag_a_domain_file_that_uses_an_external_crate() {
        let source = "use serde::Serialize;\n\
                      let width = text.len();\n\
                      let text = serde_json::to_string(&value);\n";

        let found = violations_in("settings/domain/settings.rs", source);

        assert_eq!(
            found,
            vec![
                Violation { line: 1, rule: Rule::DomainLayer },
                Violation { line: 3, rule: Rule::DomainLayer },
            ]
        );
    }

    #[test]
    fn should_flag_a_domain_file_that_prints_or_logs() {
        let source = "println!(\"{key}\");\n\
                      let key = SettingKey::new(text)?;\n\
                      eprintln!(\"{err}\");\n\
                      crate::logger::warn(\"unknown key\");\n";

        let found = violations_in("settings/domain/settings.rs", source);

        assert_eq!(
            found,
            vec![
                Violation { line: 1, rule: Rule::DomainLayer },
                Violation { line: 3, rule: Rule::DomainLayer },
                Violation { line: 4, rule: Rule::DomainLayer },
            ]
        );
    }

    #[test]
    fn should_ignore_references_inside_line_comments() {
        let source = "//! Built by crate::bootstrap, printed with println!.\n\
                      /// Same rules as crate::settings::application::set_setting_use_case.\n\
                      let width = text.len(); // serde::Serialize would leak here\n";

        let found = violations_in("settings/domain/settings.rs", source);

        assert_eq!(found, vec![]);
    }

    #[test]
    fn should_read_each_member_of_a_use_group_under_the_group_prefix() {
        let source = "use crate::{\n    \
                      shared::domain::value_objects::shell::Shell,\n    \
                      settings::domain::settings::Settings,\n\
                      };\n\
                      fn render(shell: Shell) {\n\
                      }\n";

        let found = violations_in("shared/infrastructure/json_document_store.rs", source);

        assert_eq!(found, vec![Violation { line: 3, rule: Rule::SharedKernel }]);
    }

    #[test]
    fn should_match_whole_path_segments_only() {
        let source = "use crate::settings::domain::application_state::ApplicationState;\n\
                      let text = my_serde::to_text(&state);\n\
                      let probe = crate::bootstrapping::Probe;\n\
                      let name = event_logger::name();\n";

        let found = violations_in("settings/domain/settings.rs", source);

        assert_eq!(found, vec![]);
    }

    #[test]
    fn should_not_flag_a_domain_file_that_uses_std_or_unicode_width() {
        let source = "use std::fmt;\n\
                      use unicode_width::UnicodeWidthStr;\n\
                      let width = unicode_width::UnicodeWidthChar::width(c);\n";

        let found = violations_in("dropdown/domain/menu.rs", source);

        assert_eq!(found, vec![]);
    }

    #[test]
    fn should_flag_an_application_file_that_uses_infrastructure_or_the_composition_root() {
        let source = "use crate::settings::domain::settings::Settings;\n\
                      use crate::settings::infrastructure::cli::SettingsArgs;\n\
                      use crate::bootstrap::cli::Cli;\n";

        let found = violations_in("settings/application/set_setting_use_case.rs", source);

        assert_eq!(
            found,
            vec![
                Violation { line: 2, rule: Rule::ApplicationLayer },
                Violation { line: 3, rule: Rule::ApplicationLayer },
            ]
        );
    }

    #[test]
    fn should_flag_an_application_file_that_uses_an_external_crate_other_than_unicode_width() {
        let source = "use anyhow::Result;\n\
                      use unicode_width::UnicodeWidthStr;\n\
                      let value = serde_json::json!({});\n";

        let found = violations_in("settings/application/set_setting_use_case.rs", source);

        assert_eq!(
            found,
            vec![
                Violation { line: 1, rule: Rule::ApplicationLayer },
                Violation { line: 3, rule: Rule::ApplicationLayer },
            ]
        );
    }

    #[test]
    fn should_flag_a_file_that_uses_another_modules_use_case_or_infrastructure() {
        let source = "use crate::completion::domain::suggestion::Suggestion;\n\
                      use crate::completion::application::rank_suggestions_use_case::RankSuggestionsUseCase;\n\
                      let history = crate::history::infrastructure::jsonl_history_repository::open(path);\n";

        let found = violations_in("ghost_text/infrastructure/ghost_text_module.rs", source);

        assert_eq!(
            found,
            vec![
                Violation { line: 2, rule: Rule::ModuleBoundary },
                Violation { line: 3, rule: Rule::ModuleBoundary },
            ]
        );
    }

    #[test]
    fn should_flag_a_shared_file_that_uses_a_feature_or_legacy_module() {
        let source = "use crate::shared::application::ports::clock::Clock;\n\
                      use crate::settings::domain::settings::Settings;\n\
                      let now = crate::types::now_ms();\n";

        let found = violations_in("shared/infrastructure/system_clock.rs", source);

        assert_eq!(
            found,
            vec![
                Violation { line: 2, rule: Rule::SharedKernel },
                Violation { line: 3, rule: Rule::SharedKernel },
            ]
        );
    }

    #[test]
    fn should_read_external_crate_names_from_every_dependency_table() {
        let cargo_toml = "[package]\nname = \"tabsmith\"\n\n\
                          [features]\ndefault = [\"embed-specs\"]\n\n\
                          [dependencies]\nanyhow = \"1\"\nunicode-width = \"0.2\"\n\n\
                          [target.'cfg(unix)'.dependencies]\nsignal-hook = \"0.3\"\n\n\
                          [target.'cfg(windows)'.dependencies]\n\
                          windows-sys = { version = \"0.59\", features = [\"Win32_Foundation\"] }\n\n\
                          [profile.release]\nlto = true\n";

        let names = external_crates(cargo_toml);

        assert_eq!(names, ["anyhow", "unicode_width", "signal_hook", "windows_sys"]);
    }

    #[test]
    fn should_not_flag_an_application_file_that_uses_the_domain_and_ports() {
        let source = "use super::ports::spec_bundle_installer::SpecBundleInstaller;\n\
                      use crate::settings::domain::settings::Settings;\n\
                      use crate::shared::application::ports::clock::Clock;\n\
                      use std::rc::Rc;\n";

        let found = violations_in("shell_integration/application/install_integration_use_case.rs", source);

        assert_eq!(found, vec![]);
    }

    #[test]
    fn should_flag_another_modules_use_case_imported_as_a_module() {
        let source = "use crate::specs::application::install_spec_bundle_use_case;\n";

        let found = violations_in("shell_integration/infrastructure/shell_integration_module.rs", source);

        assert_eq!(found, vec![Violation { line: 1, rule: Rule::ModuleBoundary }]);
    }

    #[test]
    fn should_not_flag_a_file_that_uses_its_own_use_cases_or_another_modules_domain_and_ports() {
        let source = "use crate::ghost_text::application::suggest_ghost_text_use_case::SuggestGhostTextUseCase;\n\
                      use crate::ghost_text::infrastructure::ghost_text_cache::GhostTextCache;\n\
                      use crate::completion::domain::suggestion::Suggestion;\n\
                      use crate::history::application::ports::history_source::HistorySource;\n";

        let found = violations_in("ghost_text/infrastructure/ghost_text_module.rs", source);

        assert_eq!(found, vec![]);
    }

    #[test]
    fn should_not_flag_a_shared_file_that_uses_the_kernel_or_an_external_crate() {
        let source = "use crate::shared::domain::value_objects::data_directory::DataDirectory;\n\
                      use serde_json::Value;\n\
                      let text = std::fs::read_to_string(path)?;\n";

        let found = violations_in("shared/infrastructure/json_document_store.rs", source);

        assert_eq!(found, vec![]);
    }

    #[test]
    fn should_not_flag_an_infrastructure_file_that_uses_the_application_layer() {
        let source = "use crate::settings::application::set_setting_use_case::SetSettingUseCase;\n";

        let found = violations_in("settings/infrastructure/settings_module.rs", source);

        assert_eq!(found, vec![]);
    }
}
