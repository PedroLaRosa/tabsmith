use std::collections::HashMap;

/// The path a `cd` completion would enter (`full` = buffer + completion), or `None` to accept: not a `cd` with
/// one argument, an open quote, or something the env cannot expand. `fallback_home` is read only for a `~` target
/// the env's HOME/USERPROFILE does not resolve.
pub(crate) fn cd_target(
    full: &str,
    env: &HashMap<String, String>,
    fallback_home: impl FnOnce() -> Option<String>,
) -> Option<String> {
    let words = split_words(full.trim())?;
    let [cd, arg] = words.as_slice() else {
        return None;
    };
    if cd != "cd" {
        return None;
    }
    expand(arg, env, fallback_home)
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
fn expand(
    arg: &str,
    env: &HashMap<String, String>,
    fallback_home: impl FnOnce() -> Option<String>,
) -> Option<String> {
    let mut out = String::new();
    let mut s = arg;
    if arg == "~" || arg.starts_with("~/") || (cfg!(windows) && arg.starts_with("~\\")) {
        let home = env
            .get("HOME")
            .or_else(|| env.get("USERPROFILE"))
            .cloned()
            .or_else(fallback_home)?;
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
    use std::cell::Cell;

    fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn should_read_the_fallback_home_only_for_a_tilde_target_the_env_does_not_resolve() {
        let reads = Cell::new(0);
        let fallback = || {
            reads.set(reads.get() + 1);
            Some("/f".to_string())
        };

        let targets = [
            cd_target("cd ~/a", &env(&[("HOME", "/h")]), fallback),
            cd_target("cd ~/a", &env(&[("USERPROFILE", "/u")]), fallback),
            cd_target("cd a", &env(&[]), fallback),
            cd_target("echo ~", &env(&[]), fallback),
            cd_target("cd ~/a", &env(&[]), fallback),
        ];

        assert_eq!(
            (targets, reads.get(), cd_target("cd ~/a", &env(&[]), || None)),
            (
                [Some("/h/a".to_string()), Some("/u/a".to_string()), Some("a".to_string()), None, Some("/f/a".to_string())],
                1,
                None
            )
        );
    }
}
