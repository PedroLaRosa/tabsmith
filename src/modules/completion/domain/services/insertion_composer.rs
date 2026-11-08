use crate::modules::completion::domain::services::fuzzy_matcher::fuzzysort;
use crate::modules::completion::domain::services::suggestion_filter::prefix_score;
use crate::modules::completion::domain::value_objects::insertion::{Insertion, InsertionParts};
use crate::modules::completion::domain::value_objects::insertion_context::InsertionContext;
use crate::modules::completion::domain::value_objects::suggestion_candidate::SuggestionCandidate;
use crate::modules::completion::domain::value_objects::suggestion_query::{after_sep, SuggestionQuery};
use crate::modules::shared::domain::services::shell_quoting;
use crate::modules::shared::domain::value_objects::shell::Shell;
use crate::modules::specs::domain::value_objects::suggestion_kind::SuggestionKind;

/// `getNameMatch`: which alias to insert.
fn name_match(s: &SuggestionCandidate, q: &str, fuzzy: bool, verbose: bool) -> Option<String> {
    let pick = |names: Vec<&String>| -> Option<String> {
        if verbose && matches!(s.kind(), SuggestionKind::Option | SuggestionKind::Subcommand) {
            let mut best: Option<&String> = None;
            for n in names {
                if n.chars().count() > best.map_or(0, |b| b.chars().count()) {
                    best = Some(n);
                }
            }
            best.cloned()
        } else {
            names.first().map(|n| n.to_string())
        }
    };
    let matched: Vec<&String> = s
        .names()
        .iter()
        .filter(|n| {
            if fuzzy {
                q.is_empty() || fuzzysort(q, n).is_some()
            } else {
                prefix_score(n, q).is_some()
            }
        })
        .collect();
    if !matched.is_empty() || s.display_name().is_none() {
        pick(matched)
    } else {
        pick(s.names().iter().collect())
    }
}

/// A bare variable reference (`$HOME`, pwsh `$env:PATH`): shell syntax the row means to insert.
fn is_var_ref(s: &str) -> bool {
    s.strip_prefix('$').is_some_and(|v| {
        !v.is_empty()
            && v.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == ':')
    })
}

/// POSIX/fish escaping of a name. `quote`: the user started the token with a quote → keep quoting; `path`: a
/// file/folder row (its `$` is never a variable). An allow-list, since a deny-list
/// misses `$ ` ; { }` and a leading `~`/`=` (SPEC §6.8 D-note).
fn escape_posix(s: &str, folder: bool, quote: bool, path: bool, shell: Shell) -> String {
    let safe = |c: char| c.is_alphanumeric() || "-_./+,:@%=~".contains(c);
    // A file name's `~` expands at the start and after `=`/`:` (assignments, zsh MAGIC_EQUAL_SUBST); `=cmd` in
    // zsh. A spec row (`cd ~`) means that expansion.
    let tilde = path && (s.starts_with(['~', '=', '%']) || s.contains("=~") || s.contains(":~"));
    if !quote && !tilde {
        if s.chars().all(safe) || (!path && is_var_ref(s)) {
            return s.to_string();
        }
        if s.chars().all(|c| safe(c) || c == ' ') {
            return s.replace(' ', "\\ ");
        }
    }
    match s.strip_suffix('/').filter(|_| folder) {
        Some(stem) => format!("{}/", shell_quoting::quote(shell, stem)),
        None => shell_quoting::quote(shell, s),
    }
}

/// PowerShell escaping: single quotes when needed. Folders become backtick-escaped bare words instead, so the
/// cursor never sits inside a quote (the mid-word guard would block the next segment).
fn escape_pwsh(s: &str, folder: bool, quote: bool, path: bool) -> String {
    // PowerShell also delimits strings with the Unicode single (‘ ’ ‚ ‛) and double (“ ” „) quotes
    const SPECIAL: &str =
        "$`'\"(){};,&|@#<>\u{2018}\u{2019}\u{201A}\u{201B}\u{201C}\u{201D}\u{201E}";
    let special = |c: char| c.is_whitespace() || SPECIAL.contains(c);
    if !quote && (!s.chars().any(special) || (!path && is_var_ref(s))) {
        return s.to_string();
    }
    if folder {
        return s
            .chars()
            .flat_map(|c| ['`', c].into_iter().skip(usize::from(!special(c))))
            .collect();
    }
    shell_quoting::quote(Shell::Pwsh, s)
}

