/// What `init <shell> <phase>` prints (SPEC §5.3), without the trailing newline: the header lines, then the shell's
/// template with `{{BIN}}` quoted.
#[derive(Debug)]
pub(crate) struct InitScript {
    text: String,
}

impl InitScript {
    pub(crate) fn new(text: String) -> Self {
        InitScript { text }
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    pub(crate) fn into_string(self) -> String {
        self.text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_hold_the_rendered_text_unchanged() {
        let script =
            InitScript::new("set -g SHOULD_TABSMITH_LAUNCH 0\nbody 'tabsmith'".to_string());

        let text = script.text();

        assert_eq!(text, "set -g SHOULD_TABSMITH_LAUNCH 0\nbody 'tabsmith'");
    }

    #[test]
    fn should_give_its_text_up_as_a_string() {
        let script = InitScript::new("$TABSMITH_SHOULD_LAUNCH = 1\nbody".to_string());

        let text = script.into_string();

        assert_eq!(text, "$TABSMITH_SHOULD_LAUNCH = 1\nbody");
    }
}
