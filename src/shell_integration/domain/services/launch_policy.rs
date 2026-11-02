use crate::shell_integration::domain::value_objects::launch_decision::LaunchDecision;
use crate::shell_integration::domain::value_objects::launch_facts::LaunchFacts;

/// SPEC §5.2, first match wins.
pub(crate) fn decide(facts: &LaunchFacts) -> LaunchDecision {
    let has = |k: &str| facts.has(k);
    let is = |k: &str, v: &str| facts.is(k, v);
    let (launch, why) = if has("TABSMITH_FORCE_LAUNCH") {
        (true, "TABSMITH_FORCE_LAUNCH")
    } else if has("TABSMITH_TERM_DISABLED") {
        (false, "TABSMITH_TERM_DISABLED")
    } else if has("PROCESS_LAUNCHED_BY_TABSMITH") {
        (false, "PROCESS_LAUNCHED_BY_TABSMITH")
    } else if has("TABSMITH_TERM") {
        if has("TMUX") && !has("TABSMITH_TERM_TMUX") {
            (true, "new tmux pane")
        } else {
            (false, "already inside tabsmith")
        }
    } else if has("INSIDE_EMACS") {
        (false, "INSIDE_EMACS")
    } else if is("TERM_PROGRAM", "WarpTerminal") {
        (false, "TERM_PROGRAM = WarpTerminal")
    } else if is("__CFBundleIdentifier", "com.vandyke.SecureCRT") {
        (false, "__CFBundleIdentifier = com.vandyke.SecureCRT")
    } else if has("__PWSH_LOGIN_CHECKED") {
        (false, "__PWSH_LOGIN_CHECKED")
    } else if has("CI") {
        (false, "In CI")
    } else if has("VSCODE_RESOLVING_ENVIRONMENT") {
        (false, "VSCODE_RESOLVING_ENVIRONMENT")
    } else if cfg!(unix) && (!has("TERM") || is("TERM", "dumb")) {
        (false, "TERM is unset or dumb")
    } else if !facts.stdin_is_terminal() {
        (
            false,
            if cfg!(windows) {
                "stdin is not a console"
            } else {
                "stdin is not a tty"
            },
        )
    } else {
        (true, "launching")
    };
    LaunchDecision::new(
        launch,
        format!("{} {why}", if launch { "✅" } else { "❌" }),
    )
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn launch_decision(env: &HashMap<String, String>, stdin_ok: bool) -> (bool, String) {
        let d = decide(&LaunchFacts::new(env.clone(), stdin_ok));
        (d.launches(), d.reason().to_string())
    }

    #[cfg(unix)]
    #[test]
    #[allow(clippy::type_complexity)]
    fn integrations_should_launch_table() {
        let base = [("TERM", "xterm-256color")];
        let cases: &[(&[(&str, &str)], bool, bool, &str)] = &[
            (&[], true, true, "✅ launching"),
            (&[], false, false, "❌ stdin is not a tty"),
            (
                &[("TABSMITH_FORCE_LAUNCH", "1"), ("CI", "1")],
                false,
                true,
                "✅ TABSMITH_FORCE_LAUNCH",
            ),
            (
                &[("TABSMITH_TERM_DISABLED", "1")],
                true,
                false,
                "❌ TABSMITH_TERM_DISABLED",
            ),
            (
                &[("PROCESS_LAUNCHED_BY_TABSMITH", "1")],
                true,
                false,
                "❌ PROCESS_LAUNCHED_BY_TABSMITH",
            ),
            (
                &[("TABSMITH_TERM", "0.1.0")],
                true,
                false,
                "❌ already inside tabsmith",
            ),
            (
                &[("TABSMITH_TERM", "0.1.0"), ("TMUX", "/tmp/t")],
                true,
                true,
                "✅ new tmux pane",
            ),
            (
                &[
                    ("TABSMITH_TERM", "0.1.0"),
                    ("TMUX", "/tmp/t"),
                    ("TABSMITH_TERM_TMUX", "0.1.0"),
                ],
                true,
                false,
                "❌ already inside tabsmith",
            ),
            (&[("INSIDE_EMACS", "29")], true, false, "❌ INSIDE_EMACS"),
            (
                &[("TERM_PROGRAM", "WarpTerminal")],
                true,
                false,
                "❌ TERM_PROGRAM = WarpTerminal",
            ),
            (&[("TERM_PROGRAM", "iTerm.app")], true, true, "✅ launching"),
            (
                &[("__CFBundleIdentifier", "com.vandyke.SecureCRT")],
                true,
                false,
                "❌ __CFBundleIdentifier = com.vandyke.SecureCRT",
            ),
            (
                &[("__PWSH_LOGIN_CHECKED", "1")],
                true,
                false,
                "❌ __PWSH_LOGIN_CHECKED",
            ),
            (&[("CI", "true")], true, false, "❌ In CI"),
            (
                &[("VSCODE_RESOLVING_ENVIRONMENT", "1")],
                true,
                false,
                "❌ VSCODE_RESOLVING_ENVIRONMENT",
            ),
            (&[("TERM", "dumb")], true, false, "❌ TERM is unset or dumb"),
            (&[("SSH_TTY", "/dev/ttys001")], true, true, "✅ launching"),
        ];
        for (vars, stdin_ok, launch, why) in cases {
            let env: HashMap<String, String> = base
                .iter()
                .chain(vars.iter())
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            assert_eq!(
                launch_decision(&env, *stdin_ok),
                (*launch, why.to_string()),
                "{vars:?}"
            );
        }
        assert_eq!(
            launch_decision(&HashMap::new(), true),
            (false, "❌ TERM is unset or dumb".to_string())
        );
    }
}
