//! Command-line tokenizer: port of qac `packages/shell-parser` (SPEC §6.1). Pure, no I/O.
//! OWNER: shell-parser agent.
use crate::types::Shell;
use std::collections::HashMap;

/// One argument of the current command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    /// Unquoted/unescaped text (`inner_text`).
    pub text: String,
    /// Raw source text as typed (quotes/escapes included).
    pub raw: String,
    /// Byte offsets into the (alias-expanded) command source.
    pub start: usize,
    pub end: usize,
}

/// The command under the cursor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandInfo {
    /// Tokens up to the cursor; a trailing empty token when the text ends with whitespace.
    pub tokens: Vec<Token>,
    /// Source text of the command (after alias expansion).
    pub text: String,
}

/// Parse `buffer_before_cursor` and return the command containing the end of it (SPEC §6.1.3), with aliases
/// expanded (SPEC §6.1.6). `None` when there is no command at the cursor (e.g. `ls |`, empty, only spaces).
pub fn current_command(buffer_before_cursor: &str, shell: Shell, aliases: &HashMap<String, String>) -> Option<CommandInfo> {
    let _ = (buffer_before_cursor, shell, aliases);
    todo!("shell-parser agent")
}

/// Parse the `alias` builtin output into a map (SPEC §6.1.6). Fish uses `alias name 'value'` lines.
pub fn parse_aliases(text: &str, shell: Shell) -> HashMap<String, String> {
    let _ = (text, shell);
    todo!("shell-parser agent")
}

/// Re-tokenize `command` with the token at `index` replaced by `replacement` (parserDirectives.alias, SPEC §6.4).
pub fn substitute_token(command: &CommandInfo, index: usize, replacement: &str, shell: Shell) -> Option<CommandInfo> {
    let _ = (command, index, replacement, shell);
    todo!("shell-parser agent")
}
