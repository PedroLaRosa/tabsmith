use crate::modules::shared::domain::value_objects::shell::Shell;
use std::collections::HashMap;

/// Alias name → expansion, from the shell's `alias` output (SPEC §6.1.6).
// ponytail: a type alias like SpecFunctionRef; make it a newtype when the table gets behaviour of its own.
pub(crate) type AliasTable = HashMap<String, String>;

/// Parse the `alias` builtin output into a map (SPEC §6.1.6). Fish uses `alias name 'value'` lines.
pub(crate) fn parse_aliases(text: &str, shell: Shell) -> AliasTable {
    let sep = if shell == Shell::Fish { ' ' } else { '=' };
    text.lines()
        .filter_map(|line| {
            let line = line
                .strip_prefix("alias")
                .and_then(|r| r.strip_prefix(char::is_whitespace))
                .unwrap_or(line);
            let (key, value) = line.split_once(sep)?;
            let value = value.strip_prefix('\'').unwrap_or(value);
            let value = value.strip_suffix('\'').unwrap_or(value);
            // Empty values never expand (falsy lookup), so they are not aliases.
            (!key.is_empty() && !value.is_empty()).then(|| (key.to_string(), value.to_string()))
        })
        .collect()
}