/// `(char index, char)`.
type At = (usize, char);

/// Each logical char of `raw` (quotes removed, escapes resolved) with the char index it starts at, and the quote
/// `raw` leaves open (`'` or `"`) with its index.
fn unquote(raw: &[char], shell: Shell) -> (Vec<At>, Option<(char, usize)>) {
    let pwsh = shell == Shell::Pwsh;
    let esc = if pwsh { '`' } else { '\\' };
    let (mut out, mut open, mut i) = (Vec::new(), None, 0);
    while i < raw.len() {
        let ch = raw[i];
        match open {
            // no escapes inside single quotes, except pwsh `''`
            Some(('\'', _)) if ch == '\'' && pwsh && raw.get(i + 1) == Some(&'\'') => {
                out.push((i, ch));
                i += 1;
            }
            Some(('\'', _)) if ch == '\'' => open = None,
            Some(('\'', _)) => out.push((i, ch)),
            _ if ch == esc && i + 1 < raw.len() => {
                out.push((i, raw[i + 1]));
                i += 1;
            }
            Some((q, _)) if ch == q => open = None,
            None if ch == '\'' || ch == '"' => open = Some((ch, i)),
            _ => out.push((i, ch)),
        }
        i += 1;
    }
    (out, open)
}

/// Escaping inside a quote the user opened before the replaced text (no quotes added around it).
fn escape_in_quote(s: &str, q: char, shell: Shell) -> String {
    match (q, shell) {
        ('\'', Shell::Pwsh) => {
            let q = shell_quoting::quote(Shell::Pwsh, s);
            q[1..q.len() - 1].to_string() // the escaping without the `'…'` around it
        }
        ('\'', Shell::Fish) => s.replace('\\', "\\\\").replace('\'', "\\'"),
        ('\'', _) => s.replace('\'', "'\\''"),
        _ => {
            let (special, esc) = match shell {
                Shell::Pwsh => ("`\"$\u{201C}\u{201D}\u{201E}", '`'),
                Shell::Fish => ("\\\"$", '\\'), // fish double quotes know only \\ \" \$
                _ => ("\\\"$`", '\\'),
            };
            s.chars()
                .map(|c| match c {
                    // interactive bash/zsh history-expand `!` even inside double quotes: single-quote it in between
                    '!' if matches!(shell, Shell::Bash | Shell::Zsh) => "\"'!'\"".to_string(),
                    c if special.contains(c) => format!("{esc}{c}"),
                    c => c.to_string(),
                })
                .collect()
        }
    }
}

