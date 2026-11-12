use crate::modules::ghost_text::application::ports::llm_client::LlmClient;
use crate::modules::ghost_text::domain::value_objects::llm_profile::LlmProfile;
use crate::modules::ghost_text::domain::value_objects::llm_prompt::LlmPrompt;
use crate::modules::ghost_text::domain::value_objects::llm_provider::LlmProvider;
use crate::modules::shared::infrastructure::logger;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Duration;

/// The provider's HTTP API, for both providers; one new ureq agent per request.
pub(crate) struct HttpLlmClient;

impl LlmClient for HttpLlmClient {
    fn complete(
        &self,
        profile: &LlmProfile,
        prompt: &LlmPrompt,
        env: &HashMap<String, String>,
    ) -> Option<String> {
        let key = env
            .get(profile.api_key_env())
            .filter(|k| !k.is_empty())
            .cloned()
            .or_else(|| std::env::var(profile.api_key_env()).ok())
            .unwrap_or_default();
        let (url, headers, body) = build_request(profile, prompt, &key);
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(5)))
            .build()
            .new_agent();
        let mut req = agent.post(url.as_str());
        for (k, v) in headers {
            req = req.header(k, v);
        }
        let resp: Value = match req
            .send_json(&body)
            .and_then(|mut r| r.body_mut().read_json())
        {
            Ok(v) => v,
            Err(e) => {
                logger::warn(&format!(
                    "inline: profile '{}' request to {url} failed: {e}",
                    profile.name()
                ));
                return None;
            }
        };
        parse_response(profile.provider(), &resp)
    }
}

/// URL, headers and JSON body for one request (SPEC §8.5). Auth header omitted when `key` is empty.
fn build_request(
    profile: &LlmProfile,
    prompt: &LlmPrompt,
    key: &str,
) -> (String, Vec<(&'static str, String)>, Value) {
    let base = profile.base_url().trim_end_matches('/');
    let mut headers = Vec::new();
    match profile.provider() {
        LlmProvider::OpenAi => {
            if !key.is_empty() {
                headers.push(("authorization", format!("Bearer {key}")));
            }
            let body = json!({
                "model": profile.model(),
                "max_tokens": profile.max_tokens(),
                "messages": [{"role": "system", "content": prompt.system()}, {"role": "user", "content": prompt.user()}],
            });
            (format!("{base}/chat/completions"), headers, body)
        }
        LlmProvider::Anthropic => {
            if !key.is_empty() {
                headers.push(("x-api-key", key.to_string()));
            }
            headers.push(("anthropic-version", "2026-10-21".to_string()));
            let mut body = json!({
                "model": profile.model(),
                "max_tokens": profile.max_tokens(),
                "system": prompt.system(),
                "messages": [{"role": "user", "content": prompt.user()}],
            });
            if let Some(effort) = profile.effort() {
                body["output_config"] = json!({ "effort": effort });
            }
            (format!("{base}/v1/messages"), headers, body)
        }
    }
}

/// Raw model text from a response body; `None` on refusal or unexpected shape.
fn parse_response(provider: LlmProvider, body: &Value) -> Option<String> {
    match provider {
        LlmProvider::OpenAi => body["choices"][0]["message"]["content"]
            .as_str()
            .map(String::from),
        LlmProvider::Anthropic => {
            if body["stop_reason"] == "refusal" {
                return None;
            }
            let text: String = body["content"]
                .as_array()?
                .iter()
                .filter(|b| b["type"] == "text")
                .filter_map(|b| b["text"].as_str())
                .collect();
            Some(text)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::ghost_text::domain::services::llm_prompt_builder::SYSTEM;
    use crate::modules::ghost_text::domain::value_objects::llm_profile::LlmProfileParts;

    fn prompt(user: &str) -> LlmPrompt {
        LlmPrompt::new(SYSTEM.into(), user.into())
    }

    fn profile(provider: LlmProvider, effort: Option<&str>) -> LlmProfile {
        LlmProfile::new(LlmProfileParts {
            name: "p".into(),
            provider,
            model: "m".into(),
            base_url: "http://x/".into(),
            api_key_env: "K".into(),
            max_tokens: 7,
            effort: effort.map(String::from),
        })
    }

    #[test]
    fn request_bodies() {
        let (url, headers, body) = build_request(&profile(LlmProvider::OpenAi, None), &prompt("U"), "k");
        assert_eq!(url, "http://x/chat/completions");
        assert_eq!(headers, [("authorization", "Bearer k".to_string())]);
        assert_eq!(
            body.to_string(),
            json!({"max_tokens": 7, "messages": [{"content": SYSTEM, "role": "system"}, {"content": "U", "role": "user"}], "model": "m"}).to_string()
        );
        assert!(build_request(&profile(LlmProvider::OpenAi, None), &prompt("U"), "")
            .1
            .is_empty());

        let (url, headers, body) = build_request(&profile(LlmProvider::Anthropic, None), &prompt("U"), "k");
        assert_eq!(url, "http://x/v1/messages");
        assert_eq!(
            headers,
            [
                ("x-api-key", "k".to_string()),
                ("anthropic-version", "2026-10-21".to_string())
            ]
        );
        assert_eq!(
            body,
            json!({"model": "m", "max_tokens": 7, "system": SYSTEM, "messages": [{"role": "user", "content": "U"}]})
        );
        let (_, _, body) = build_request(&profile(LlmProvider::Anthropic, Some("low")), &prompt("U"), "k");
        assert_eq!(body["output_config"], json!({"effort": "low"}));
        for b in [
            body,
            build_request(&profile(LlmProvider::OpenAi, Some("low")), &prompt("U"), "k").2,
        ] {
            let s = b.to_string();
            assert!(
                !s.contains("temperature") && !s.contains("top_p") && !s.contains("top_k"),
                "{s}"
            );
        }
    }

    #[test]
    fn response_parsing() {
        let openai = json!({"choices": [{"message": {"role": "assistant", "content": "tus"}}]});
        assert_eq!(
            parse_response(LlmProvider::OpenAi, &openai).as_deref(),
            Some("tus")
        );
        let refused = json!({"choices": [{"message": {"content": null, "refusal": "no"}}]});
        assert_eq!(parse_response(LlmProvider::OpenAi, &refused), None);
        let anthropic = json!({"stop_reason": "end_turn", "content": [
            {"type": "thinking", "thinking": "hmm", "signature": "s"},
            {"type": "text", "text": "tu"}, {"type": "text", "text": "s"}]});
        assert_eq!(
            parse_response(LlmProvider::Anthropic, &anthropic).as_deref(),
            Some("tus")
        );
        let refusal = json!({"stop_reason": "refusal", "content": [{"type": "text", "text": "x"}]});
        assert_eq!(parse_response(LlmProvider::Anthropic, &refusal), None);
        assert_eq!(
            parse_response(LlmProvider::Anthropic, &json!({"error": {}})),
            None
        );
    }
}
