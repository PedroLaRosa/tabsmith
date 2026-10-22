//! Inline ghost text: history store, local sources, LLM profiles/clients, completion cache, validation (SPEC §8,
//! §3.5, §6.11). OWNER: inline agent. Used by the engine (sources, LLM worker), the CLI (`inline` command) and
//! doctor (profile checks).
use crate::settings::Settings;
use crate::types::{HistoryEntry, Item, ItemType, Shell};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

const HISTORY_IN_MEMORY: usize = 5000;
const HISTORY_COMPACT_AT: usize = 10_000;

/// `~/.tabsmith/history.jsonl` (last 5000 entries in memory; file compacted past 10 000 lines).
pub struct History {
    entries: Vec<HistoryEntry>,
    /// Backing file (`None` = memory only).
    path: Option<PathBuf>,
    /// Lines in the file as far as this process knows (other sessions append too).
    lines: usize,
}

impl History {
    /// Load the history file (missing/corrupt lines skipped).
    pub fn load() -> History {
        match crate::settings::home() {
            Ok(home) => History::load_from(home.join("history.jsonl")),
            Err(e) => {
                crate::logger::warn(&format!("history: {e:#}"));
                History::from_entries(Vec::new())
            }
        }
    }

    /// Load (and later append to) an explicit file (tests, `load`).
    pub fn load_from(path: PathBuf) -> History {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let lines = text.lines().count();
        let mut entries: Vec<HistoryEntry> = text.lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
        entries.drain(..entries.len().saturating_sub(HISTORY_IN_MEMORY));
        let mut h = History { entries, path: Some(path), lines };
        if lines > HISTORY_COMPACT_AT {
            h.compact();
        }
        h
    }

    /// In-memory history (tests).
    pub fn from_entries(entries: Vec<HistoryEntry>) -> History {
        History { entries, path: None, lines: 0 }
    }

    /// Append to memory and to the file (non-empty commands only).
    pub fn append(&mut self, entry: HistoryEntry) {
        if entry.command.trim().is_empty() {
            return;
        }
        if let Some(path) = &self.path {
            let line = serde_json::to_string(&entry).unwrap_or_default() + "\n";
            let res = path.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|_| {
                open_private(path, true)?.write_all(line.as_bytes())
            });
            match res {
                Ok(()) => self.lines += 1,
                Err(e) => crate::logger::warn(&format!("history: appending to {}: {e}", path.display())),
            }
        }
        self.entries.push(entry);
        self.entries.drain(..self.entries.len().saturating_sub(HISTORY_IN_MEMORY));
        if self.lines > HISTORY_COMPACT_AT {
            self.compact();
        }
    }

    /// Oldest → newest.
    pub fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }

    /// Rewrite the file with its last 5000 lines (temp + rename). Re-reads the file so lines appended by other
    /// sessions survive. ponytail: no lock; a line appended between read and rename is lost.
    fn compact(&mut self) {
        let Some(path) = &self.path else { return };
        let res = (|| {
            let text = std::fs::read_to_string(path)?;
            let all: Vec<&str> = text.lines().collect();
            let keep = &all[all.len().saturating_sub(HISTORY_IN_MEMORY)..];
            let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
            open_private(&tmp, false)?.write_all((keep.join("\n") + "\n").as_bytes())?;
            std::fs::rename(&tmp, path)?;
            Ok::<_, std::io::Error>(keep.len())
        })();
        match res {
            Ok(n) => self.lines = n,
            Err(e) => crate::logger::warn(&format!("history: compacting {}: {e}", path.display())),
        }
    }
}

/// Open for append or truncate+write; created with mode 0600 on Unix.
fn open_private(path: &Path, append: bool) -> std::io::Result<std::fs::File> {
    let mut o = std::fs::OpenOptions::new();
    o.create(true);
    if append {
        o.append(true);
    } else {
        o.write(true).truncate(true);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600);
    }
    o.open(path)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    OpenAi,
    Anthropic,
}

/// A user-defined LLM profile `inline.profiles.<name>` (SPEC §3.3).
#[derive(Clone, Debug, PartialEq)]
pub struct Profile {
    pub name: String,
    pub provider: Provider,
    pub model: String,
    pub base_url: String,
    pub api_key_env: String,
    pub max_tokens: u32,
    pub effort: Option<String>,
}

/// The selected inline source (`inline.source`, default `history+specs`).
#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    History,
    Specs,
    HistoryAndSpecs,
    Profile(Profile),
}

