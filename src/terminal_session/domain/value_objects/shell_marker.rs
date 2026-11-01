use crate::terminal_session::domain::value_objects::pwsh_prediction_color::PwshPredictionColor;
use crate::terminal_session::domain::value_objects::shell_suggestion_color::ShellSuggestionColor;
use std::path::PathBuf;

/// One OSC 697 payload (SPEC §5.1), read: a bare token or `Key=Value` split at the first `=`. Each key keeps the
/// trimming rule it always had: `NewCmd`, `Dir` and `PwshPrompts` read the value untrimmed, every other key trimmed.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ShellMarker {
    StartPrompt,
    EndPrompt,
    /// The session id the script sent, untrimmed; `None` for a bare `NewCmd`.
    NewCmd(Option<String>),
    PreExec,
    NoHistory,
    OscLock(String),
    OscUnlock(String),
    /// Untrimmed; a bare `Dir` is the empty path.
    Dir(PathBuf),
    ExitCode(Option<i32>),
    /// The name or path the shell reported (trimmed, so a bare `Shell` is empty).
    Shell(String),
    Pid(Option<i32>),
    ZshAutosuggestionColor(Option<ShellSuggestionColor>),
    FishSuggestionColor(Option<ShellSuggestionColor>),
    PwshPredictionColor(Option<PwshPredictionColor>),
    PwshListView(bool),
    /// `<ContinuationPrompt>\x1f<PromptText>…`, untrimmed (the prompts end in spaces); empty prompt texts dropped.
    PwshPrompts {
        continuation: String,
        prompt_texts: Vec<String>,
    },
    /// An empty value, an unknown key, or a key that sets nothing the wrapper reads (`ShellPath`, `TTY`, `User`).
    Ignored,
}

impl ShellMarker {
    pub(crate) fn parse(payload: &str) -> ShellMarker {
        let (key, value) = match payload.split_once('=') {
            Some((key, value)) => (key, Some(value)),
            None => (payload, None),
        };
        if value == Some("") {
            return ShellMarker::Ignored; // empty values are ignored
        }
        let untrimmed = value.unwrap_or("");
        let trimmed = untrimmed.trim();
        match key {
            "StartPrompt" => ShellMarker::StartPrompt,
            "EndPrompt" => ShellMarker::EndPrompt,
            "NewCmd" => ShellMarker::NewCmd(value.map(String::from)),
            "PreExec" => ShellMarker::PreExec,
            "NoHistory" => ShellMarker::NoHistory,
            "OSCLock" => ShellMarker::OscLock(trimmed.to_string()),
            "OSCUnlock" => ShellMarker::OscUnlock(trimmed.to_string()),
            "Dir" => ShellMarker::Dir(PathBuf::from(untrimmed)),
            "ExitCode" => ShellMarker::ExitCode(trimmed.parse().ok()),
            "Shell" => ShellMarker::Shell(trimmed.to_string()),
            "PID" => ShellMarker::Pid(trimmed.parse().ok()),
            "ZshAutosuggestionColor" => {
                ShellMarker::ZshAutosuggestionColor(ShellSuggestionColor::parse_zsh(trimmed))
            }
            "FishSuggestionColor" => {
                ShellMarker::FishSuggestionColor(ShellSuggestionColor::parse_fish(trimmed))
            }
            "PwshPredictionColor" => {
                ShellMarker::PwshPredictionColor(PwshPredictionColor::parse(trimmed))
            }
            "PwshListView" => ShellMarker::PwshListView(trimmed == "1"),
            "PwshPrompts" => {
                let mut parts = untrimmed.split('\x1f').map(String::from);
                ShellMarker::PwshPrompts {
                    continuation: parts.next().unwrap_or_default(),
                    prompt_texts: parts.filter(|p| !p.is_empty()).collect(),
                }
            }
            _ => ShellMarker::Ignored,
        }
    }

