//! Architecture fitness test (plan section 1): every registered module keeps the hexagonal layer rules.

/// External crates the inner layers may use. std, core and alloc are not in Cargo.toml, so they are never flagged.
const ALLOWED_EXTERNAL_CRATES: &[&str] = &["unicode_width"];

#[derive(Debug, Clone, Copy, PartialEq)]
enum Rule {
    DomainLayer,
    ApplicationLayer,
}

#[derive(Debug, PartialEq)]
struct Violation {
    line: usize,
    rule: Rule,
}

fn violations(path: &str, source: &str, external_crates: &[String]) -> Vec<Violation> {
    let rule = match path.split('/').nth(1) {
        Some("domain") => Rule::DomainLayer,
        Some("application") => Rule::ApplicationLayer,
        _ => return Vec::new(),
    };
    source
        .lines()
        .map(|line| line.find("//").map_or(line, |comment| &line[..comment]))
        .enumerate()
        .filter(|(_, code)| match rule {
            Rule::DomainLayer => {
                code.contains("::application")
                    || code.contains("::infrastructure")
                    || code.contains("crate::bootstrap")
                    || code.contains("println!")
                    || code.contains("logger::")
                    || external_crates
                        .iter()
                        .filter(|name| !ALLOWED_EXTERNAL_CRATES.contains(&name.as_str()))
                        .any(|name| code.contains(&format!("{name}::")))
            }
            Rule::ApplicationLayer => {
                code.contains("::infrastructure") || code.contains("crate::bootstrap")
            }
        })
        .map(|(index, _)| Violation { line: index + 1, rule })
        .collect()
}

mod tests {
    use super::*;

    fn violations_in(path: &str, source: &str) -> Vec<Violation> {
        let external_crates = ["anyhow", "serde", "serde_json", "unicode_width"].map(String::from);
        violations(path, source, &external_crates)
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
    fn should_not_flag_an_infrastructure_file_that_uses_the_application_layer() {
        let source = "use crate::settings::application::set_setting_use_case::SetSettingUseCase;\n";

        let found = violations_in("settings/infrastructure/settings_module.rs", source);

        assert_eq!(found, vec![]);
    }
}