/// Built-in source names with descriptions, in display order.
pub const BUILTIN_SOURCES: [(&str, &str); 3] = [
    ("history+specs", "Local history, then completion specs"),
    ("history", "Local history only"),
    ("specs", "Completion specs only"),
];

const PROFILES_PREFIX: &str = "inline.profiles.";

/// Every `inline.profiles.*` entry, parsed (`Err(reason)` for invalid objects), sorted by name.
pub fn profiles(settings: &Settings) -> Vec<(String, Result<Profile, String>)> {
    let mut v: Vec<_> =
        settings.with_prefix(PROFILES_PREFIX).map(|(n, val)| (n.to_string(), parse_profile(n, val))).collect();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v
}

/// Resolve `inline.source` (unset → HistoryAndSpecs). `Err` when it names a missing/invalid profile.
pub fn source(settings: &Settings) -> Result<Source, String> {
    match settings.string("inline.source").unwrap_or("history+specs") {
        "history+specs" => Ok(Source::HistoryAndSpecs),
        "history" => Ok(Source::History),
        "specs" => Ok(Source::Specs),
        name => match settings.get(&format!("{PROFILES_PREFIX}{name}")) {
            None => Err(format!("profile '{name}' not found")),
            Some(v) => parse_profile(name, v).map(Source::Profile).map_err(|e| format!("profile '{name}' is invalid: {e}")),
        },
    }
}

/// One profile object with per-provider defaults (SPEC §3.3).
fn parse_profile(name: &str, v: &Value) -> Result<Profile, String> {
    if BUILTIN_SOURCES.iter().any(|(b, _)| *b == name) {
        return Err(format!("'{name}' is a reserved source name"));
    }
    let o = v.as_object().ok_or("not a JSON object")?;
    let s = |k: &str| match o.get(k) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(format!("{k} must be a string")),
    };
    let provider = match s("provider")?.as_deref() {
        Some("openai") => Provider::OpenAi,
        Some("anthropic") => Provider::Anthropic,
        Some(p) => return Err(format!("unknown provider '{p}' (expected openai or anthropic)")),
        None => return Err("missing provider".into()),
    };
    let model = s("model")?.filter(|m| !m.is_empty()).ok_or("missing model")?;
    let (base_url, api_key_env, max_tokens) = match provider {
        Provider::OpenAi => ("https://api.openai.com/v1", "OPENAI_API_KEY", 64),
        Provider::Anthropic => ("https://api.anthropic.com", "ANTHROPIC_API_KEY", 1024),
    };
    let max_tokens = match o.get("max_tokens") {
        None | Some(Value::Null) => max_tokens,
        Some(n) => n
            .as_u64()
            .filter(|n| (1..=u32::MAX as u64).contains(n))
            .ok_or("max_tokens must be a positive integer")? as u32,
    };
    Ok(Profile {
        name: name.to_string(),
        provider,
        model,
        base_url: s("base_url")?.unwrap_or_else(|| base_url.into()),
        api_key_env: s("api_key_env")?.unwrap_or_else(|| api_key_env.into()),
        max_tokens,
        effort: s("effort")?,
    })
}

/// History source (SPEC §6.11): newest successful entry extending `buffer` → the remaining suffix.
pub fn history_ghost(history: &[HistoryEntry], buffer: &str) -> Option<String> {
    if buffer.is_empty() {
        return None;
    }
    let newest = |ok_only: bool| {
        history.iter().rev().find(|e| {
            e.command.len() > buffer.len() && e.command.starts_with(buffer) && (!ok_only || e.exit_code == Some(0))
        })
    };
    newest(true).or_else(|| newest(false)).map(|e| e.command[buffer.len()..].to_string())
}

/// Specs source (SPEC §6.11): first non-auto-execute/history item with `delete == 0`, prefix match and non-empty
/// text (trailing space trimmed).
pub fn spec_ghost(items: &[Item]) -> Option<String> {
    items.iter().find_map(|i| {
        let text = i.insert.text.strip_suffix(' ').unwrap_or(&i.insert.text);
        // accepting a ghost types its text and leaves the cursor at the end: rows that move it back are skipped
        let ok = !matches!(i.kind, ItemType::AutoExecute | ItemType::History)
            && i.insert.delete == 0
            && i.insert.cursor_back <= i.insert.text.len() - text.len()
            && i.prefix_match
            && !text.is_empty();
        ok.then(|| text.to_string())
    })
}

