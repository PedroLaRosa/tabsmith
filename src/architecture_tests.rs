//! Architecture fitness test (plan section 1): every registered module keeps the hexagonal layer rules.

#[derive(Debug, PartialEq)]
enum Rule {
    DomainLayer,
}

#[derive(Debug, PartialEq)]
struct Violation {
    line: usize,
    rule: Rule,
}

fn violations(path: &str, source: &str) -> Vec<Violation> {
    if path.split('/').nth(1) != Some("domain") {
        return Vec::new();
    }
    source
        .lines()
        .enumerate()
        .filter(|(_, code)| {
            code.contains("::application")
                || code.contains("::infrastructure")
                || code.contains("crate::bootstrap")
        })
        .map(|(index, _)| Violation { line: index + 1, rule: Rule::DomainLayer })
        .collect()
}

mod tests {
    use super::*;

    fn violations_in(path: &str, source: &str) -> Vec<Violation> {
        violations(path, source)
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
    fn should_not_flag_an_infrastructure_file_that_uses_the_application_layer() {
        let source = "use crate::settings::application::set_setting_use_case::SetSettingUseCase;\n";

        let found = violations_in("settings/infrastructure/settings_module.rs", source);

        assert_eq!(found, vec![]);
    }
}
