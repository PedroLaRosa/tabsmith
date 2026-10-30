//! Inline ghost text: local sources, LLM profiles/clients, completion cache, validation (SPEC §8,
//! §3.5, §6.11). OWNER: inline agent. Used by the engine (sources, LLM worker), the CLI (`inline` command) and
//! doctor (profile checks).
use crate::ghost_text::domain::value_objects::llm_profile::LlmProfile;
use crate::ghost_text::domain::value_objects::llm_provider::LlmProvider;
use crate::shared::domain::value_objects::shell::Shell;
use crate::shared::infrastructure::data_directory_from_env::user_home_from_env;
use crate::shared::infrastructure::logger;
use crate::history::domain::entities::history_entry::HistoryEntry;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

/// Snapshot for an LLM worker: the last `count` (`inline.historyCount`) entries with an exit code that pass the
/// privacy filter, oldest → newest.
pub fn llm_history(entries: &[HistoryEntry], count: usize) -> Vec<HistoryEntry> {
    let mut v: Vec<HistoryEntry> = entries
        .iter()
        .rev()
        .filter(|e| usable(e))
        .take(count)
        .cloned()
        .collect();
    v.reverse();
    v
}

/// Blocking LLM request (worker thread): prompt with privacy filter, HTTP, cleaning, validation (SPEC §8.5).
/// `None` on any error (logged) or when nothing should be suggested. `history` = `llm_history(..)` snapshot.
pub fn llm_complete(
    profile: &LlmProfile,
    buffer: &str,
    cwd: &Path,
    shell: Shell,
    history: &[HistoryEntry],
    env: &HashMap<String, String>,
) -> Option<String> {
    let prompt = history_prompt(history, buffer)?;
    let user = format!(
        "OS: {}\nShell: {}\nCWD: {}\n\n{prompt}",
        std::env::consts::OS,
        shell.name(),
        cwd.display()
    );
    let key = env
        .get(profile.api_key_env())
        .filter(|k| !k.is_empty())
        .cloned()
        .or_else(|| std::env::var(profile.api_key_env()).ok())
        .unwrap_or_default();
    let (url, headers, body) = build_request(profile, &user, &key);
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
    let completion = clean(&parse_response(profile.provider(), &resp)?, buffer)?;
    let full = format!("{buffer}{completion}");
    validate(&full, cwd, env).then_some(completion)
}

const SYSTEM: &str = "You complete shell commands. The user message shows the user's recent shell history in \
`history` format; the last numbered line is the command being typed. Reply with ONLY the characters that should be \
appended to that last line to complete it: one line, no explanation, no quotes, no code fences. If unsure, reply \
with nothing.";

const PROMPT_MAX_BYTES: usize = 10_240;
const PRIVATE_WORDS: [&str; 8] = [
    "password",
    "passwd",
    "secret",
    "token",
    "api_key",
    "apikey",
    "authorization:",
    "bearer ",
];

fn is_private(s: &str) -> bool {
    let s = s.to_lowercase();
    PRIVATE_WORDS.iter().any(|w| s.contains(w))
}

fn usable(e: &HistoryEntry) -> bool {
    e.exit_code().is_some() && !is_private(e.command())
}

/// History-style prompt (SPEC §8.5): usable entries then the buffer, numbered from 1, oldest dropped until
/// ≤ 10 240 bytes. `None` when the buffer is private or alone too long.
fn history_prompt(history: &[HistoryEntry], buffer: &str) -> Option<String> {
    if is_private(buffer) {
        return None;
    }
    let mut lines: Vec<&str> = history
        .iter()
        .filter(|e| usable(e))
        .map(|e| e.command())
        .collect();
    lines.push(buffer);
    // Every line costs `{:>5}  ` + "\n" = 8 bytes (numbers < 100 000), so drop by arithmetic, then format once.
    let cost = |c: &str| c.len() + 8;
    let mut total = lines.iter().map(|c| cost(c)).sum::<usize>() - 1;
    let mut skip = 0;
    while total > PROMPT_MAX_BYTES {
        if skip + 1 == lines.len() {
            return None;
        }
        total -= cost(lines[skip]);
        skip += 1;
    }
    let numbered: Vec<String> = lines[skip..]
        .iter()
        .enumerate()
        .map(|(i, c)| format!("{:>5}  {c}", i + 1))
        .collect();
    Some(numbered.join("\n"))
}

