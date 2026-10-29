use crate::completion::domain::entities::completion_session::CompletionSession;
use std::collections::HashMap;

/// The shell reported its env and aliases at a prompt: they replace the ones it reported before, so the aliases are
/// parsed again and the first-token suggestions listed again; the generators of the current argument stay.
pub(crate) struct ApplySessionContextUseCase;

impl ApplySessionContextUseCase {
    pub(crate) fn execute(&self, session: &mut CompletionSession, env: HashMap<String, String>, alias: Option<String>) {
        session.apply_session_context(env, alias);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::completion::domain::entities::suggestion_recency::SuggestionRecency;
    use crate::completion::domain::services::first_token_suggestions::first_token_items;
    use crate::settings::domain::value_objects::settings::Settings;
    use crate::shared::domain::value_objects::shell::Shell;
    use crate::shared::infrastructure::testing::temp_dir::TempDir;
    use std::time::Instant;

    /// A session whose wrapper env is `PATH=/wrapper/bin`, and the use case that applies a session context to it.
    /// The helpers keep their engine-era names so the five test bodies moved from the engine stay byte-identical.
    fn engine_with_wrapper_env(_dir: &TempDir) -> (CompletionSession, ApplySessionContextUseCase) {
        (
            CompletionSession::new(Settings::default(), wrapper_env(), SuggestionRecency::default()),
            ApplySessionContextUseCase,
        )
    }

    fn wrapper_env() -> HashMap<String, String> {
        HashMap::from([("PATH".to_string(), "/wrapper/bin".to_string())])
    }

    fn report_session_context(
        e: &mut CompletionSession,
        js: &mut ApplySessionContextUseCase,
        env: &[(&str, &str)],
        alias: Option<&str>,
    ) {
        let env = env
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect();
        let alias = alias.map(str::to_owned);
        js.execute(e, env, alias);
    }

    #[test]
    fn should_use_the_env_of_a_session_context_in_place_of_the_wrapper_env() {
        let dir = TempDir::new("engine-session-env");
        let (mut e, mut js) = engine_with_wrapper_env(&dir);

        report_session_context(&mut e, &mut js, &[("PATH", "/session/bin")], None);

        assert_eq!(
            e.env(),
            &HashMap::from([("PATH".to_string(), "/session/bin".to_string())])
        );
    }

    /// An empty env means the shell reported none, not that it has none.
    #[test]
    fn should_fall_back_to_the_wrapper_env_after_a_session_context_with_an_empty_env() {
        let dir = TempDir::new("engine-session-empty-env");
        let (mut e, mut js) = engine_with_wrapper_env(&dir);
        report_session_context(&mut e, &mut js, &[("PATH", "/session/bin")], None);

        report_session_context(&mut e, &mut js, &[], None);

        assert_eq!(e.env(), &wrapper_env());
    }

    #[test]
    fn should_parse_the_aliases_of_a_session_context_for_the_shell() {
        let dir = TempDir::new("engine-session-aliases");
        let (mut e, mut js) = engine_with_wrapper_env(&dir);

        report_session_context(&mut e, &mut js, &[], Some("alias ll='ls -l'"));

        assert_eq!(
            e.alias_map(Shell::Bash),
            HashMap::from([("ll".to_string(), "ls -l".to_string())])
        );
    }

    #[test]
    fn should_find_no_aliases_after_a_session_context_with_an_empty_alias_text() {
        let dir = TempDir::new("engine-session-empty-alias");
        let (mut e, mut js) = engine_with_wrapper_env(&dir);
        report_session_context(&mut e, &mut js, &[], Some("alias ll='ls -l'"));
        e.alias_map(Shell::Bash);

        report_session_context(&mut e, &mut js, &[], Some(""));

        assert_eq!(e.alias_map(Shell::Bash), HashMap::new());
    }

    #[test]
    fn should_find_no_aliases_after_a_session_context_without_alias_text() {
        let dir = TempDir::new("engine-session-no-alias");
        let (mut e, mut js) = engine_with_wrapper_env(&dir);
        report_session_context(&mut e, &mut js, &[], Some("alias ll='ls -l'"));
        e.alias_map(Shell::Bash);

        report_session_context(&mut e, &mut js, &[], None);

        assert_eq!(e.alias_map(Shell::Bash), HashMap::new());
    }

    #[test]
    fn session_update_refreshes_first_token_items() {
        // first-token-cache-ignores-session-update: `alias zzal=…` at this prompt shows `zzal` at the next one
        let mut session = CompletionSession::new(Settings::default(), HashMap::new(), SuggestionRecency::default());
        assert!(session
            .first_token_suggestions(&Instant::now, || first_token_items(&[], &HashMap::new(), &[]))
            .is_empty());
        ApplySessionContextUseCase.execute(&mut session, HashMap::new(), None);
        let aliases = HashMap::from([("zzal".to_string(), "git status".to_string())]);
        assert_eq!(
            session
                .first_token_suggestions(&Instant::now, || first_token_items(&[], &aliases, &[]))
                .iter()
                .map(|s| s.names()[0].as_str())
                .collect::<Vec<_>>(),
            ["zzal"]
        );
    }
}