    /// While a script holds the integration lock (`OSCLock`), only these still act: they are how a script ends a
    /// command, starts one, hides it from history or releases the lock.
    pub(crate) fn passes_integration_lock(&self) -> bool {
        match self {
            ShellMarker::NewCmd(_)
            | ShellMarker::PreExec
            | ShellMarker::NoHistory
            | ShellMarker::OscLock(_)
            | ShellMarker::OscUnlock(_) => true,
            ShellMarker::StartPrompt
            | ShellMarker::EndPrompt
            | ShellMarker::Dir(_)
            | ShellMarker::ExitCode(_)
            | ShellMarker::Shell(_)
            | ShellMarker::Pid(_)
            | ShellMarker::ZshAutosuggestionColor(_)
            | ShellMarker::FishSuggestionColor(_)
            | ShellMarker::PwshPredictionColor(_)
            | ShellMarker::PwshListView(_)
            | ShellMarker::PwshPrompts { .. }
            | ShellMarker::Ignored => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal_session::domain::value_objects::shell_suggestion_color::ShellSuggestionColor;
    use std::path::PathBuf;

    fn parse_all(payloads: &[&str]) -> Vec<ShellMarker> {
        payloads.iter().map(|p| ShellMarker::parse(p)).collect()
    }

    #[test]
    fn should_read_a_bare_token_as_a_marker_without_a_value() {
        let payloads = ["StartPrompt", "EndPrompt", "PreExec", "NoHistory"];

        let parsed = parse_all(&payloads);

        assert_eq!(
            parsed,
            [
                ShellMarker::StartPrompt,
                ShellMarker::EndPrompt,
                ShellMarker::PreExec,
                ShellMarker::NoHistory
            ]
        );
    }

    #[test]
    fn should_split_the_payload_at_the_first_equals_sign() {
        let payload = "Dir=/a=b";

        let parsed = ShellMarker::parse(payload);

        assert_eq!(parsed, ShellMarker::Dir(PathBuf::from("/a=b")));
    }

    #[test]
    fn should_ignore_a_marker_with_an_empty_value() {
        let payloads = ["Dir=", "StartPrompt=", "NewCmd=", "PwshPrompts="];

        let parsed = parse_all(&payloads);

        assert_eq!(parsed, vec![ShellMarker::Ignored; 4]);
    }

    #[test]
    fn should_ignore_an_unknown_key_and_the_keys_that_set_nothing_the_wrapper_reads() {
        let payloads = [
            "DoneSourcing",
            "Log=x",
            "ShellPath=/bin/zsh",
            "TTY=/dev/pts/1",
            "User=root",
            " Dir=/a",
            "dir=/a",
        ];

        let parsed = parse_all(&payloads);

        assert_eq!(parsed, vec![ShellMarker::Ignored; 7]);
    }

    #[test]
    fn should_keep_the_session_id_of_new_cmd_untrimmed_and_none_when_bare() {
        let payloads = ["NewCmd=abc", "NewCmd= abc", "NewCmd"];

        let parsed = parse_all(&payloads);

        assert_eq!(
            parsed,
            [
                ShellMarker::NewCmd(Some("abc".into())),
                ShellMarker::NewCmd(Some(" abc".into())),
                ShellMarker::NewCmd(None)
            ]
        );
    }

    #[test]
    fn should_trim_the_session_id_of_osc_lock_and_unlock() {
        let payloads = ["OSCLock= abc ", "OSCUnlock=abc ", "OSCLock"];

        let parsed = parse_all(&payloads);

        assert_eq!(
            parsed,
            [
                ShellMarker::OscLock("abc".into()),
                ShellMarker::OscUnlock("abc".into()),
                ShellMarker::OscLock(String::new())
            ]
        );
    }

    #[test]
    fn should_keep_the_dir_untrimmed_and_read_a_bare_dir_as_the_empty_path() {
        let payloads = ["Dir= /a b ", "Dir"];

        let parsed = parse_all(&payloads);

        assert_eq!(
            parsed,
            [
                ShellMarker::Dir(PathBuf::from(" /a b ")),
                ShellMarker::Dir(PathBuf::new())
            ]
        );
    }

    #[test]
    fn should_read_an_unparsable_exit_code_or_pid_as_none() {
        let payloads = [
            "ExitCode= 2 ",
            "ExitCode=x",
            "ExitCode",
            "PID=-1",
            "PID=4294967296",
            "PID",
        ];

        let parsed = parse_all(&payloads);

        assert_eq!(
            parsed,
            [
                ShellMarker::ExitCode(Some(2)),
                ShellMarker::ExitCode(None),
                ShellMarker::ExitCode(None),
                ShellMarker::Pid(Some(-1)),
                ShellMarker::Pid(None),
                ShellMarker::Pid(None)
            ]
        );
    }

    #[test]
    fn should_trim_the_shell_name_and_read_a_bare_shell_as_empty() {
        let payloads = ["Shell= zsh ", "Shell"];

        let parsed = parse_all(&payloads);

        assert_eq!(
            parsed,
            [
                ShellMarker::Shell("zsh".into()),
                ShellMarker::Shell(String::new())
            ]
        );
    }

    #[test]
    fn should_switch_the_list_view_on_only_for_one() {
        let payloads = [
            "PwshListView=1",
            "PwshListView= 1 ",
            "PwshListView=0",
            "PwshListView=true",
            "PwshListView",
        ];

        let parsed = parse_all(&payloads);

        assert_eq!(
            parsed,
            [
                ShellMarker::PwshListView(true),
                ShellMarker::PwshListView(true),
                ShellMarker::PwshListView(false),
                ShellMarker::PwshListView(false),
                ShellMarker::PwshListView(false)
            ]
        );
    }

    #[test]
    fn should_split_pwsh_prompts_untrimmed_on_the_unit_separator_and_drop_empty_texts() {
        let payload = "PwshPrompts=... \x1f> \x1f\x1f>>> ";

        let parsed = ShellMarker::parse(payload);

        assert_eq!(
            parsed,
            ShellMarker::PwshPrompts {
                continuation: "... ".into(),
                prompt_texts: vec!["> ".into(), ">>> ".into()]
            }
        );
    }

    #[test]
    fn should_read_the_suggestion_colours_and_none_when_they_do_not_parse() {
        let payloads = [
            "ZshAutosuggestionColor=fg=8,bold",
            "ZshAutosuggestionColor=bold",
            "FishSuggestionColor=555",
            "PwshPredictionColor=[1;4m",
        ];

        let parsed = parse_all(&payloads);

        assert_eq!(
            parsed,
            [
                ShellMarker::ZshAutosuggestionColor(ShellSuggestionColor::parse_zsh("fg=8,bold")),
                ShellMarker::ZshAutosuggestionColor(None),
                ShellMarker::FishSuggestionColor(ShellSuggestionColor::parse_fish("555")),
                ShellMarker::PwshPredictionColor(None)
            ]
        );
    }

    #[test]
    fn should_let_only_the_five_lock_exempt_markers_through_the_integration_lock() {
        let exempt = [
            "NewCmd=a",
            "PreExec",
            "NoHistory",
            "OSCLock=a",
            "OSCUnlock=a",
        ];
        let guarded = [
            "StartPrompt",
            "EndPrompt",
            "Dir=/a",
            "ExitCode=1",
            "Shell=zsh",
            "PID=1",
            "PwshListView=1",
            "DoneSourcing",
        ];

        let passes = |payloads: &[&str]| -> Vec<bool> {
            parse_all(payloads)
                .iter()
                .map(ShellMarker::passes_integration_lock)
                .collect()
        };

        assert_eq!(
            (passes(&exempt), passes(&guarded)),
            (vec![true; 5], vec![false; 8])
        );
    }
}