/// LLM completion cache (SPEC §8.4), cleared on every new command.
#[derive(Default)]
pub struct GhostCache {
    entries: Vec<(String, f64)>,
}

impl GhostCache {
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Store full texts (`buffer + completion`); the first gets priority 0.0, others 1.0.
    pub fn insert(&mut self, full_texts: &[String]) {
        for (i, t) in full_texts.iter().enumerate() {
            self.entries.push((t.clone(), if i == 0 { 0.0 } else { 1.0 }));
        }
        // ponytail: linear scan instead of Kiro's radix trie, so keep it small (oldest dropped).
        self.entries.drain(..self.entries.len().saturating_sub(50));
    }

    /// Lowest-priority cached text extending `buffer` (first inserted wins ties) → the remaining suffix.
    pub fn lookup(&self, buffer: &str) -> Option<String> {
        self.entries
            .iter()
            .filter(|(full, _)| full.len() > buffer.len() && full.starts_with(buffer))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(full, _)| full[buffer.len()..].to_string())
    }
}

/// Snapshot for an LLM worker: the last `count` (`inline.historyCount`) entries with an exit code that pass the
/// privacy filter, oldest → newest.
pub fn llm_history(entries: &[HistoryEntry], count: usize) -> Vec<HistoryEntry> {
    let mut v: Vec<HistoryEntry> =
        entries.iter().rev().filter(|e| usable(e)).take(count).cloned().collect();
    v.reverse();
    v
}

/// Blocking LLM request (worker thread): prompt with privacy filter, HTTP, cleaning, validation (SPEC §8.5).
/// `None` on any error (logged) or when nothing should be suggested. `history` = `llm_history(..)` snapshot.
pub fn llm_complete(
    profile: &Profile,
    buffer: &str,
    cwd: &Path,
    shell: Shell,
    history: &[HistoryEntry],
    env: &HashMap<String, String>,
) -> Option<String> {
    let prompt = history_prompt(history, buffer)?;
    let user = format!("OS: {}\nShell: {}\nCWD: {}\n\n{prompt}", std::env::consts::OS, shell.name(), cwd.display());
    let key = env
        .get(&profile.api_key_env)
        .filter(|k| !k.is_empty())
        .cloned()
        .or_else(|| std::env::var(&profile.api_key_env).ok())
        .unwrap_or_default();
    let (url, headers, body) = build_request(profile, &user, &key);
    let agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(5))).build().new_agent();
    let mut req = agent.post(url.as_str());
    for (k, v) in headers {
        req = req.header(k, v);
    }
    let resp: Value = match req.send_json(&body).and_then(|mut r| r.body_mut().read_json()) {
        Ok(v) => v,
        Err(e) => {
            crate::logger::warn(&format!("inline: profile '{}' request to {url} failed: {e}", profile.name));
            return None;
        }
    };
    let completion = clean(&parse_response(profile.provider, &resp)?, buffer)?;
    let full = format!("{buffer}{completion}");
    validate(&full, cwd, env).then_some(completion)
}

const SYSTEM: &str = "You complete shell commands. The user message shows the user's recent shell history in \
`history` format; the last numbered line is the command being typed. Reply with ONLY the characters that should be \
appended to that last line to complete it: one line, no explanation, no quotes, no code fences. If unsure, reply \
with nothing.";

const PROMPT_MAX_BYTES: usize = 10_240;
const PRIVATE_WORDS: [&str; 8] =
    ["password", "passwd", "secret", "token", "api_key", "apikey", "authorization:", "bearer "];

fn is_private(s: &str) -> bool {
    let s = s.to_lowercase();
    PRIVATE_WORDS.iter().any(|w| s.contains(w))
}

fn usable(e: &HistoryEntry) -> bool {
    e.exit_code.is_some() && !is_private(&e.command)
}

/// Kiro `history`-style prompt (SPEC §8.5): usable entries then the buffer, numbered from 1, oldest dropped until
/// ≤ 10 240 bytes. `None` when the buffer is private or alone too long.
fn history_prompt(history: &[HistoryEntry], buffer: &str) -> Option<String> {
    if is_private(buffer) {
        return None;
    }
    let mut lines: Vec<&str> = history.iter().filter(|e| usable(e)).map(|e| e.command.as_str()).collect();
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
    let numbered: Vec<String> =
        lines[skip..].iter().enumerate().map(|(i, c)| format!("{:>5}  {c}", i + 1)).collect();
    Some(numbered.join("\n"))
}