/// Precomputed full insertion for a row (SPEC §6.8, D22/D25).
pub(crate) fn insertion(s: &SuggestionCandidate, c: &InsertionContext) -> Insertion {
    let q = s.query().of(c.search_term());
    let raw: Vec<char> = c.raw().chars().collect();
    let (logical, open) = unquote(&raw, c.shell());
    // special (`help` template) rows replace the typed prefix like any other row
    let delete = if s.kind() == SuggestionKind::AutoExecute {
        0
    } else if q == c.search_term() {
        raw.len()
    } else if let SuggestionQuery::Str(g) = s.query() {
        c.raw()[c.raw().rfind(g.as_str()).map_or(0, |i| i + g.len())..]
            .chars()
            .count()
    } else if let SuggestionQuery::Path { backslash } = *s.query() {
        c.raw()[after_sep(c.raw(), backslash)..].chars().count()
    } else {
        // function getQueryTerm: delete the raw chars that spell q's logical chars (quotes/escapes included);
        // otherwise an approximation (escaped whitespace), never past the token
        let n = q.chars().count();
        let tail = logical
            .get(logical.len().saturating_sub(n)..)
            .filter(|t| t.len() == n && t.iter().map(|x| x.1).eq(q.chars()));
        match tail {
            Some(t) => raw.len() - t.first().map_or(raw.len(), |x| x.0),
            None => q
                .chars()
                .map(|ch| if ch.is_whitespace() { 2 } else { 1 })
                .sum::<usize>()
                .min(raw.len()),
        }
    };
    let deleted: String = raw[raw.len() - delete..].iter().collect();
    let quote = deleted.starts_with(['\'', '"']);
    // a quote opened before the replaced part (`cat "my dir/x`): escape for inside it
    let inside = open
        .filter(|&(_, i)| i < raw.len() - delete)
        .map(|(q, _)| q);
    let (folder, path) = (
        s.kind() == SuggestionKind::Folder,
        matches!(s.kind(), SuggestionKind::File | SuggestionKind::Folder),
    );
    let mut text = match s.insert_value() {
        Some(v) if !path => v.clone(),
        _ => match name_match(s, &q, c.fuzzy(), c.prefer_verbose()).filter(|m| !m.is_empty()) {
            None => String::new(),
            Some(m) => {
                let name = match (inside, c.shell()) {
                    // a file closes the quote; a folder leaves it open for the next segment
                    (Some(qc), sh) => {
                        let mut t = escape_in_quote(&m, qc, sh);
                        t.extend((s.kind() == SuggestionKind::File).then_some(qc));
                        t
                    }
                    (None, Shell::Pwsh) => escape_pwsh(&m, folder, quote, path),
                    (None, sh) => escape_posix(&m, folder, quote, path, sh),
                };
                // only the name is escaped: `--out=` keeps its separator and the cursor outside any quote
                match s.separator() {
                    Some(sep) => format!("{name}{sep}{{cursor}}"),
                    None => name,
                }
            }
        },
    };
    if text.ends_with('\n') {
        let n = text.trim_end_matches('\n').len();
        text.truncate(n);
        text.push('\n');
    }
    if !text.ends_with('\n') && s.should_add_space() && c.insert_space() {
        text.push(' ');
    }
    let mut delete = delete;
    // don't retype what is already typed (only when the deleted raw chars are exactly the query)
    if !q.is_empty() && deleted == q && text.starts_with(&q) {
        delete = 0; // the deleted chars are exactly q
        text.drain(..q.len());
    }
    let execute = text.ends_with('\n');
    if execute {
        text.pop();
    }
    // Names from directories, generators and specs reach the PTY as typed bytes: a CR/LF/ESC/... in them would
    // run or rewrite the line. Only the trailing "\n" (→ `execute`) is meaningful.
    text.retain(|ch| !ch.is_control());
    let mut cursor_back = 0;
    if let Some(i) = text.find("{cursor}") {
        text.replace_range(i..i + "{cursor}".len(), "");
        cursor_back = text[i..].chars().count();
    }
    Insertion::new(InsertionParts {
        delete,
        text,
        cursor_back,
        execute,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::completion::domain::services::suggestion_filter::auto_exec;
    use crate::modules::completion::domain::test_fixtures::ctx;

    #[test]
    fn insertion_examples() {
        let status = SuggestionCandidate::new("status", SuggestionKind::Subcommand).with_should_add_space(true);
        let i = insertion(&status, &ctx("st", "st", Shell::Bash));
        assert_eq!((i.delete(), i.text()), (0, "atus "));
        let checkout = SuggestionCandidate::new("checkout", SuggestionKind::Subcommand).with_should_add_space(true);
        assert_eq!(
            insertion(&checkout, &ctx("ch", "ch", Shell::Zsh)).text(),
            "eckout "
        );
        // case differs → retype the whole token
        let i = insertion(&checkout, &ctx("CH", "CH", Shell::Zsh));
        assert_eq!((i.delete(), i.text()), (2, "checkout "));
        // requiresSeparator + mandatory arg: `--out= ` with the cursor right after `=`
        let out = SuggestionCandidate::new("--out", SuggestionKind::Option)
            .with_separator(Some("=".into()))
            .with_should_add_space(true);
        let i = insertion(&out, &ctx("", "", Shell::Bash));
        assert_eq!(
            i,
            Insertion::new(InsertionParts {
                delete: 0,
                text: "--out= ".into(),
                cursor_back: 1,
                execute: false
            })
        );
        let i = insertion(&out, &ctx("--o", "--o", Shell::Bash));
        assert_eq!(
            i,
            Insertion::new(InsertionParts {
                delete: 0,
                text: "ut= ".into(),
                cursor_back: 1,
                execute: false
            })
        );
        // auto-space off
        let c = ctx("ch", "ch", Shell::Zsh).with_insert_space(false);
        assert_eq!(insertion(&checkout, &c).text(), "eckout");
        // escaped space in the raw token
        let dir = SuggestionCandidate::new("my dir/", SuggestionKind::Folder)
            .with_insert_value(Some("my dir/".into()))
            .with_query(SuggestionQuery::Str("/".into()));
        let i = insertion(&dir, &ctx("my di", "my\\ di", Shell::Bash));
        assert_eq!((i.delete(), i.text()), (6, "my\\ dir/"));
        // quoted raw token
        let file = SuggestionCandidate::new("my file", SuggestionKind::File).with_query(SuggestionQuery::Str("/".into()));
        let i = insertion(&file, &ctx("my fi", "'my fi", Shell::Bash));
        assert_eq!((i.delete(), i.text()), (6, "'my file'"));
        // getQueryTerm "/": only the part after the last "/" is deleted / not retyped
        let main = SuggestionCandidate::new("main.rs", SuggestionKind::File).with_query(SuggestionQuery::Str("/".into()));
        let i = insertion(&main, &ctx("src/ma", "src/ma", Shell::Bash));
        assert_eq!((i.delete(), i.text()), (0, "in.rs"));
        // POSIX special chars → single quotes, embedded quote escaped
        let i = insertion(
            &SuggestionCandidate::new("it's", SuggestionKind::File),
            &ctx("", "", Shell::Bash),
        );
        assert_eq!(i.text(), "'it'\\''s'");
        let i = insertion(
            &SuggestionCandidate::new("a(b)/", SuggestionKind::Folder),
            &ctx("", "", Shell::Bash),
        );
        assert_eq!(i.text(), "'a(b)'/");
        // pwsh
        let i = insertion(
            &SuggestionCandidate::new("my file", SuggestionKind::File),
            &ctx("", "", Shell::Pwsh),
        );
        assert_eq!(i.text(), "'my file'");
        let i = insertion(
            &SuggestionCandidate::new("it's", SuggestionKind::File),
            &ctx("", "", Shell::Pwsh),
        );
        assert_eq!(i.text(), "'it''s'");
        let i = insertion(
            &SuggestionCandidate::new("my dir/", SuggestionKind::Folder),
            &ctx("", "", Shell::Pwsh),
        );
        assert_eq!((i.text(), i.cursor_back()), ("my` dir/", 0));
        let i = insertion(&out, &ctx("", "", Shell::Pwsh));
        assert_eq!((i.text(), i.cursor_back()), ("--out= ", 1));
        // insertValue verbatim with {cursor}; "\n" → execute; auto-execute deletes nothing
        let iv = SuggestionCandidate::new("x", SuggestionKind::Arg).with_insert_value(Some("a{cursor}b\n\n".into()));
        assert_eq!(
            insertion(&iv, &ctx("x", "x", Shell::Bash)),
            Insertion::new(InsertionParts {
                delete: 1,
                text: "ab".into(),
                cursor_back: 1,
                execute: true
            })
        );
        let ae = auto_exec("↪", "", None);
        assert_eq!(
            insertion(&ae, &ctx("x", "x", Shell::Bash)),
            Insertion::new(InsertionParts {
                delete: 0,
                text: String::new(),
                cursor_back: 0,
                execute: true
            })
        );
        // option chain arg rows: query "" → nothing deleted
        let a = SuggestionCandidate::new("val", SuggestionKind::Arg).with_query(SuggestionQuery::Value(String::new()));
        assert_eq!(insertion(&a, &ctx("-la", "-la", Shell::Bash)).delete(), 0);
        // preferVerboseSuggestions
        let o = SuggestionCandidate::new("", SuggestionKind::Option).with_names(vec!["-m".into(), "--message".into()]);
        assert_eq!(insertion(&o, &ctx("", "", Shell::Bash)).text(), "-m");
        let c = ctx("", "", Shell::Bash).with_prefer_verbose(true);
        assert_eq!(insertion(&o, &c).text(), "--message");
        let i = insertion(&o, &ctx("--m", "--m", Shell::Bash));
        assert_eq!((i.delete(), i.text()), (0, "essage"));
    }

    #[test]
    fn insertion_review_regressions() {
        // composite-optarg-delete: the parser hands over only the value (`al` of `--color=al`)
        let always = SuggestionCandidate::new("always", SuggestionKind::Arg);
        assert_eq!(
            insertion(&always, &ctx("al", "al", Shell::Zsh)),
            Insertion::new(InsertionParts {
                delete: 0,
                text: "ways".into(),
                cursor_back: 0,
                execute: false
            })
        );
        // insert-control-chars: CR/LF/ESC/DEL never reach the PTY; a trailing "\n" still means execute
        for (kind, name) in [
            (SuggestionKind::File, "a\rtouch${IFS}PWNED\r"),
            (SuggestionKind::Arg, "x\x1b[2J\nrm\x7f"),
        ] {
            let i = insertion(&SuggestionCandidate::new(name, kind), &ctx("", "", Shell::Bash));
            assert!(
                !i.text().chars().any(char::is_control) && !i.executes(),
                "{name:?} → {i:?}"
            );
        }
        let generator = SuggestionCandidate::new("x", SuggestionKind::Arg).with_insert_value(Some("dev\rrm -rf ~\n".into()));
        assert_eq!(
            insertion(&generator, &ctx("", "", Shell::Bash)),
            Insertion::new(InsertionParts {
                delete: 0,
                text: "devrm -rf ~".into(),
                cursor_back: 0,
                execute: true
            })
        );
        // special-rows-no-delete: `help` rows replace the typed prefix instead of doubling it
        let build = SuggestionCandidate::new("build", SuggestionKind::Special);
        assert_eq!(insertion(&build, &ctx("bu", "bu", Shell::Bash)).text(), "ild");
        let i = insertion(&build, &ctx("BU", "BU", Shell::Bash));
        assert_eq!((i.delete(), i.text()), (2, "build"));
        // open-quote-escaping: inside a quote opened before the replaced segment, escape for that quote
        let path = |name: &str, kind| SuggestionCandidate::new(name, kind).with_query(SuggestionQuery::Path { backslash: false });
        let file = path("x y.txt", SuggestionKind::File);
        let i = insertion(&file, &ctx("my dir/x", "\"my dir/x", Shell::Zsh));
        assert_eq!((i.delete(), i.text()), (0, " y.txt\""));
        let i = insertion(&file, &ctx("my dir/x", "'my dir/x", Shell::Bash));
        assert_eq!((i.delete(), i.text()), (0, " y.txt'"));
        let i = insertion(
            &path("sub dir/", SuggestionKind::Folder),
            &ctx("my dir/", "\"my dir/", Shell::Bash),
        );
        assert_eq!((i.delete(), i.text()), (0, "sub dir/")); // folders leave the quote open
        assert_eq!(
            insertion(
                &path("a\"b$`\\", SuggestionKind::File),
                &ctx("d/", "\"d/", Shell::Bash)
            )
            .text(),
            "a\\\"b\\$\\`\\\\\""
        );
        assert_eq!(
            insertion(
                &path("it's", SuggestionKind::File),
                &ctx("d/", "'d/", Shell::Bash)
            )
            .text(),
            "it'\\''s'"
        );
        assert_eq!(
            insertion(
                &path("it's", SuggestionKind::File),
                &ctx("d/", "'d/", Shell::Pwsh)
            )
            .text(),
            "it''s'"
        );
        assert_eq!(
            insertion(
                &path("a\"$b", SuggestionKind::File),
                &ctx("d/", "\"d/", Shell::Pwsh)
            )
            .text(),
            "a`\"`$b\""
        );
        // closed or escaped quotes, or a quote inside the replaced segment: unchanged escaping
        assert_eq!(
            insertion(&file, &ctx("my dir/x", "\"my dir\"/x", Shell::Bash)).text(),
            "\\ y.txt"
        );
        assert_eq!(
            insertion(&file, &ctx("\"d/x", "\\\"d/x", Shell::Bash)).text(),
            "\\ y.txt"
        );
        assert_eq!(
            insertion(&file, &ctx("d/x", "d/\"x", Shell::Bash)).text(),
            "'x y.txt'"
        );
    }

    #[test]
    fn insertion_round2_regressions() {
        let text = |s: SuggestionCandidate, sh| insertion(&s, &ctx("", "", sh)).text().to_string();
        // posix-escape-misses-shell-metachars / path-metachar-injection: names never become live shell syntax
        for sh in [Shell::Bash, Shell::Zsh, Shell::Fish] {
            for (name, kind) in [
                ("zz;id", SuggestionKind::Arg),
                ("a$b.txt", SuggestionKind::File),
                ("a`id`b", SuggestionKind::File),
                ("x{1,2}.txt", SuggestionKind::File),
                ("notes;touch${IFS}PWNED.txt", SuggestionKind::File),
                ("semi;colon dir.txt", SuggestionKind::File),
                ("~x", SuggestionKind::File),
                ("=cmd", SuggestionKind::File),
                ("a=~b", SuggestionKind::File),
            ] {
                assert_eq!(
                    text(SuggestionCandidate::new(name, kind), sh),
                    format!("'{name}'"),
                    "{sh:?} {name}"
                );
            }
            assert_eq!(text(SuggestionCandidate::new("$b;x/", SuggestionKind::Folder), sh), "'$b;x'/");
            assert_eq!(
                text(SuggestionCandidate::new("a b-c_d.e+f,g:h@i%j=k~", SuggestionKind::File), sh),
                "a\\ b-c_d.e+f,g:h@i%j=k~"
            );
        }
        assert_eq!(
            text(SuggestionCandidate::new("it's\\", SuggestionKind::File), Shell::Fish),
            "'it\\'s\\\\'"
        );
        // a separator stays outside the escaping (`--out=` + cursor), even when the name needs quotes
        let sep = SuggestionCandidate::new("--a;b", SuggestionKind::Option).with_separator(Some("=".into()));
        assert_eq!(
            insertion(&sep, &ctx("", "", Shell::Bash)),
            Insertion::new(InsertionParts {
                delete: 0,
                text: "'--a;b'=".into(),
                cursor_back: 0,
                execute: false
            })
        );
        // pwsh-quotes-variable-refs: a `$NAME` row stays a variable reference; a file named so is still quoted
        for sh in [Shell::Bash, Shell::Fish, Shell::Pwsh] {
            let i = insertion(&SuggestionCandidate::new("$HOME", SuggestionKind::Arg), &ctx("$HO", "$HO", sh));
            assert_eq!((i.delete(), i.text()), (0, "ME"), "{sh:?}");
            assert_eq!(text(SuggestionCandidate::new("$foo", SuggestionKind::File), sh), "'$foo'");
        }
        assert_eq!(
            text(SuggestionCandidate::new("$env:PATH", SuggestionKind::Arg), Shell::Pwsh),
            "$env:PATH"
        );
        assert_eq!(
            text(SuggestionCandidate::new("a$b.txt", SuggestionKind::File), Shell::Pwsh),
            "'a$b.txt'"
        );
        // pwsh-unicode-smart-quote-injection: pwsh ends a '…' string at ’ too, so smart single quotes are doubled
        assert_eq!(
            text(SuggestionCandidate::new("r’;x;’.txt", SuggestionKind::File), Shell::Pwsh),
            "'r’’;x;’’.txt'"
        );
        // pwsh-quoted-folder-midword-guard: folders are backtick-escaped bare words (cursor at the end), and a
        // quoted file can follow in the same word
        let i = insertion(
            &SuggestionCandidate::new("dir with space/", SuggestionKind::Folder),
            &ctx("di", "'di", Shell::Pwsh),
        );
        assert_eq!(
            (i.delete(), i.text(), i.cursor_back()),
            (3, "dir` with` space/", 0)
        );
        assert_eq!(
            text(SuggestionCandidate::new("a$b'(c)/", SuggestionKind::Folder), Shell::Pwsh),
            "a`$b`'`(c`)/"
        );
        let file = SuggestionCandidate::new("a b.txt", SuggestionKind::File).with_query(SuggestionQuery::Path { backslash: true });
        let i = insertion(
            &file,
            &ctx("dir with space/", "dir` with` space/", Shell::Pwsh),
        );
        assert_eq!((i.delete(), i.text()), (0, "'a b.txt'"));
        // fn-queryterm-quoted-delete: a function getQueryTerm deletes the raw chars spelling the query
        let fq = |name: &str, kind, q: &str| SuggestionCandidate::new(name, kind).with_query(SuggestionQuery::Value(q.into()));
        for (raw, sh) in [
            ("\"dir with space/sub d", Shell::Zsh),
            ("'dir with space/sub d", Shell::Bash),
            ("\"dir with space/sub d", Shell::Fish),
            ("'dir with space/sub d", Shell::Pwsh),
        ] {
            let i = insertion(
                &fq("sub dir/", SuggestionKind::Folder, "sub d"),
                &ctx("dir with space/sub d", raw, sh),
            );
            assert_eq!((i.delete(), i.text()), (0, "ir/"), "{raw} {sh:?}");
        }
        let i = insertion(
            &fq("my file.py", SuggestionKind::File, "my fi"),
            &ctx(
                "dir with space/my fi",
                "\"dir with space/my fi",
                Shell::Bash,
            ),
        );
        assert_eq!((i.delete(), i.text()), (0, "le.py\""));
        let i = insertion(
            &fq("my file.py", SuggestionKind::File, "my fi"),
            &ctx(
                "dir with space/my fi",
                "dir\\ with\\ space/my\\ fi",
                Shell::Bash,
            ),
        );
        assert_eq!((i.delete(), i.text()), (6, "my\\ file.py"));
        let i = insertion(
            &fq("my file.py", SuggestionKind::File, "my fi"),
            &ctx(
                "dir with space/my fi",
                "dir` with` space/my` fi",
                Shell::Pwsh,
            ),
        );
        assert_eq!((i.delete(), i.text()), (6, "'my file.py'"));
        let i = insertion(
            &fq("it's.txt", SuggestionKind::File, "it's"),
            &ctx("d/it's", "'d/it''s", Shell::Pwsh),
        );
        assert_eq!((i.delete(), i.text()), (5, "it''s.txt'"));
        // dq-bang-history-expansion: `!` inside bash/zsh double quotes; fish has no backtick escape there
        let path = |name: &str| SuggestionCandidate::new(name, SuggestionKind::File).with_query(SuggestionQuery::Path { backslash: false });
        for sh in [Shell::Bash, Shell::Zsh] {
            assert_eq!(
                insertion(&path("wow!x"), &ctx("d/", "\"d/", sh)).text(),
                "wow\"'!'\"x\""
            );
        }
        assert_eq!(
            insertion(&path("a`b!$"), &ctx("d/", "\"d/", Shell::Fish)).text(),
            "a`b!\\$\""
        );
        assert_eq!(
            insertion(&path("a\\b'"), &ctx("d/", "'d/", Shell::Fish)).text(),
            "a\\\\b\\''"
        );
        // r3-escape-quotes-spec-tilde: a spec row's leading `~`/`%` is shell syntax (`cd ~`, `kill %1`)
        for sh in [Shell::Bash, Shell::Zsh, Shell::Fish] {
            assert_eq!(text(SuggestionCandidate::new("~", SuggestionKind::Arg), sh), "~", "{sh:?}");
            assert_eq!(text(SuggestionCandidate::new("%1", SuggestionKind::Arg), sh), "%1", "{sh:?}");
        }
        // pwsh-unicode-smart-quote-injection: PowerShell also ends strings at ‘ ’ ‚ ‛ (single) and “ ” „ (double):
        // they force quoting (shell_quoting::quote escapes them), are backtick-escaped in folders and open `"`
        let sq = |s: &str| shell_quoting::quote(Shell::Pwsh, s);
        for name in [
            "plain’name.txt",
            "report’;New-Item -Name TS -ItemType File;’.txt",
            "a‘b‚c‛d“e”f„g",
        ] {
            for kind in [SuggestionKind::File, SuggestionKind::Arg] {
                assert_eq!(
                    text(SuggestionCandidate::new(name, kind), Shell::Pwsh),
                    sq(name),
                    "{name} {kind:?}"
                );
            }
            assert_eq!(
                insertion(&path(name), &ctx("d/", "'d/", Shell::Pwsh)).text().to_string(),
                sq(name)[1..],
                "{name} in '"
            );
        }
        assert_eq!(
            text(SuggestionCandidate::new("x’;calc;‘/", SuggestionKind::Folder), Shell::Pwsh),
            "x`’`;calc`;`‘/"
        );
        assert_eq!(
            insertion(&path("a”;calc;“„"), &ctx("d/", "\"d/", Shell::Pwsh)).text(),
            "a`”;calc;`“`„\""
        );
    }
}