/// URL, headers and JSON body for one request (SPEC §8.5). Auth header omitted when `key` is empty.
fn build_request(
    profile: &LlmProfile,
    user: &str,
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
                "messages": [{"role": "system", "content": SYSTEM}, {"role": "user", "content": user}],
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
                "system": SYSTEM,
                "messages": [{"role": "user", "content": user}],
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

/// Cleaning (SPEC §8.5): first non-empty non-fence line, surrounding backticks/quotes, history number, echoed
/// buffer, trailing whitespace.
fn clean(text: &str, buffer: &str) -> Option<String> {
    let line = text.lines().find(|l| {
        let t = l.trim();
        !t.is_empty() && !t.starts_with("```")
    })?;
    let t = line.trim();
    let mut s = ['`', '"', '\'']
        .into_iter()
        .find_map(|q| {
            t.strip_prefix(q)?
                .strip_suffix(q)
                .filter(|inner| !inner.contains(q))
        })
        .unwrap_or(line);
    // Number first so a fully echoed `    3  git status` loses both its number and the buffer.
    let digits = s.trim_start();
    let rest = digits.trim_start_matches(|c: char| c.is_ascii_digit());
    if rest.len() < digits.len() {
        s = rest.strip_prefix("  ").unwrap_or(s);
    }
    if !buffer.is_empty() {
        s = s.strip_prefix(buffer).unwrap_or(s);
    }
    let s = s.trim_end();
    // Ghost text is typed into the shell as-is: an embedded CR/ESC/C1 byte would run or rewrite commands.
    (!s.is_empty() && !s.chars().any(char::is_control)).then(|| s.to_string())
}

/// `cd` validation (SPEC §8.5): `full` = buffer + completion.
pub fn validate(full: &str, cwd: &Path, env: &HashMap<String, String>) -> bool {
    let Some(words) = split_words(full.trim()) else {
        return true;
    };
    let [cd, arg] = words.as_slice() else {
        return true;
    };
    if cd != "cd" {
        return true;
    }
    let Some(path) = expand(arg, env) else {
        return true;
    };
    let path = Path::new(&path);
    if path.is_absolute() && !path.is_dir() {
        return false;
    }
    cwd.join(path).canonicalize().is_ok_and(|p| p.is_dir())
}

/// POSIX-ish word split (quotes, backslash escapes; backslash is literal on Windows). `None` on an open quote.
fn split_words(s: &str) -> Option<Vec<String>> {
    let escapes = !cfg!(windows);
    let mut words = Vec::new();
    let mut cur: Option<String> = None;
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        match c {
            c if c.is_whitespace() => words.extend(cur.take()),
            '\'' => {
                let w = cur.get_or_insert_default();
                loop {
                    match chars.next()? {
                        '\'' => break,
                        c => w.push(c),
                    }
                }
            }
            '"' => {
                let w = cur.get_or_insert_default();
                loop {
                    match chars.next()? {
                        '"' => break,
                        '\\' if escapes => {
                            let n = chars.next()?;
                            if !matches!(n, '"' | '\\' | '$' | '`') {
                                w.push('\\');
                            }
                            w.push(n);
                        }
                        c => w.push(c),
                    }
                }
            }
            '\\' if escapes => cur.get_or_insert_default().extend(chars.next()),
            c => cur.get_or_insert_default().push(c),
        }
    }
    words.extend(cur);
    Some(words)
}

