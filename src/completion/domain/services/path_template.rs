use crate::completion::domain::value_objects::annotation_kind::AnnotationKind;
use crate::completion::domain::value_objects::parsed_command_line::ParsedCommandLine;
use crate::completion::domain::value_objects::suggestion_query::after_sep;
use crate::specs::domain::entities::subcommand::Subcommand;
use crate::specs::domain::value_objects::spec_suggestion::SpecSuggestion;
use crate::specs::domain::value_objects::suggestion_kind::SuggestionKind;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Bundled `shellExpand`: `~`, `$NAME`, `${NAME}` / `${NAME:-default}` from `env`.
pub(crate) fn shell_expand(s: &str, env: &HashMap<String, String>) -> String {
    let home = env
        .get("HOME")
        .or_else(|| env.get("USERPROFILE"))
        .map_or("~", String::as_str);
    let s = if s.starts_with('~') && (s.len() == 1 || s[1..].starts_with('/')) {
        format!("{home}{}", &s[1..])
    } else {
        s.to_string()
    };
    let is_name = |c: char| c.is_ascii_alphanumeric() || c == '_';
    // pass 1: $NAME
    let mut out = String::new();
    let mut rest = s.as_str();
    while let Some(i) = rest.find('$') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        let n = after.find(|c: char| !is_name(c)).unwrap_or(after.len());
        if n == 0 {
            out.push('$');
            rest = after;
            continue;
        }
        match env.get(&after[..n]) {
            Some(v) => out.push_str(v),
            None => out.push_str(&rest[i..i + 1 + n]),
        }
        rest = &after[n..];
    }
    out.push_str(rest);
    // pass 2: ${NAME} / ${NAME:-default}
    let s = out;
    let mut out = String::new();
    let mut rest = s.as_str();
    while let Some(i) = rest.find("${") {
        out.push_str(&rest[..i]);
        let after = &rest[i + 2..];
        let n = after.find(|c: char| !is_name(c)).unwrap_or(after.len());
        let tail = &after[n..];
        let parsed = if n == 0 {
            None
        } else if let Some(t) = tail.strip_prefix('}') {
            Some((None, t))
        } else if let Some(d) = tail.strip_prefix(":-") {
            d.find('}')
                .filter(|&j| j > 0)
                .map(|j| (Some(&d[..j]), &d[j + 1..]))
        } else {
            None
        };
        match parsed {
            Some((default, t)) => {
                let whole = &rest[i..rest.len() - t.len()];
                out.push_str(
                    env.get(&after[..n])
                        .map(String::as_str)
                        .or(default)
                        .unwrap_or(whole),
                );
                rest = t;
            }
            None => {
                out.push_str("${");
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Bundled `getCurrentInsertedDirectory(cwd, searchTerm)`; `backslash`: `\` separates too (pwsh).
pub(crate) fn inserted_directory(
    cwd: &str,
    search_term: &str,
    env: &HashMap<String, String>,
    backslash: bool,
) -> PathBuf {
    let s = shell_expand(search_term, env);
    let dir = &s[..after_sep(&s, backslash)];
    let base = if cwd.ends_with('/') || (backslash && cwd.ends_with('\\')) {
        cwd.to_string()
    } else {
        format!("{cwd}/")
    };
    if dir.is_empty() {
        PathBuf::from(base)
    } else if dir.starts_with('/') || Path::new(dir).is_absolute() {
        PathBuf::from(dir)
    } else {
        PathBuf::from(format!("{base}{dir}"))
    }
}

/// `help` template: subcommand names of the level above the triggering subcommand, minus its aliases.
pub(crate) fn help_items(pr: &ParsedCommandLine) -> Vec<SpecSuggestion> {
    let trig = pr
        .annotations()
        .iter()
        .rev()
        .find(|a| matches!(a.kind(), AnnotationKind::Subcommand | AnnotationKind::Option));
    let n = pr.path().len();
    let (level, exclude): (&Subcommand, &[String]) = match trig {
        Some(a) if a.kind() == AnnotationKind::Subcommand && n >= 2 => {
            (&pr.path()[n - 2], pr.path()[n - 1].names())
        }
        _ => (pr.path().last().unwrap_or(pr.completion()), &[]),
    };
    level
        .subcommands()
        .iter()
        .flat_map(|s| s.names().iter())
        .filter(|n| !exclude.contains(n))
        .map(|n| {
            SpecSuggestion::default()
                .with_names(vec![n.clone()])
                .with_insert_value(Some(n.clone()))
                .with_kind(Some(SuggestionKind::Special))
                .with_template_type(Some("help".into()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::completion::domain::services::insertion_composer::insertion;
    use crate::completion::domain::test_fixtures::{ctx, pr_for, sub};
    use crate::completion::domain::value_objects::suggestion_candidate::SuggestionCandidate;
    use crate::completion::domain::value_objects::suggestion_query::SuggestionQuery;
    use crate::completion::domain::value_objects::token_annotation::TokenAnnotation;
    use crate::shared::domain::value_objects::shell::Shell;
    use std::rc::Rc;

    #[test]
    fn shell_expand_and_directory() {
        let env: HashMap<String, String> = [
            ("HOME".to_string(), "/h".to_string()),
            ("X".into(), "x".into()),
        ]
        .into();
        assert_eq!(shell_expand("~/a", &env), "/h/a");
        assert_eq!(shell_expand("~a", &env), "~a");
        assert_eq!(
            shell_expand("$X/$NOPE/${X}/${NOPE:-d}/${NOPE}", &env),
            "x/$NOPE/x/d/${NOPE}"
        );
        assert_eq!(
            inserted_directory("/w", "src/ma", &env, false),
            PathBuf::from("/w/src/")
        );
        assert_eq!(
            inserted_directory("/w/", "ma", &env, false),
            PathBuf::from("/w/")
        );
        assert_eq!(
            inserted_directory("/w", "/etc/pa", &env, false),
            PathBuf::from("/etc/")
        );
        assert_eq!(
            inserted_directory("/w", "$HOME/d", &env, false),
            PathBuf::from("/h/")
        );
        // windows-backslash-paths (pwsh)
        assert_eq!(
            inserted_directory("C:\\w", "src\\ma", &env, true),
            PathBuf::from("C:\\w/src\\")
        );
        assert_eq!(
            inserted_directory("C:\\w\\", "ma", &env, true),
            PathBuf::from("C:\\w\\")
        );
        assert_eq!(
            inserted_directory("/w", "my\\ d", &env, false),
            PathBuf::from("/w/")
        );
        let q = SuggestionQuery::Path { backslash: true };
        assert_eq!(
            (q.of("src\\ma"), q.of("a/b\\c"), q.of("ma")),
            ("ma".into(), "c".into(), "ma".into())
        );
        assert_eq!(SuggestionQuery::Path { backslash: false }.of("a\\b/c\\d"), "c\\d");
        let main = SuggestionCandidate::new("main.rs", SuggestionKind::File).with_query(q);
        let i = insertion(&main, &ctx("src\\ma", "src\\ma", Shell::Pwsh));
        assert_eq!((i.delete(), i.text()), (0, "in.rs"));
        let i = insertion(&main, &ctx("src\\MA", "src\\MA", Shell::Pwsh));
        assert_eq!((i.delete(), i.text()), (2, "main.rs"));
    }

    #[test]
    fn help_template() {
        let help = sub(&["help", "h"]);
        let root = Rc::new(
            Subcommand::default()
                .with_subcommands(vec![sub(&["add"]), help.clone(), sub(&["commit", "ci"])]),
        );
        let mut pr = pr_for(Subcommand::default(), "");
        pr.completion = help.clone();
        pr.path = vec![root, help];
        pr.annotations = vec![TokenAnnotation::new(AnnotationKind::Subcommand, "help".into(), vec![])];
        let got: Vec<String> = help_items(&ParsedCommandLine::new(pr))
            .into_iter()
            .map(|s| s.names()[0].clone())
            .collect();
        assert_eq!(got, ["add", "commit", "ci"]);
    }
}
