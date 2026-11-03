use crate::terminal_session::domain::value_objects::session_id::SessionId;
use std::collections::HashMap;

/// What a shell last reported through `_ pre-cmd` (SPEC §2.11): its env (UTF-8 entries only) and alias text, stamped
/// with the report time in ms. Its identity is the session's id. An empty env means "use the wrapper's env"; an empty
/// alias text means no aliases.
#[derive(Clone, Debug)]
pub(crate) struct SessionContext {
    id: SessionId,
    env: HashMap<String, String>,
    alias: Option<String>,
    reported_at_ms: u64,
}

impl SessionContext {
    pub(crate) fn new(
        id: SessionId,
        env: HashMap<String, String>,
        alias: Option<String>,
        reported_at_ms: u64,
    ) -> Self {
        SessionContext {
            id,
            env,
            alias,
            reported_at_ms,
        }
    }

    pub(crate) fn id(&self) -> &SessionId {
        &self.id
    }

    pub(crate) fn env(&self) -> &HashMap<String, String> {
        &self.env
    }

    pub(crate) fn alias(&self) -> Option<&str> {
        self.alias.as_deref()
    }

    pub(crate) fn reported_at_ms(&self) -> u64 {
        self.reported_at_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_keep_the_env_and_alias_text_a_shell_reported_and_when() {
        let id = SessionId::new("abc123".to_string()).unwrap();
        let env = HashMap::from([("A".to_string(), "1".to_string())]);

        let context =
            SessionContext::new(id.clone(), env.clone(), Some("ll='ls -l'".to_string()), 7);

        assert_eq!(
            (
                context.id(),
                context.env(),
                context.alias(),
                context.reported_at_ms()
            ),
            (&id, &env, Some("ll='ls -l'"), 7)
        );
    }
}