/// Expand a leading `~` and `$VAR`/`${VAR}` from `env`. `None` when something is unknown (→ accept).
fn expand(arg: &str, env: &HashMap<String, String>) -> Option<String> {
    let mut out = String::new();
    let mut s = arg;
    if arg == "~" || arg.starts_with("~/") || (cfg!(windows) && arg.starts_with("~\\")) {
        let home = env
            .get("HOME")
            .or_else(|| env.get("USERPROFILE"))
            .cloned()
            .or_else(|| {
                user_home_from_env()
                    .ok()
                    .map(|p| p.to_string_lossy().into_owned())
            })?;
        out.push_str(&home);
        s = &arg[1..];
    }
    while let Some(i) = s.find('$') {
        out.push_str(&s[..i]);
        let after = &s[i + 1..];
        let (name, tail) = match after.strip_prefix('{') {
            Some(b) => {
                let end = b.find('}')?;
                (&b[..end], &b[end + 1..])
            }
            None => {
                let end = after
                    .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                    .unwrap_or(after.len());
                after.split_at(end)
            }
        };
        if name.is_empty() {
            out.push('$');
            s = after;
            continue;
        }
        out.push_str(env.get(name)?);
        s = tail;
    }
    out.push_str(s);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ghost_text::domain::value_objects::llm_profile::LlmProfileParts;
    use crate::history::domain::entities::history_entry::HistoryEntryPrimitives;
    use crate::shared::infrastructure::testing::temp_dir::TempDir;

    fn entry(cmd: &str, exit: Option<i32>) -> HistoryEntry {
        HistoryEntry::new(HistoryEntryPrimitives {
            command: cmd.into(),
            shell: "zsh".into(),
            exit_code: exit,
            ..Default::default()
        })
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
    fn prompt_numbering_and_filters() {
        let h = [
            entry("echo hello", Some(0)),
            entry("never finished", None),
            entry("export API_KEY=1", Some(0)),
            entry("curl -H 'Authorization: x'", Some(0)),
            entry("echo world", Some(1)),
        ];
        assert_eq!(
            history_prompt(&h, "echo ").unwrap(),
            "    1  echo hello\n    2  echo world\n    3  echo "
        );
        assert_eq!(history_prompt(&h, "mysql --password=x"), None);
        assert_eq!(history_prompt(&[], "ls").unwrap(), "    1  ls");
    }

    #[test]
    fn prompt_truncation() {
        let big = "a".repeat(3000);
        let h: Vec<_> = (0..5)
            .map(|i| entry(&format!("{i}{big}"), Some(0)))
            .collect();
        let p = history_prompt(&h, "ls").unwrap();
        assert!(p.len() <= PROMPT_MAX_BYTES, "{}", p.len());
        // 3 history lines (3008 bytes each incl. prefix) + buffer fit; the oldest two are dropped.
        assert!(p.starts_with("    1  2aaa"));
        assert!(p.ends_with("\n    4  ls"));
        // Exactly at the limit is kept.
        let exact = "b".repeat(PROMPT_MAX_BYTES - 7);
        assert_eq!(
            history_prompt(&[entry("x", Some(0))], &exact)
                .unwrap()
                .len(),
            PROMPT_MAX_BYTES
        );
        assert_eq!(history_prompt(&[], &"c".repeat(PROMPT_MAX_BYTES)), None);
    }

    #[test]
    fn llm_history_snapshot() {
        let h = [
            entry("a", Some(0)),
            entry("b", None),
            entry("c token", Some(0)),
            entry("d", Some(2)),
            entry("e", Some(0)),
        ];
        let cmds: Vec<_> = llm_history(&h, 2).into_iter().map(|e| e.command().to_string()).collect();
        assert_eq!(cmds, ["d", "e"]);
        assert_eq!(llm_history(&h, 49).len(), 3);
    }

    #[test]
    fn cleaning() {
        assert_eq!(clean("tus\nmore", "git sta").as_deref(), Some("tus"));
        assert_eq!(
            clean("```bash\n status\n```", "git").as_deref(),
            Some(" status")
        );
        assert_eq!(clean("`status`", "git ").as_deref(), Some("status"));
        assert_eq!(clean("\"status\"", "git ").as_deref(), Some("status"));
        assert_eq!(
            clean("\"a\" \"b\"", "echo ").as_deref(),
            Some("\"a\" \"b\"")
        );
        assert_eq!(clean("git status", "git st").as_deref(), Some("atus"));
        assert_eq!(
            clean("    3  git status", "git st").as_deref(),
            Some("atus")
        );
        assert_eq!(clean("50  ls -la", "ls").as_deref(), Some(" -la"));
        assert_eq!(clean(" -la   ", "ls").as_deref(), Some(" -la"));
        assert_eq!(clean("\n  \n", "ls"), None);
        assert_eq!(clean("git st", "git st"), None);
        assert_eq!(clean("", "ls"), None);
        // Control characters (C0, DEL, C1) are never offered as ghost text.
        assert_eq!(clean("tus\rtouch x", "git sta"), None);
        assert_eq!(clean(" status\rtouch PWNED #", "git"), None);
        assert_eq!(clean(" -la\x1b[2J", "ls"), None);
        assert_eq!(clean(" -l\ta", "ls"), None);
        assert_eq!(clean(" -l\u{9b}2J", "ls"), None);
        assert_eq!(clean(" -l\x7f", "ls"), None);
    }

    #[test]
    fn request_bodies() {
        let (url, headers, body) = build_request(&profile(LlmProvider::OpenAi, None), "U", "k");
        assert_eq!(url, "http://x/chat/completions");
        assert_eq!(headers, [("authorization", "Bearer k".to_string())]);
        assert_eq!(
            body.to_string(),
            json!({"max_tokens": 7, "messages": [{"content": SYSTEM, "role": "system"}, {"content": "U", "role": "user"}], "model": "m"}).to_string()
        );
        assert!(build_request(&profile(LlmProvider::OpenAi, None), "U", "")
            .1
            .is_empty());

        let (url, headers, body) = build_request(&profile(LlmProvider::Anthropic, None), "U", "k");
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
        let (_, _, body) = build_request(&profile(LlmProvider::Anthropic, Some("low")), "U", "k");
        assert_eq!(body["output_config"], json!({"effort": "low"}));
        for b in [
            body,
            build_request(&profile(LlmProvider::OpenAi, Some("low")), "U", "k").2,
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

    #[test]
    fn cd_validation() {
        let temp_dir = TempDir::new("cd");
        let d = temp_dir.path().to_path_buf();
        std::fs::create_dir_all(d.join("a/b")).unwrap();
        std::fs::create_dir_all(d.join("space in name")).unwrap();
        std::fs::write(d.join("file"), "x").unwrap();
        std::fs::write(d.join("a/file"), "x").unwrap();
        let home = d.to_string_lossy().into_owned();
        let env: HashMap<String, String> = [("HOME", home.as_str()), ("A", "a"), ("B", "b")]
            .into_iter()
            .map(|(k, v)| (k.into(), v.into()))
            .collect();
        let valid = |c: &str| validate(c, &d, &env);
        for ok in [
            "echo \"hello\"",
            "cd ~",
            "cd ~/",
            "cd ~/a/b",
            "cd ~/$A/$B",
            "cd ${A}/b",
            "cd a/",
            "cd $A",
            "cd space\\ in\\ name",
            "cd \"space in name\"",
            "cd 'space in name'",
            "cd space in name",
            "cd $UNKNOWN/x",
            "cd \"open",
            &format!("cd {home}/a"),
        ] {
            assert!(valid(ok), "{ok}");
        }
        for bad in [
            "cd /folder/doesnt/exist",
            "cd ~/file",
            "cd ~/$A/file",
            "cd b",
            "cd file",
            "cd $B",
            "cd a/b/file",
        ] {
            assert!(!valid(bad), "{bad}");
        }
        assert!(!valid(&format!("cd {home}/file")));
        std::fs::remove_dir_all(&d).unwrap();
    }
}
