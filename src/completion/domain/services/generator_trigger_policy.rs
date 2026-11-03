use crate::completion::domain::value_objects::suggestion_query::after_sep;
use crate::settings::domain::services::settings_catalog::AUTOCOMPLETE_SCRIPT_TIMEOUT;
use crate::settings::domain::value_objects::settings::Settings;
use crate::specs::domain::value_objects::generator::Generator;
use crate::specs::domain::value_objects::generator_cache_policy::GeneratorCachePolicy;
use crate::specs::domain::value_objects::generator_trigger::GeneratorTrigger;
use std::time::Duration;

/// Timeout for synchronous spec callbacks (function triggers, `getQueryTerm`).
pub(crate) const SYNC_CALL: Duration = Duration::from_secs(1);

/// Non-function trigger decision (`new` = current search term, `old` = previous); `None` for `GeneratorTrigger::Fn`.
pub(crate) fn triggered(t: &GeneratorTrigger, new: &str, old: &str) -> Option<bool> {
    let last = |s: &str, p: &str| s.rfind(p).map(|i| s[..i].chars().count()); // JS lastIndexOf (char units)
    Some(match t {
        GeneratorTrigger::Str(p) => last(new, p) != last(old, p),
        GeneratorTrigger::Change => new != old,
        GeneratorTrigger::Threshold(n) => new.chars().count() > *n && old.chars().count() <= *n,
        GeneratorTrigger::Match(list) => {
            list.iter().position(|x| x == new) != list.iter().position(|x| x == old)
        }
        GeneratorTrigger::Fn(_) => return None,
    })
}

/// Bundled `filepaths()` trigger: the directory part (up to the last separator, see [`after_sep`]) changed.
pub(crate) fn dir_changed(new: &str, old: &str, backslash: bool) -> bool {
    let dir = |s: &str| s[..after_sep(s, backslash)].to_string();
    dir(new) != dir(old)
}

/// `[cacheByDirectory ? cwd : "", key].join(",")`.
pub(crate) fn cache_key(cfg: &GeneratorCachePolicy, cwd: &str, key: String) -> String {
    format!("{},{key}", if cfg.cache_by_directory() { cwd } else { "" })
}

/// `filepaths` / `folders` templates behave like the bundled `filepaths()` helper (trigger, query term).
pub(crate) fn is_path_template(g: &Generator) -> bool {
    g.template()
        .iter()
        .any(|t| t == "filepaths" || t == "folders")
}

/// `autocomplete.scriptTimeout` (5000 ms; Windows 20000).
pub(crate) fn script_timeout(settings: &Settings) -> u64 {
    settings.f64(AUTOCOMPLETE_SCRIPT_TIMEOUT).max(0.0) as u64
}

/// How long a fired generator waits before it runs: a `debounce` of 0 waits 200 ms, none runs at once.
pub(crate) fn effective_debounce(debounce: Option<u64>) -> Option<u64> {
    debounce.map(|ms| if ms > 0 { ms } else { 200 })
}

/// A run's whole deadline in ms: its debounce, twice the larger of the floor and the generator's own script timeout,
/// and a second.
/// ponytail: plain `+`/`*`, so a huge `scriptTimeout` overflows as it always did (GEN-GT9).
pub(crate) fn run_budget(debounce: Option<u64>, floor: u64, generator_timeout: Option<u64>) -> u64 {
    debounce.unwrap_or(0) + 2 * floor.max(generator_timeout.unwrap_or(0)) + 1000
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::domain::value_objects::setting_number::SettingNumber;
    use crate::settings::domain::value_objects::setting_value::SettingValue;

    #[test]
    fn triggers() {
        let s = GeneratorTrigger::Str("/".into());
        assert_eq!(triggered(&s, "src/", "src"), Some(true));
        assert_eq!(triggered(&s, "src/a", "src/"), Some(false));
        assert_eq!(triggered(&GeneratorTrigger::Change, "a", "a"), Some(false));
        assert_eq!(triggered(&GeneratorTrigger::Change, "ab", "a"), Some(true));
        assert_eq!(triggered(&GeneratorTrigger::Threshold(2), "abc", "ab"), Some(true));
        assert_eq!(triggered(&GeneratorTrigger::Threshold(2), "ab", "abc"), Some(false));
        assert_eq!(
            triggered(&GeneratorTrigger::Threshold(2), "abcd", "abc"),
            Some(false)
        );
        let m = GeneratorTrigger::Match(vec!["x".into(), "y".into()]);
        assert_eq!(triggered(&m, "y", "x"), Some(true));
        assert_eq!(triggered(&m, "z", "w"), Some(false));
        assert_eq!(triggered(&GeneratorTrigger::Fn(1), "a", "b"), None);
        assert!(dir_changed("src/a", "lib/a", false));
        assert!(!dir_changed("src/ab", "src/a", false));
        assert!(dir_changed("src/", "src", false));
        // windows-backslash-paths: `\` separates in pwsh only (POSIX: it is the escape char)
        assert!(dir_changed("src\\", "src", true));
        assert!(!dir_changed("src\\ab", "src\\a", true));
        assert!(!dir_changed("my\\ ", "my", false));
    }

    #[test]
    fn should_read_the_script_timeout_floor_in_whole_milliseconds() {
        let fractional = Settings::default().with(
            AUTOCOMPLETE_SCRIPT_TIMEOUT,
            SettingValue::Number(SettingNumber::from_f64(7500.5).unwrap()),
        );
        let negative = Settings::default().with(
            AUTOCOMPLETE_SCRIPT_TIMEOUT,
            SettingValue::Number(SettingNumber::from(-1i64)),
        );

        let floors = (script_timeout(&fractional), script_timeout(&negative));

        assert_eq!(floors, (7500, 0));
    }

    #[test]
    fn should_debounce_a_zero_debounce_for_200_ms() {
        let debounces = [Some(0), Some(350), None];

        let effective = debounces.map(effective_debounce);

        assert_eq!(effective, [Some(200), Some(350), None]);
    }

    #[test]
    fn should_budget_a_run_for_its_debounce_twice_its_script_timeout_and_a_second() {
        let runs = [(Some(200), 5000, Some(7000)), (None, 5000, None), (None, 5000, Some(10))];

        let budgets = runs.map(|(debounce, floor, timeout)| run_budget(debounce, floor, timeout));

        assert_eq!(budgets, [15200, 11000, 11000]);
    }
}
