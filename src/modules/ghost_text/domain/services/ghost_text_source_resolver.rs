use crate::modules::ghost_text::domain::value_objects::ghost_text_source::GhostTextSource;
use crate::modules::ghost_text::domain::value_objects::llm_profile::LlmProfile;
use crate::modules::settings::domain::services::settings_catalog::INLINE_SOURCE;
use crate::modules::settings::domain::value_objects::settings::Settings;

const PROFILES_PREFIX: &str = "inline.profiles.";

/// Every `inline.profiles.*` entry, parsed (`Err(reason)` for invalid objects), sorted by name.
pub(crate) fn profiles(settings: &Settings) -> Vec<(String, Result<LlmProfile, String>)> {
    let mut v: Vec<_> = settings
        .with_prefix(PROFILES_PREFIX)
        .map(|(n, val)| (n.to_string(), LlmProfile::from_setting(n, val)))
        .collect();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v
}

/// Resolve `inline.source` (unset → HistoryAndSpecs). `Err` when it names a missing/invalid profile.
pub(crate) fn source(settings: &Settings) -> Result<GhostTextSource, String> {
    match settings.string(INLINE_SOURCE).unwrap_or_default() {
        "history+specs" => Ok(GhostTextSource::HistoryAndSpecs),
        "history" => Ok(GhostTextSource::History),
        "specs" => Ok(GhostTextSource::Specs),
        name => match settings.get(&format!("{PROFILES_PREFIX}{name}")) {
            None => Err(format!("profile '{name}' not found")),
            Some(v) => LlmProfile::from_setting(name, v)
                .map(GhostTextSource::Profile)
                .map_err(|e| format!("profile '{name}' is invalid: {e}")),
        },
    }
}

pub(crate) fn selected_source(s: &Settings) -> &str {
    s.string(INLINE_SOURCE).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::ghost_text::domain::value_objects::llm_profile::LlmProfileParts;
    use crate::modules::ghost_text::domain::value_objects::llm_provider::LlmProvider;
    use crate::modules::settings::infrastructure::adapters::setting_value_json::settings_from_json;
    use serde_json::{json, Value};

    fn settings(v: Value) -> Settings {
        settings_from_json(v.as_object().unwrap().clone())
    }

    #[test]
    fn profiles_and_source() {
        let s = settings(json!({
            "inline.source": "claude",
            "inline.profiles.claude": {"provider": "anthropic", "model": "claude-opus-5-5", "effort": "low"},
            "inline.profiles.local": {"provider": "openai", "model": "llama", "base_url": "http://localhost:11434/v1",
                                      "max_tokens": 32, "api_key_env": "NONE"},
            "inline.profiles.bad": {"provider": "gemini", "model": "x"},
            "inline.profiles.nomodel": {"provider": "openai"},
            "inline.profiles.str": "openai",
            "inline.profiles.history": {"provider": "openai", "model": "x"},
            "inline.profiles.tokens": {"provider": "openai", "model": "x", "max_tokens": 0},
            "inline.profiles.url": {"provider": "openai", "model": "x", "base_url": 1},
        }));
        let p = profiles(&s);
        let names: Vec<_> = p.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(
            names,
            ["bad", "claude", "history", "local", "nomodel", "str", "tokens", "url"]
        );
        let get = |n: &str| p.iter().find(|(k, _)| k == n).unwrap().1.clone();
        let claude = get("claude").unwrap();
        assert_eq!(
            claude,
            LlmProfile::new(LlmProfileParts {
                name: "claude".into(),
                provider: LlmProvider::Anthropic,
                model: "claude-opus-5-5".into(),
                base_url: "https://api.anthropic.com".into(),
                api_key_env: "ANTHROPIC_API_KEY".into(),
                max_tokens: 1024,
                effort: Some("low".into()),
            })
        );
        let local = get("local").unwrap();
        assert_eq!(
            (
                local.base_url(),
                local.api_key_env(),
                local.max_tokens()
            ),
            ("http://localhost:11434/v1", "NONE", 32)
        );
        assert!(get("bad").unwrap_err().contains("gemini"));
        assert_eq!(get("nomodel").unwrap_err(), "missing model");
        assert_eq!(get("str").unwrap_err(), "not a JSON object");
        assert!(get("history").unwrap_err().contains("reserved"));
        assert!(get("tokens").unwrap_err().contains("max_tokens"));
        assert_eq!(get("url").unwrap_err(), "base_url must be a string");

        assert_eq!(source(&s), Ok(GhostTextSource::Profile(claude)));
        let openai = settings(
            json!({"inline.source": "o", "inline.profiles.o": {"provider": "openai", "model": "g"}}),
        );
        let Ok(GhostTextSource::Profile(o)) = source(&openai) else {
            panic!()
        };
        assert_eq!(
            (o.base_url(), o.api_key_env(), o.max_tokens()),
            ("https://api.openai.com/v1", "OPENAI_API_KEY", 64)
        );
        assert_eq!(source(&settings(json!({}))), Ok(GhostTextSource::HistoryAndSpecs));
        assert_eq!(
            source(&settings(json!({"inline.source": "history"}))),
            Ok(GhostTextSource::History)
        );
        assert_eq!(
            source(&settings(json!({"inline.source": "specs"}))),
            Ok(GhostTextSource::Specs)
        );
        assert!(source(&settings(json!({"inline.source": "nope"})))
            .unwrap_err()
            .contains("not found"));
        let bad =
            settings(json!({"inline.source": "b", "inline.profiles.b": {"provider": "openai"}}));
        assert!(source(&bad).unwrap_err().contains("invalid"));
    }
}