/// URL, headers and JSON body for one request (SPEC §8.5). Auth header omitted when `key` is empty.
fn build_request(profile: &Profile, user: &str, key: &str) -> (String, Vec<(&'static str, String)>, Value) {
    let base = profile.base_url.trim_end_matches('/');
    let mut headers = Vec::new();
    match profile.provider {
        Provider::OpenAi => {
            if !key.is_empty() {
                headers.push(("authorization", format!("Bearer {key}")));
            }
            let body = json!({
                "model": profile.model,
                "max_tokens": profile.max_tokens,
                "messages": [{"role": "system", "content": SYSTEM}, {"role": "user", "content": user}],
            });
            (format!("{base}/chat/completions"), headers, body)
        }
        Provider::Anthropic => {
            if !key.is_empty() {
                headers.push(("x-api-key", key.to_string()));
            }
            headers.push(("anthropic-version", "2026-10-21".to_string()));
            let mut body = json!({
                "model": profile.model,
                "max_tokens": profile.max_tokens,
                "system": SYSTEM,
                "messages": [{"role": "user", "content": user}],
            });
            if let Some(effort) = &profile.effort {
                body["output_config"] = json!({ "effort": effort });
            }
            (format!("{base}/v1/messages"), headers, body)
        }
    }
}

/// Raw model text from a response body; `None` on refusal or unexpected shape.
fn parse_response(provider: Provider, body: &Value) -> Option<String> {
    match provider {
        Provider::OpenAi => body["choices"][0]["message"]["content"].as_str().map(String::from),
        Provider::Anthropic => {
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
        .find_map(|q| t.strip_prefix(q)?.strip_suffix(q).filter(|inner| !inner.contains(q)))
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
    let Some(words) = split_words(full.trim()) else { return true };
    let [cd, arg] = words.as_slice() else { return true };
    if cd != "cd" {
        return true;
    }
    let Some(path) = expand(arg, env) else { return true };
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
        let home = env.get("HOME").or_else(|| env.get("USERPROFILE")).cloned().or_else(|| {
            crate::settings::user_home().ok().map(|p| p.to_string_lossy().into_owned())
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
                let end = after.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).unwrap_or(after.len());
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
    use crate::types::Insertion;

    fn tmp_dir(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let d = std::env::temp_dir().join(format!("tabsmith-inline-{tag}-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn entry(cmd: &str, exit: Option<i32>) -> HistoryEntry {
        HistoryEntry { command: cmd.into(), shell: "zsh".into(), exit_code: exit, ..Default::default() }
    }

    fn settings(v: Value) -> Settings {
        Settings::from_map(v.as_object().unwrap().clone())
    }

    fn profile(provider: Provider, effort: Option<&str>) -> Profile {
        Profile {
            name: "p".into(),
            provider,
            model: "m".into(),
            base_url: "http://x/".into(),
            api_key_env: "K".into(),
            max_tokens: 7,
            effort: effort.map(String::from),
        }
    }

    #[test]
    fn kiro_cache_vector() {
        let mut c = GhostCache::default();
        for (t, p) in [
            ("echo hello, world", 0.5),
            ("echo hello there", 1.0),
            ("ls", 0.5),
            ("ls -l", 1.0),
            ("ls -al", 1.0),
            ("cat -n", 0.5),
            ("cat", 1.0),
        ] {
            c.entries.push((t.into(), p));
        }
        assert_eq!(c.lookup("e").as_deref(), Some("cho hello, world"));
        assert_eq!(c.lookup("echo hello ").as_deref(), Some("there"));
        assert_eq!(c.lookup("ls -a").as_deref(), Some("l"));
        assert_eq!(c.lookup("cat -").as_deref(), Some("n"));
        assert_eq!(c.lookup("other"), None);
        assert_eq!(c.lookup("ls -al"), None, "an exact match is not a completion");
        c.clear();
        assert_eq!(c.lookup("e"), None);
    }

    #[test]
    fn cache_insert_priorities() {
        let mut c = GhostCache::default();
        c.insert(&["git status".into(), "git stash".into()]);
        assert_eq!(c.lookup("git sta").as_deref(), Some("tus"));
        assert_eq!(c.lookup("git stas").as_deref(), Some("h"));
        c.insert(&(0..60).map(|i| format!("x{i}")).collect::<Vec<_>>());
        assert_eq!(c.entries.len(), 50);
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
        assert_eq!(history_prompt(&h, "echo ").unwrap(), "    1  echo hello\n    2  echo world\n    3  echo ");
        assert_eq!(history_prompt(&h, "mysql --password=x"), None);
        assert_eq!(history_prompt(&[], "ls").unwrap(), "    1  ls");
    }

    #[test]
    fn prompt_truncation() {
        let big = "a".repeat(3000);
        let h: Vec<_> = (0..5).map(|i| entry(&format!("{i}{big}"), Some(0))).collect();
        let p = history_prompt(&h, "ls").unwrap();
        assert!(p.len() <= PROMPT_MAX_BYTES, "{}", p.len());
        // 3 history lines (3008 bytes each incl. prefix) + buffer fit; the oldest two are dropped.
        assert!(p.starts_with("    1  2aaa"));
        assert!(p.ends_with("\n    4  ls"));
        // Exactly at the limit is kept.
        let exact = "b".repeat(PROMPT_MAX_BYTES - 7);
        assert_eq!(history_prompt(&[entry("x", Some(0))], &exact).unwrap().len(), PROMPT_MAX_BYTES);
        assert_eq!(history_prompt(&[], &"c".repeat(PROMPT_MAX_BYTES)), None);
    }

    #[test]
    fn llm_history_snapshot() {
        let h = [entry("a", Some(0)), entry("b", None), entry("c token", Some(0)), entry("d", Some(2)), entry("e", Some(0))];
        let cmds: Vec<_> = llm_history(&h, 2).into_iter().map(|e| e.command).collect();
        assert_eq!(cmds, ["d", "e"]);
        assert_eq!(llm_history(&h, 49).len(), 3);
    }

    #[test]
    fn cleaning() {
        assert_eq!(clean("tus\nmore", "git sta").as_deref(), Some("tus"));
        assert_eq!(clean("```bash\n status\n```", "git").as_deref(), Some(" status"));
        assert_eq!(clean("`status`", "git ").as_deref(), Some("status"));
        assert_eq!(clean("\"status\"", "git ").as_deref(), Some("status"));
        assert_eq!(clean("\"a\" \"b\"", "echo ").as_deref(), Some("\"a\" \"b\""));
        assert_eq!(clean("git status", "git st").as_deref(), Some("atus"));
        assert_eq!(clean("    3  git status", "git st").as_deref(), Some("atus"));
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
        let (url, headers, body) = build_request(&profile(Provider::OpenAi, None), "U", "k");
        assert_eq!(url, "http://x/chat/completions");
        assert_eq!(headers, [("authorization", "Bearer k".to_string())]);
        assert_eq!(
            body.to_string(),
            json!({"max_tokens": 7, "messages": [{"content": SYSTEM, "role": "system"}, {"content": "U", "role": "user"}], "model": "m"}).to_string()
        );
        assert!(build_request(&profile(Provider::OpenAi, None), "U", "").1.is_empty());

        let (url, headers, body) = build_request(&profile(Provider::Anthropic, None), "U", "k");
        assert_eq!(url, "http://x/v1/messages");
        assert_eq!(headers, [("x-api-key", "k".to_string()), ("anthropic-version", "2026-10-21".to_string())]);
        assert_eq!(
            body,
            json!({"model": "m", "max_tokens": 7, "system": SYSTEM, "messages": [{"role": "user", "content": "U"}]})
        );
        let (_, _, body) = build_request(&profile(Provider::Anthropic, Some("low")), "U", "k");
        assert_eq!(body["output_config"], json!({"effort": "low"}));
        for b in [body, build_request(&profile(Provider::OpenAi, Some("low")), "U", "k").2] {
            let s = b.to_string();
            assert!(!s.contains("temperature") && !s.contains("top_p") && !s.contains("top_k"), "{s}");
        }
    }

    #[test]
    fn response_parsing() {
        let openai = json!({"choices": [{"message": {"role": "assistant", "content": "tus"}}]});
        assert_eq!(parse_response(Provider::OpenAi, &openai).as_deref(), Some("tus"));
        let refused = json!({"choices": [{"message": {"content": null, "refusal": "no"}}]});
        assert_eq!(parse_response(Provider::OpenAi, &refused), None);
        let anthropic = json!({"stop_reason": "end_turn", "content": [
            {"type": "thinking", "thinking": "hmm", "signature": "s"},
            {"type": "text", "text": "tu"}, {"type": "text", "text": "s"}]});
        assert_eq!(parse_response(Provider::Anthropic, &anthropic).as_deref(), Some("tus"));
        let refusal = json!({"stop_reason": "refusal", "content": [{"type": "text", "text": "x"}]});
        assert_eq!(parse_response(Provider::Anthropic, &refusal), None);
        assert_eq!(parse_response(Provider::Anthropic, &json!({"error": {}})), None);
    }

    #[test]
    fn cd_validation() {
        let d = tmp_dir("cd");
        std::fs::create_dir_all(d.join("a/b")).unwrap();
        std::fs::create_dir_all(d.join("space in name")).unwrap();
        std::fs::write(d.join("file"), "x").unwrap();
        std::fs::write(d.join("a/file"), "x").unwrap();
        let home = d.to_string_lossy().into_owned();
        let env: HashMap<String, String> =
            [("HOME", home.as_str()), ("A", "a"), ("B", "b")].into_iter().map(|(k, v)| (k.into(), v.into())).collect();
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
        for bad in ["cd /folder/doesnt/exist", "cd ~/file", "cd ~/$A/file", "cd b", "cd file", "cd $B", "cd a/b/file"] {
            assert!(!valid(bad), "{bad}");
        }
        assert!(!valid(&format!("cd {home}/file")));
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn history_ghost_rules() {
        let h = [entry("git status", Some(0)), entry("git stash", Some(1)), entry("git switch", None)];
        assert_eq!(history_ghost(&h, "git st").as_deref(), Some("atus"), "newest successful wins");
        assert_eq!(history_ghost(&h, "git sw").as_deref(), Some("itch"), "fallback: any");
        assert_eq!(history_ghost(&h, "git status"), None);
        assert_eq!(history_ghost(&h, ""), None);
        assert_eq!(history_ghost(&h, "ls"), None);
        let h2 = [entry("git stash", Some(0)), entry("git status", Some(0))];
        assert_eq!(history_ghost(&h2, "git st").as_deref(), Some("atus"));
    }

    fn item(kind: ItemType, delete: usize, text: &str, prefix: bool) -> Item {
        Item {
            names: vec!["n".into()],
            display_name: None,
            description: None,
            kind,
            original_kind: None,
            priority: 50.0,
            is_dangerous: false,
            args_hint: String::new(),
            highlight: Vec::new(),
            query: String::new(),
            insert: Insertion { delete, text: text.into(), cursor_back: 0, execute: false },
            generator: None,
            prefix_match: prefix,
            template_type: None,
        }
    }

    #[test]
    fn spec_ghost_rules() {
        let items = [
            item(ItemType::AutoExecute, 0, "x", true),
            item(ItemType::History, 0, "y", true),
            item(ItemType::Subcommand, 2, "checkout ", true),
            item(ItemType::Subcommand, 0, "erry-pick ", false),
            item(ItemType::Option, 0, " ", true),
            item(ItemType::Subcommand, 0, "eckout ", true),
            item(ItemType::Subcommand, 0, "erry ", true),
        ];
        assert_eq!(spec_ghost(&items).as_deref(), Some("eckout"));
        assert_eq!(spec_ghost(&items[..5]), None);
        assert_eq!(spec_ghost(&[item(ItemType::Arg, 0, "a  ", true)]).as_deref(), Some("a "));
        // spec-ghost-ignores-cursor: `commit -m '{cursor}'` would leave the cursor outside the quotes; `--out=`
        // (cursor right after `=`, before the auto-space) is fine
        let with_back = |text: &str, back| {
            let mut i = item(ItemType::Arg, 0, text, true);
            i.insert.cursor_back = back;
            i
        };
        assert_eq!(spec_ghost(&[with_back("commit -m ''", 1)]), None);
        assert_eq!(spec_ghost(&[with_back("author= ", 1)]).as_deref(), Some("author="));
        assert_eq!(spec_ghost(&[with_back("author= ", 2)]), None);
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
        assert_eq!(names, ["bad", "claude", "history", "local", "nomodel", "str", "tokens", "url"]);
        let get = |n: &str| p.iter().find(|(k, _)| k == n).unwrap().1.clone();
        let claude = get("claude").unwrap();
        assert_eq!(
            claude,
            Profile {
                name: "claude".into(),
                provider: Provider::Anthropic,
                model: "claude-opus-5-5".into(),
                base_url: "https://api.anthropic.com".into(),
                api_key_env: "ANTHROPIC_API_KEY".into(),
                max_tokens: 1024,
                effort: Some("low".into()),
            }
        );
        let local = get("local").unwrap();
        assert_eq!((local.base_url.as_str(), local.api_key_env.as_str(), local.max_tokens), ("http://localhost:11434/v1", "NONE", 32));
        assert!(get("bad").unwrap_err().contains("gemini"));
        assert_eq!(get("nomodel").unwrap_err(), "missing model");
        assert_eq!(get("str").unwrap_err(), "not a JSON object");
        assert!(get("history").unwrap_err().contains("reserved"));
        assert!(get("tokens").unwrap_err().contains("max_tokens"));
        assert_eq!(get("url").unwrap_err(), "base_url must be a string");

        assert_eq!(source(&s), Ok(Source::Profile(claude)));
        let openai = settings(json!({"inline.source": "o", "inline.profiles.o": {"provider": "openai", "model": "g"}}));
        let Ok(Source::Profile(o)) = source(&openai) else { panic!() };
        assert_eq!((o.base_url.as_str(), o.api_key_env.as_str(), o.max_tokens), ("https://api.openai.com/v1", "OPENAI_API_KEY", 64));
        assert_eq!(source(&settings(json!({}))), Ok(Source::HistoryAndSpecs));
        assert_eq!(source(&settings(json!({"inline.source": "history"}))), Ok(Source::History));
        assert_eq!(source(&settings(json!({"inline.source": "specs"}))), Ok(Source::Specs));
        assert!(source(&settings(json!({"inline.source": "nope"}))).unwrap_err().contains("not found"));
        let bad = settings(json!({"inline.source": "b", "inline.profiles.b": {"provider": "openai"}}));
        assert!(source(&bad).unwrap_err().contains("invalid"));
    }

    #[test]
    fn history_file_round_trip_and_compaction() {
        let d = tmp_dir("hist");
        let path = d.join("sub/history.jsonl");
        let mut h = History::load_from(path.clone());
        assert!(h.entries().is_empty());
        h.append(entry("echo hi", Some(0)));
        h.append(entry("   ", Some(0)));
        h.append(HistoryEntry { pid: Some(7), cwd: Some("/x".into()), ..entry("ls", None) });
        let again = History::load_from(path.clone());
        assert_eq!(again.entries(), h.entries());
        assert_eq!(again.entries().len(), 2);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        }

        // 10 000 lines (one corrupt) stay; the append that goes past 10 000 rewrites the file with the last 5000.
        let lines = |n: usize| -> String {
            (0..n).map(|i| serde_json::to_string(&entry(&format!("c{i}"), Some(0))).unwrap() + "\n").collect()
        };
        std::fs::write(&path, String::from("not json\n") + &lines(HISTORY_COMPACT_AT - 1)).unwrap();
        let mut h = History::load_from(path.clone());
        assert_eq!(h.entries().len(), HISTORY_IN_MEMORY);
        assert_eq!(std::fs::read_to_string(&path).unwrap().lines().count(), HISTORY_COMPACT_AT, "not yet");
        h.append(entry("newest", Some(0)));
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text.lines().count(), HISTORY_IN_MEMORY);
        assert!(text.lines().last().unwrap().contains("\"command\":\"newest\""));
        assert_eq!(h.entries().len(), HISTORY_IN_MEMORY);
        assert_eq!(h.entries().last().unwrap().command, "newest");
        let reloaded = History::load_from(path.clone());
        assert_eq!(reloaded.entries().first().unwrap().command, "c5000");
        assert_eq!(reloaded.entries().len(), HISTORY_IN_MEMORY);

        // A file already past 10 000 lines is compacted at load.
        std::fs::write(&path, lines(HISTORY_COMPACT_AT + 1)).unwrap();
        let h = History::load_from(path.clone());
        assert_eq!(std::fs::read_to_string(&path).unwrap().lines().count(), HISTORY_IN_MEMORY);
        assert_eq!(h.entries().first().unwrap().command, "c5001");
        assert!(!d.join("sub").read_dir().unwrap().any(|e| e.unwrap().file_name().to_string_lossy().contains("tmp")));
        std::fs::remove_dir_all(&d).unwrap();
    }
}
