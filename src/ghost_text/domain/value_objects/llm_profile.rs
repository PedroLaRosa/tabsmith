use crate::ghost_text::domain::value_objects::ghost_text_source::BUILTIN_SOURCES;
use crate::ghost_text::domain::value_objects::llm_provider::LlmProvider;
use crate::settings::domain::value_objects::setting_value::SettingValue;

/// A user-defined LLM profile `inline.profiles.<name>` (SPEC §3.3).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LlmProfile {
    name: String,
    provider: LlmProvider,
    model: String,
    base_url: String,
    api_key_env: String,
    max_tokens: u32,
    effort: Option<String>,
}

#[cfg(test)]
pub(crate) struct LlmProfileParts {
    pub(crate) name: String,
    pub(crate) provider: LlmProvider,
    pub(crate) model: String,
    pub(crate) base_url: String,
    pub(crate) api_key_env: String,
    pub(crate) max_tokens: u32,
    pub(crate) effort: Option<String>,
}

impl LlmProfile {
    /// One profile object with per-provider defaults (SPEC §3.3).
    pub(crate) fn from_setting(name: &str, v: &SettingValue) -> Result<LlmProfile, String> {
        if BUILTIN_SOURCES.iter().any(|(b, _)| *b == name) {
            return Err(format!("'{name}' is a reserved source name"));
        }
        let o = v.as_object().ok_or("not a JSON object")?;
        let s = |k: &str| match o.get(k) {
            None | Some(SettingValue::Null) => Ok(None),
            Some(SettingValue::Text(s)) => Ok(Some(s.clone())),
            Some(_) => Err(format!("{k} must be a string")),
        };
        let provider = match s("provider")?.as_deref() {
            Some("openai") => LlmProvider::OpenAi,
            Some("anthropic") => LlmProvider::Anthropic,
            Some(p) => {
                return Err(format!(
                    "unknown provider '{p}' (expected openai or anthropic)"
                ))
            }
            None => return Err("missing provider".into()),
        };
        let model = s("model")?
            .filter(|m| !m.is_empty())
            .ok_or("missing model")?;
        let (base_url, api_key_env, max_tokens) = match provider {
            LlmProvider::OpenAi => ("https://api.openai.com/v1", "OPENAI_API_KEY", 64),
            LlmProvider::Anthropic => ("https://api.anthropic.com", "ANTHROPIC_API_KEY", 1024),
        };
        let max_tokens = match o.get("max_tokens") {
            None | Some(SettingValue::Null) => max_tokens,
            Some(n) => n
                .as_u64()
                .filter(|n| (1..=u32::MAX as u64).contains(n))
                .ok_or("max_tokens must be a positive integer")? as u32,
        };
        Ok(LlmProfile {
            name: name.to_string(),
            provider,
            model,
            base_url: s("base_url")?.unwrap_or_else(|| base_url.into()),
            api_key_env: s("api_key_env")?.unwrap_or_else(|| api_key_env.into()),
            max_tokens,
            effort: s("effort")?,
        })
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn provider(&self) -> LlmProvider {
        self.provider
    }

    pub(crate) fn model(&self) -> &str {
        &self.model
    }

    pub(crate) fn base_url(&self) -> &str {
        &self.base_url
    }

    pub(crate) fn api_key_env(&self) -> &str {
        &self.api_key_env
    }

    pub(crate) fn max_tokens(&self) -> u32 {
        self.max_tokens
    }

    pub(crate) fn effort(&self) -> Option<&str> {
        self.effort.as_deref()
    }

    #[cfg(test)]
    pub(crate) fn new(parts: LlmProfileParts) -> Self {
        LlmProfile {
            name: parts.name,
            provider: parts.provider,
            model: parts.model,
            base_url: parts.base_url,
            api_key_env: parts.api_key_env,
            max_tokens: parts.max_tokens,
            effort: parts.effort,
        }
    }
}
