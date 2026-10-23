//! Command-line tokenizer: port of qac `packages/shell-parser` (SPEC §6.1). Pure, no I/O.
//! OWNER: shell-parser agent.
//!
//! Scans bytes: every syntax char is ASCII, so node boundaries always fall on UTF-8 char boundaries.
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
    /// Source text as typed, before alias expansion (what the menu compares between keystrokes).
    pub original_text: String,
}

/// Parse `buffer_before_cursor` and return the command containing the end of it (SPEC §6.1.3), with aliases
/// expanded (SPEC §6.1.6). `None` when there is no command at the cursor (e.g. `ls |`, empty, only spaces).
/// Token offsets are relative to `CommandInfo::text`.
pub fn current_command(
    buffer_before_cursor: &str,
    shell: Shell,
    aliases: &HashMap<String, String>,
) -> Option<CommandInfo> {
    let p = P::new(buffer_before_cursor, shell);
    let tree = p.parse();
    let mut cmd = p.command_info(descendant_at(&tree, buffer_before_cursor.len())?);
    // `expandCommand`: only the first token, only once the user typed past it, each alias once.
    let mut used: Vec<String> = Vec::new();
    while cmd.tokens.len() > 1 {
        let name = cmd.tokens[0].text.clone();
        let Some(value) = aliases.get(&name).filter(|v| !v.is_empty()) else {
            break;
        };
        if used.contains(&name) {
            break;
        }
        let value = value
            .strip_prefix('\'')
            .and_then(|v| v.strip_suffix('\''))
            .unwrap_or(value);
        if let Some(expanded) = substitute_token(&cmd, 0, value, shell) {
            cmd = expanded;
        }
        used.push(name);
    }
    (!cmd.tokens.is_empty()).then_some(cmd)
}

/// Parse the `alias` builtin output into a map (SPEC §6.1.6). Fish uses `alias name 'value'` lines.
pub fn parse_aliases(text: &str, shell: Shell) -> HashMap<String, String> {
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
            // Empty values never expand in Kiro (falsy lookup), so they are not aliases.
            (!key.is_empty() && !value.is_empty()).then(|| (key.to_string(), value.to_string()))
        })
        .collect()
}

/// Re-tokenize `command` with the token at `index` replaced by `replacement` (parserDirectives.alias, SPEC §6.4).
/// `None` unless the result is exactly one plain command (so `|`, `&&`, `;` in `replacement` fail). Tokens that
/// come from `replacement` keep the replaced token's `raw` (what is on screen).
pub fn substitute_token(
    command: &CommandInfo,
    index: usize,
    replacement: &str,
    shell: Shell,
) -> Option<CommandInfo> {
    let tok = command.tokens.get(index)?;
    let text = format!(
        "{}{}{}",
        command.text.get(..tok.start)?,
        replacement,
        command.text.get(tok.end..)?
    );
    let p = P::new(&text, shell);
    let tree = p.parse();
    let [node] = tree.children.as_slice() else {
        return None;
    };
    if node.kind != Kind::Command {
        return None;
    }
    let mut new = p.command_info(node);
    new.original_text = command.original_text.clone();
    let (alias_start, alias_end) = (tok.start, tok.start + replacement.len());
    let (mut diff, mut last_in) = (0, false);
    for (k, t) in new.tokens.iter_mut().enumerate() {
        let in_alias = alias_start < node.start + t.end && node.start + t.start < alias_end;
        if in_alias && last_in {
            diff += 1;
        }
        t.raw = command.tokens.get(k - diff)?.raw.clone();
        last_in = in_alias;
    }
    (new.tokens.len() == command.tokens.len() + diff).then_some(new)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Program,
    AssignmentList,
    Assignment,
    Command,
    Compound,
    Subshell,
    Pipeline,
    List,
    Concatenation,
    Word,
    Str,
    RawStr,
    AnsiC,
    Expansion,
    CmdSubst,
    SimpleExp,
    SpecialExp,
    Arith,
}

/// `BaseNode` without the derived `text`/`innerText` (computed from the source on demand). Offsets are bytes.
/// ponytail: assignment `name` (variable_name/subscript) nodes are not built; nothing reads them.
#[derive(Clone, Debug)]
struct Node {
    kind: Kind,
    start: usize,
    end: usize,
    complete: bool,
    children: Vec<Node>,
}

impl Node {
    fn new(kind: Kind, start: usize, end: usize, complete: bool, children: Vec<Node>) -> Node {
        Node {
            kind,
            start,
            end,
            complete,
            children,
        }
    }
}

/// Deepest `command` node with `start <= index <= end`; children in order, first hit wins (`descendantAtIndex`).
fn descendant_at(node: &Node, index: usize) -> Option<&Node> {
    if node.start > index || index > node.end {
        return None;
    }
    node.children
        .iter()
        .find_map(|c| descendant_at(c, index))
        .or((node.kind == Kind::Command).then_some(node))
}

/// `a op b` → `kind(a, b)`, flattening a right-hand side of the same kind (`reduceStatements`).
fn reduce(lhs: Node, rhs: Node, kind: Kind) -> Node {
    let (start, end, complete) = (lhs.start, rhs.end, lhs.complete && rhs.complete);
    let children = if rhs.kind == kind {
        std::iter::once(lhs).chain(rhs.children).collect()
    } else {
        vec![lhs, rhs]
    };
    Node::new(kind, start, end, complete, children)
}

/// Parser over one source string (SPEC §6.1.1-6.1.2). pwsh: escape is `` ` `` (so no backtick substitution, and
/// `\` is a plain char) and there is no `$'…'` (SPEC §6.1.4).
/// ponytail: pwsh `'it''s'` reads as `its` and a leading `&` call operator ends the command (bash rules);
/// add pwsh-specific quoting if completions inside such commands matter.
struct P<'a> {
    src: &'a str,
    s: &'a [u8],
    esc: u8,
    pwsh: bool,
}

impl<'a> P<'a> {
    fn new(src: &'a str, shell: Shell) -> P<'a> {
        let pwsh = shell == Shell::Pwsh;
        P {
            src,
            s: src.as_bytes(),
            esc: if pwsh { b'`' } else { b'\\' },
            pwsh,
        }
    }

    fn text(&self, n: &Node) -> &'a str {
        &self.src[n.start..n.end]
    }

    /// `getInnerText`: unquote (end delimiter only if complete) and unescape.
    fn inner(&self, n: &Node) -> String {
        if n.kind == Kind::Concatenation {
            return n.children.iter().map(|c| self.inner(c)).collect();
        }
        let (open, close) = match n.kind {
            Kind::Str | Kind::RawStr => (1, 1),
            Kind::AnsiC => (2, 1),
            _ => (0, 0),
        };
        let t = self.text(n).as_bytes();
        let end = t.len() - if n.complete { close } else { 0 };
        let mut out = Vec::with_capacity(t.len());
        let mut i = open;
        while i < end {
            let escape = t[i] == self.esc
                && (n.kind == Kind::Word
                    // `None`: JS `includes("")` is true, so a trailing `\` of an unterminated string is dropped.
                    || n.kind == Kind::Str && matches!(t.get(i + 1), None | Some(b'$' | b'`' | b'"' | b'\\' | b'\n')));
            if escape {
                i += 1;
            }
            out.extend(t.get(i));
            i += 1;
        }
        String::from_utf8_lossy(&out).into_owned()
    }

    fn next_word(&self, i: usize) -> Option<usize> {
        (i..self.s.len()).find(|&j| !self.s[j].is_ascii_whitespace())
    }

    /// `parseOperator`: `; & | &; |& && ||` at `i`.
    fn op(&self, i: usize) -> Option<&'a [u8]> {
        if !matches!(self.s.get(i), Some(b'&' | b';' | b'|')) {
            return None;
        }
        match self.s.get(i..i + 2) {
            Some(two @ (b"&;" | b"|&" | b"&&" | b"||")) => Some(two),
            _ => Some(&self.s[i..i + 1]),
        }
    }

    /// Literal dispatch by lookahead (`childAtIndex`). `terms` end a `$name`.
    fn child_at(&self, i: usize, in_string: bool, terms: &[u8]) -> Option<Node> {
        let s = self.s;
        match s[i] {
            b'$' => match (s.get(i + 1), s.get(i + 2)) {
                (Some(b'('), Some(b'(')) => Some(self.literal(Kind::Arith, i, 3, b"))")),
                (Some(b'('), _) => Some(self.cmd_subst(i)),
                (Some(b'{'), _) => Some(self.literal(Kind::Expansion, i, 2, b"}")),
                (Some(b'\''), _) if !in_string && !self.pwsh => {
                    Some(self.literal(Kind::AnsiC, i, 2, b"'"))
                }
                _ => self.simple_expansion(i, terms),
            },
            b'`' if !self.pwsh => Some(self.cmd_subst(i)),
            b'\'' if !in_string => Some(self.literal(Kind::RawStr, i, 1, b"'")),
            b'"' if !in_string => Some(self.literal(Kind::Str, i, 1, b"\"")),
            _ => None,
        }
    }

    /// `$name` / `$$`-style specials; a lone `$` (not at EOF) is `None` so it stays part of a word.
    fn simple_expansion(&self, i: usize, terms: &[u8]) -> Option<Node> {
        let s = self.s;
        if s.get(i + 1).is_some_and(|c| b"*@?-$0_".contains(c)) {
            return Some(Node::new(Kind::SpecialExp, i, i + 2, true, vec![]));
        }
        match (i + 1..s.len()).find(|&j| b"\t \n$\\".contains(&s[j]) || terms.contains(&s[j])) {
            Some(j) if j == i + 1 => None,
            end => Some(Node::new(
                Kind::SimpleExp,
                i,
                end.unwrap_or(s.len()),
                true,
                vec![],
            )),
        }
    }

    /// `$(…)` or `` `…` ``: nested statements until the closing char.
    fn cmd_subst(&self, i: usize) -> Node {
        let (from, close) = if self.s[i] == b'`' {
            (i + 1, b'`')
        } else {
            (i + 2, b')')
        };
        let (children, term) = self.statements(from, Some(close), false);
        let end = term.map_or(self.s.len(), |t| t + 1);
        Node::new(
            Kind::CmdSubst,
            i,
            end,
            term.is_some() && !children.is_empty(),
            children,
        )
    }

    /// `parseLiteral`: quoted strings and `${…}`/`$((…))`; unterminated → `complete=false`, extends to EOF.
    fn literal(&self, kind: Kind, i: usize, open: usize, close: &[u8]) -> Node {
        let s = self.s;
        let nested = matches!(kind, Kind::Str | Kind::Expansion);
        let mut children = Vec::new();
        let mut j = i + open;
        while j < s.len() {
            if let Some(c) = nested
                .then(|| self.child_at(j, kind == Kind::Str, close))
                .flatten()
            {
                j = c.end;
                children.push(c);
            } else if s[j] == self.esc && kind != Kind::RawStr {
                j += 2;
            } else if s[j..].starts_with(close) {
                return Node::new(kind, i, j + close.len(), true, children);
            } else {
                j += 1;
            }
        }
        Node::new(kind, i, s.len(), false, children)
    }

    /// `parseConcatenationOrLiteralNode`: arguments until an operator or `terminal`. Adjacent literals form one
    /// `concatenation`; space/tab ends an argument.
    fn words(&self, start: usize, terminal: Option<u8>) -> (Vec<Node>, usize) {
        fn end_word(arg: &mut Vec<Node>, word: &mut Option<usize>, end: usize) {
            if let Some(ws) = word.take() {
                arg.push(Node::new(Kind::Word, ws, end, true, vec![]));
            }
        }
        fn end_argument(
            out: &mut Vec<Node>,
            arg: &mut Vec<Node>,
            word: &mut Option<usize>,
            end: usize,
        ) {
            end_word(arg, word, end);
            if arg.len() > 1 {
                let last = &arg[arg.len() - 1];
                let (start, end, complete) = (arg[0].start, last.end, last.complete);
                out.push(Node::new(
                    Kind::Concatenation,
                    start,
                    end,
                    complete,
                    std::mem::take(arg),
                ));
            } else {
                out.append(arg);
            }
        }
        let s = self.s;
        let mut terms = b"&|;\n'\"`".to_vec();
        terms.extend(terminal);
        let (mut out, mut arg, mut word) = (Vec::new(), Vec::new(), None);
        let mut i = start;
        while i < s.len() {
            if self.op(i).is_some() || Some(s[i]) == terminal {
                break;
            }
            if let Some(c) = self.child_at(i, false, &terms) {
                end_word(&mut arg, &mut word, i);
                i = c.end;
                arg.push(c);
                continue;
            }
            if s[i] == b' ' || s[i] == b'\t' {
                end_argument(&mut out, &mut arg, &mut word, i);
            } else {
                // Fix: qac starts the word after the escape char, so `\ x` had raw ` x` (SPEC D22 needs raw).
                word.get_or_insert(i);
                if s[i] == self.esc {
                    i += 1;
                }
            }
            i += 1;
        }
        let i = i.min(s.len()); // a trailing escape char skips past EOF
        end_argument(&mut out, &mut arg, &mut word, i);
        (out, i)
    }

    /// A command without arguments extends to EOF and is incomplete.
    fn command(&self, idx: usize, terminal: Option<u8>) -> Node {
        let start = self.next_word(idx).unwrap_or(idx);
        let (children, end) = self.words(start, terminal);
        let has = !children.is_empty();
        Node::new(
            Kind::Command,
            start,
            if has { end } else { self.s.len() },
            has,
            children,
        )
    }

    /// `=` index if `^[\w\[\]]+\+?=` matches at `i`.
    fn assignment_eq(&self, i: usize) -> Option<usize> {
        let s = self.s;
        let n = s[i..]
            .iter()
            .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'[' | b']'))
            .count();
        let j = i + n + usize::from(s.get(i + n) == Some(&b'+'));
        (n > 0 && s.get(j) == Some(&b'=')).then_some(j)
    }

    fn assignments(&self, idx: usize) -> Vec<Node> {
        let mut out = Vec::new();
        let mut last = idx;
        while last < self.s.len() {
            let Some(start) = self.next_word(last) else {
                break;
            };
            let Some(eq) = self.assignment_eq(start) else {
                break;
            };
            let (children, end) = self.words(eq + 1, Some(b' '));
            // SPEC D23 fix: an empty value (`FOO= cmd`) is a complete empty assignment (Kiro throws).
            let complete = children.last().is_none_or(|c| c.complete);
            out.push(Node::new(Kind::Assignment, start, end, complete, children));
            last = end;
        }
        out
    }

    /// `FOO=1 BAR=2 cmd …` → assignment_list (with the command if one follows), else a plain command.
    fn assignment_list_or_command(&self, start: usize, terminal: Option<u8>) -> Node {
        let mut children = self.assignments(start);
        let Some(last) = children.last() else {
            return self.command(start, terminal);
        };
        let (mut end, complete) = (last.end, last.complete);
        let op = self.next_word(end).and_then(|j| self.op(j));
        if op.is_none() && complete && end != self.s.len() {
            let cmd = self.command(end, terminal);
            end = cmd.end;
            children.push(cmd);
        }
        Node::new(Kind::AssignmentList, start, end, true, children)
    }

    /// `{ … }`, `( … )` or a command, then `&&`/`||` (flattened `list`) and `|`/`|&` (`pipeline`, binds tighter).
    fn statement(&self, idx: usize, terminal: Option<u8>) -> Node {
        let s = self.s;
        let i = self.next_word(idx).unwrap_or(idx);
        let lhs = match s.get(i) {
            Some(&c @ (b'{' | b'(')) => {
                let compound = c == b'{';
                let (children, term) =
                    self.statements(i + 1, Some(if compound { b'}' } else { b')' }), compound);
                let end = match term {
                    Some(t) => t + 1,
                    None => children.last().map_or(s.len(), |c| c.end),
                };
                let kind = if compound {
                    Kind::Compound
                } else {
                    Kind::Subshell
                };
                Node::new(
                    kind,
                    i,
                    end,
                    term.is_some() && !children.is_empty(),
                    children,
                )
            }
            _ => self.assignment_list_or_command(i, terminal),
        };
        let Some(op_i) = self.next_word(lhs.end) else {
            return lhs;
        };
        let op = match self.op(op_i) {
            Some(op) if !matches!(op, b";" | b"&" | b"&;") => op,
            _ => return lhs,
        };
        let rhs = self.statement(op_i + op.len(), terminal);
        if op == b"&&" || op == b"||" {
            return reduce(lhs, rhs, Kind::List);
        }
        if rhs.kind != Kind::List {
            return reduce(lhs, rhs, Kind::Pipeline);
        }
        let (end, rhs_complete, mut rest) = (rhs.end, rhs.complete, rhs.children);
        let first = reduce(lhs, rest.remove(0), Kind::Pipeline);
        let (start, complete) = (first.start, first.complete && rhs_complete);
        rest.insert(0, first);
        Node::new(Kind::List, start, end, complete, rest)
    }

    /// Statements separated by `; & &;` until EOF or `terminal` (returned as its index). `must_terminate` (`{`):
    /// `terminal` only counts right after a separator.
    fn statements(
        &self,
        idx: usize,
        terminal: Option<u8>,
        must_terminate: bool,
    ) -> (Vec<Node>, Option<usize>) {
        let s = self.s;
        let mut out = Vec::new();
        let mut i = idx;
        while i < s.len() {
            let mut st = self.statement(i, if must_terminate { None } else { terminal });
            let Some(op_i) = self.next_word(st.end) else {
                out.push(st);
                break;
            };
            if !must_terminate && Some(s[op_i]) == terminal {
                out.push(st);
                return (out, Some(op_i));
            }
            if let Some(op) = self.op(op_i) {
                i = op_i + op.len();
                out.push(st);
                if let Some(n) = self.next_word(i).filter(|&n| Some(s[n]) == terminal) {
                    return (out, Some(n));
                }
            } else {
                // Missing terminator but tokens left (`(a) b`); assignments need none.
                if st.kind != Kind::AssignmentList {
                    st.complete = false;
                }
                out.push(st);
                i = op_i;
            }
        }
        (out, None)
    }

    fn parse(&self) -> Node {
        Node::new(
            Kind::Program,
            0,
            self.s.len(),
            true,
            self.statements(0, None, false).0,
        )
    }

    /// `convertCommandNodeToCommand`, offsets relative to the command text.
    fn command_info(&self, cmd: &Node) -> CommandInfo {
        let text = self.text(cmd).to_string();
        let mut tokens: Vec<Token> = cmd
            .children
            .iter()
            .map(|c| Token {
                text: self.inner(c),
                raw: self.text(c).to_string(),
                start: c.start - cmd.start,
                end: c.end - cmd.start,
            })
            .collect();
        // Trailing empty token ("typed a space, completing a new argument"). The `1` is absolute, as in qac.
        if cmd.children.last().map_or(1, |c| c.end) < cmd.end && text.ends_with(' ') {
            tokens.push(Token {
                text: String::new(),
                raw: String::new(),
                start: text.len(),
                end: text.len(),
            });
        }
        CommandInfo {
            tokens,
            original_text: text.clone(),
            text,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(buf: &str, shell: Shell, aliases: &[(&str, &str)]) -> Option<CommandInfo> {
        let map = aliases
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        current_command(buf, shell, &map)
    }

    fn toks_in(buf: &str, shell: Shell, aliases: &[(&str, &str)]) -> Vec<String> {
        cmd(buf, shell, aliases)
            .map(|c| c.tokens.into_iter().map(|t| t.text).collect())
            .unwrap_or_default()
    }

    fn toks(buf: &str) -> Vec<String> {
        toks_in(buf, Shell::Bash, &[])
    }

    fn tree(src: &str) -> Node {
        P::new(src, Shell::Bash).parse()
    }

    #[test]
    fn spec_table() {
        // SPEC §6.1.3 worked examples.
        let cases: &[(&str, &[&str])] = &[
            ("git co ", &["git", "co", ""]),
            ("ls | gr", &["gr"]),
            ("ls | ", &[""]),
            ("ls |", &[]),
            ("echo $(git ch", &["git", "ch"]),
            ("cd my\\ di", &["cd", "my di"]),
            ("git commit -m \"fix bu", &["git", "commit", "-m", "fix bu"]),
            ("FOO=1 BAR=2 npm ", &["npm", ""]),
            ("(cd x && git ", &["git", ""]),
            ("FOO= git ", &["git", ""]),
            (" ", &[]),
        ];
        for (buf, want) in cases {
            assert_eq!(toks(buf), *want, "buffer {buf:?}");
        }
        assert_eq!(cmd("ls |", Shell::Bash, &[]), None);
        assert_eq!(cmd("", Shell::Bash, &[]), None);
        assert_eq!(cmd(" ", Shell::Bash, &[]), None);
        assert_eq!(toks("  "), [""]); // qac quirk: two spaces give one empty token
        assert_eq!(toks("git"), ["git"]);
    }

    #[test]
    fn operators_without_spaces() {
        for op in ["&&", "||", "|", "|&", ";", "&", "&;"] {
            assert_eq!(toks(&format!("a{op}b")), ["b"], "op {op}");
            assert_eq!(toks(&format!("a x{op}b c ")), ["b", "c", ""], "op {op}");
        }
        assert_eq!(toks("a&&"), Vec::<String>::new());
        assert_eq!(toks("a;b;"), Vec::<String>::new());
    }

    #[test]
    fn assignments() {
        assert_eq!(cmd("FOO=bar", Shell::Bash, &[]), None);
        assert_eq!(cmd("FOO=", Shell::Bash, &[]), None);
        assert_eq!(toks("FOO=bar "), [""]);
        assert_eq!(toks("FOO=\"a b\" git "), ["git", ""]);
        assert_eq!(toks("A[0]=x B+=y make t"), ["make", "t"]);
        assert_eq!(toks("FOO=bar && git "), ["git", ""]);
        // Not an assignment past the first word.
        assert_eq!(toks("git --foo=bar "), ["git", "--foo=bar", ""]);
        // The fix: an empty value is a complete assignment.
        let t = tree("FOO= git");
        assert_eq!(t.children[0].kind, Kind::AssignmentList);
        assert!(t.children[0].children[0].complete);
        assert_eq!(t.children[0].children[1].kind, Kind::Command);
    }

    #[test]
    fn unterminated_quotes() {
        let p = P::new("a \"b c", Shell::Bash);
        let t = p.parse();
        let s = &t.children[0].children[1];
        assert_eq!(
            (s.kind, s.complete, p.inner(s).as_str()),
            (Kind::Str, false, "b c")
        );
        let p = P::new("a 'b c", Shell::Bash);
        let t = p.parse();
        let s = &t.children[0].children[1];
        assert_eq!(
            (s.kind, s.complete, p.inner(s).as_str()),
            (Kind::RawStr, false, "b c")
        );
        let p = P::new("a $'b c", Shell::Bash);
        let t = p.parse();
        let s = &t.children[0].children[1];
        assert_eq!(
            (s.kind, s.complete, p.inner(s).as_str()),
            (Kind::AnsiC, false, "b c")
        );
        let c = cmd("cat 'my fi", Shell::Bash, &[]).unwrap();
        assert_eq!(
            (c.tokens[1].text.as_str(), c.tokens[1].raw.as_str()),
            ("my fi", "'my fi")
        );
        let c = cmd("cat \"a b\"", Shell::Bash, &[]).unwrap();
        assert_eq!(
            (c.tokens[1].text.as_str(), c.tokens[1].raw.as_str()),
            ("a b", "\"a b\"")
        );
        // An escaped quote does not terminate a string; a trailing `\` in an open string is dropped.
        assert_eq!(toks("echo \"a\\\" b"), ["echo", "a\" b"]);
        assert_eq!(toks("echo \"a b\\"), ["echo", "a b"]);
    }

    #[test]
    fn escapes() {
        // Word: every `\x` is `x`; a lone trailing `\` is dropped.
        assert_eq!(toks("echo a\\ b\\$c\\\\d\\"), ["echo", "a b$c\\d"]);
        // String: only `\$ \` \" \\ \newline` drop the backslash.
        assert_eq!(
            toks("echo \"a\\$b\\`c\\\"d\\\\e\\f\""),
            ["echo", "a$b`c\"d\\e\\f"]
        );
        // Raw and ANSI-C strings are verbatim.
        assert_eq!(toks("echo 'a\\b' $'c\\'d'"), ["echo", "a\\b", "c\\'d"]);
        // A word that starts with an escape keeps it in `raw` (fix, SPEC D22).
        let c = cmd("cd \\ fo", Shell::Bash, &[]).unwrap();
        assert_eq!(
            (c.tokens[1].text.as_str(), c.tokens[1].raw.as_str()),
            (" fo", "\\ fo")
        );
        let c = cmd("cd my\\ di", Shell::Bash, &[]).unwrap();
        assert_eq!(c.tokens[1].raw, "my\\ di");
    }

    #[test]
    fn concatenation() {
        let c = cmd("cmd --foo=\"a b\"c", Shell::Bash, &[]).unwrap();
        assert_eq!(c.tokens.len(), 2);
        assert_eq!(
            (c.tokens[1].text.as_str(), c.tokens[1].raw.as_str()),
            ("--foo=a bc", "--foo=\"a b\"c")
        );
        assert_eq!(toks("a $'b''c'd$$$e${f}\"g\""), ["a", "bcd$$$e${f}g"]);
        let t = tree("a b\"c");
        let arg = &t.children[0].children[1];
        assert_eq!((arg.kind, arg.complete), (Kind::Concatenation, false));
    }

    #[test]
    fn substitutions_and_nesting() {
        assert_eq!(toks("echo $(git $(ls -"), ["ls", "-"]);
        assert_eq!(toks("echo `git ch"), ["git", "ch"]);
        assert_eq!(toks("echo \"$(git ch"), ["git", "ch"]);
        assert_eq!(toks("x=$(git ch"), ["git", "ch"]);
        assert_eq!(
            toks("echo $(git log) foo "),
            ["echo", "$(git log)", "foo", ""]
        );
        assert_eq!(toks("echo $((1+2)) "), ["echo", "$((1+2))", ""]);
        assert_eq!(toks("{ a; git "), ["git", ""]);
        assert_eq!(toks("(a) | b && git c"), ["git", "c"]);
        assert_eq!(toks("a | b | c "), ["c", ""]);
        assert_eq!(toks("echo $"), ["echo", "$"]);
        assert_eq!(toks("echo a$ b"), ["echo", "a$", "b"]);
    }

    #[test]
    fn statement_structure() {
        let kinds =
            |src: &str| -> Vec<Kind> { tree(src).children.iter().map(|c| c.kind).collect() };
        assert_eq!(kinds("a | b && c"), [Kind::List]);
        assert_eq!(
            tree("a | b && c").children[0].children[0].kind,
            Kind::Pipeline
        );
        assert_eq!(
            tree("a && b | c").children[0].children[1].kind,
            Kind::Pipeline
        );
        assert_eq!(tree("a && b || c").children[0].children.len(), 3);
        assert_eq!(kinds("a; b"), [Kind::Command, Kind::Command]);
        assert_eq!(kinds("{ a; b; }"), [Kind::Compound]);
        assert!(tree("{ a; b; }").children[0].complete);
        assert!(!tree("{a; b}").children[0].complete); // `b}` is a word (bash semantics)
        assert!(!tree("(a) b").children[0].complete);
        // Cursor inside an earlier command.
        let t = tree("git co && ls -l");
        assert_eq!(descendant_at(&t, 3).map(|n| (n.start, n.end)), Some((0, 7)));
        assert_eq!(
            descendant_at(&t, 15).map(|n| (n.start, n.end)),
            Some((10, 15))
        );
    }

    #[test]
    fn token_offsets() {
        let c = cmd("FOO=1  git  co ", Shell::Bash, &[]).unwrap();
        assert_eq!(c.text, "git  co ");
        for t in &c.tokens {
            assert_eq!(&c.text[t.start..t.end], t.raw);
        }
        assert_eq!((c.tokens[2].start, c.tokens[2].end), (8, 8));
    }

    #[test]
    fn multibyte_and_no_panics() {
        assert_eq!(
            toks("echo héllo\\ wörld 日本 "),
            ["echo", "héllo wörld", "日本", ""]
        );
        assert_eq!(toks("cd \\é"), ["cd", "é"]);
        let nasty = [
            "a \"é$(b `c 'd\\é ${e} $((f)) $'g\\é' | h && i; (j) {k; l;} M=é\\ n o\\",
            "FOO=é BAR+=\"x\" A[1]=$(y) cmd --o=\"日\"本 -- `z` & w &; v |& u || t",
            "$ $é ${ $(( $( ` ' \" \\ ) } ;; && || | & ",
        ];
        for src in nasty {
            for shell in Shell::ALL {
                for (i, _) in src.char_indices().chain([(src.len(), ' ')]) {
                    let _ = cmd(&src[..i], shell, &[("a", "b c"), ("M", "x | y")]);
                }
            }
        }
    }

    #[test]
    fn aliases_qac() {
        // qac tests/command.test.ts.
        let aliases = [("woman", "man"), ("quote", "'q'"), ("g", "git")];
        assert_eq!(toks_in("git co ", Shell::Bash, &aliases), ["git", "co", ""]);
        assert_eq!(toks_in("woman ", Shell::Bash, &[]), ["woman", ""]);
        assert_eq!(
            toks_in("another string ", Shell::Bash, &aliases),
            ["another", "string", ""]
        );
        assert_eq!(toks_in("woman", Shell::Bash, &aliases), ["woman"]);
        assert_eq!(toks_in("woman ", Shell::Bash, &aliases), ["man", ""]);
        assert_eq!(
            toks_in("man woman ", Shell::Bash, &aliases),
            ["man", "woman", ""]
        );
        assert_eq!(toks_in("quote ", Shell::Bash, &aliases), ["q", ""]);
    }

    #[test]
    fn aliases_expansion() {
        let aliases = [
            ("g", "git"),
            ("ll", "ls -l"),
            ("ls", "ls -G"),
            ("lg", "ls | grep"),
            ("x", "a && b"),
            ("y", "a; b"),
        ];
        let c = cmd("g co ", Shell::Bash, &aliases).unwrap();
        assert_eq!(c.text, "git co ");
        assert_eq!(c.original_text, "g co "); // pre-expansion text for the menu (alias-command-text-hides-menu)
        let raws: Vec<_> = c.tokens.iter().map(|t| t.raw.as_str()).collect();
        assert_eq!(raws, ["g", "co", ""]);
        let ll = cmd("ll ", Shell::Bash, &aliases).unwrap();
        assert_eq!(
            (ll.text.as_str(), ll.original_text.as_str()),
            ("ls -G -l ", "ll ")
        );
        assert_eq!(
            cmd("echo | g ch", Shell::Bash, &aliases)
                .unwrap()
                .original_text,
            "g ch"
        );
        // Multi-word alias: every alias token keeps the alias' raw; later tokens map 1:1.
        let c = cmd("ll foo ", Shell::Bash, &aliases).unwrap();
        let pairs: Vec<_> = c
            .tokens
            .iter()
            .map(|t| (t.text.as_str(), t.raw.as_str()))
            .collect();
        // ll → ls -l → ls -G -l
        assert_eq!(
            pairs,
            [
                ("ls", "ll"),
                ("-G", "ll"),
                ("-l", "ll"),
                ("foo", "foo"),
                ("", "")
            ]
        );
        // Self-reference expands once.
        assert_eq!(toks_in("ls x", Shell::Bash, &aliases), ["ls", "-G", "x"]);
        // Aliases with `|`, `&&`, `;` are not expanded.
        assert_eq!(toks_in("lg x", Shell::Bash, &aliases), ["lg", "x"]);
        assert_eq!(toks_in("x y", Shell::Bash, &aliases), ["x", "y"]);
        assert_eq!(toks_in("y z", Shell::Bash, &aliases), ["y", "z"]);
        // Inside a pipeline / substitution.
        assert_eq!(toks_in("echo | g ch", Shell::Bash, &aliases), ["git", "ch"]);
    }

    #[test]
    fn parse_aliases_formats() {
        let bash = parse_aliases(
            "alias g='git'\nalias ll='ls -l'\nalias e='a=b'\nalias bad\n\n",
            Shell::Bash,
        );
        let want: HashMap<String, String> = [("g", "git"), ("ll", "ls -l"), ("e", "a=b")]
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        assert_eq!(bash, want);
        let zsh = parse_aliases("g=git\r\nll='ls -l'\ne='a=b'\n", Shell::Zsh);
        assert_eq!(zsh, want);
        let fish = parse_aliases(
            "alias g git\nalias ll 'ls -l'\nalias e 'a=b'\n",
            Shell::Fish,
        );
        assert_eq!(fish, want);
        let parsed = parse_aliases("alias woman=man\n", Shell::Bash);
        assert_eq!(
            toks_in(
                "woman ",
                Shell::Bash,
                &[("woman", parsed["woman"].as_str())]
            ),
            ["man", ""]
        );
    }

    #[test]
    fn substitute() {
        let c = cmd("git co -b ", Shell::Bash, &[]).unwrap();
        let s = substitute_token(&c, 1, "checkout", Shell::Bash).unwrap();
        assert_eq!(s.text, "git checkout -b ");
        let pairs: Vec<_> = s
            .tokens
            .iter()
            .map(|t| (t.text.as_str(), t.raw.as_str()))
            .collect();
        assert_eq!(
            pairs,
            [("git", "git"), ("checkout", "co"), ("-b", "-b"), ("", "")]
        );
        let s = substitute_token(&c, 1, "commit -m", Shell::Bash).unwrap();
        let texts: Vec<_> = s.tokens.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, ["git", "commit", "-m", "-b", ""]);
        assert_eq!(s.tokens[2].raw, "co");
        assert_eq!(substitute_token(&c, 1, "log | less", Shell::Bash), None);
        assert_eq!(substitute_token(&c, 1, "", Shell::Bash), None);
        assert_eq!(substitute_token(&c, 9, "x", Shell::Bash), None);
    }

    #[test]
    fn pwsh() {
        let ps = |b: &str| toks_in(b, Shell::Pwsh, &[]);
        assert_eq!(ps("cd my` di"), ["cd", "my di"]);
        assert_eq!(ps("cd C:\\Users\\me\\Doc"), ["cd", "C:\\Users\\me\\Doc"]);
        assert_eq!(ps("echo \"a`\"b\" "), ["echo", "a\"b", ""]);
        assert_eq!(ps("echo \"a\\\" b"), ["echo", "a\\", "b"]); // `\` does not escape in pwsh
        assert_eq!(ps("echo `ls x"), ["echo", "ls", "x"]); // no backtick substitution
        assert_eq!(ps("echo $'a b"), ["echo", "$a b"]); // no ANSI-C string: `$` + raw string
        assert_eq!(ps("Get-Item $(git ch"), ["git", "ch"]);
        assert_eq!(ps("a | b; git co "), ["git", "co", ""]);
        let c = cmd("cd my` di", Shell::Pwsh, &[]).unwrap();
        assert_eq!(c.tokens[1].raw, "my` di");
    }

    /// qac `tests/fixtures/*` (MIT): parse trees as JSON, compared on type, offsets, complete and innerText.
    #[test]
    fn qac_fixtures() {
        use serde_json::{json, Value};
        fn name(k: Kind) -> &'static str {
            match k {
                Kind::Program => "program",
                Kind::AssignmentList => "assignment_list",
                Kind::Assignment => "assignment",
                Kind::Command => "command",
                Kind::Compound => "compound_statement",
                Kind::Subshell => "subshell",
                Kind::Pipeline => "pipeline",
                Kind::List => "list",
                Kind::Concatenation => "concatenation",
                Kind::Word => "word",
                Kind::Str => "string",
                Kind::RawStr => "raw_string",
                Kind::AnsiC => "ansi_c_string",
                Kind::Expansion => "expansion",
                Kind::CmdSubst => "command_substitution",
                Kind::SimpleExp => "simple_expansion",
                Kind::SpecialExp => "special_expansion",
                Kind::Arith => "arithmetic_expansion",
            }
        }
        fn mine(p: &P, n: &Node) -> Value {
            let children: Vec<Value> = n.children.iter().map(|c| mine(p, c)).collect();
            // D22 fix: a word starting with an escape starts at it; qac starts right after it.
            let start =
                n.start + usize::from(n.kind == Kind::Word && p.s.get(n.start) == Some(&b'\\'));
            json!({"type": name(n.kind), "startIndex": start, "endIndex": n.end, "complete": n.complete,
                   "innerText": p.inner(n), "children": children})
        }
        fn project(v: &Value) -> Value {
            let children: Vec<Value> = v["children"]
                .as_array()
                .unwrap()
                .iter()
                .map(project)
                .collect();
            json!({"type": v["type"], "startIndex": v["startIndex"], "endIndex": v["endIndex"],
                   "complete": v["complete"], "innerText": v["innerText"], "children": children})
        }
        fn cases(text: &str) -> Vec<(String, String)> {
            text.split("\n\n")
                .map(|c| {
                    let (title, body) = c.split_once('\n').unwrap();
                    (
                        title.trim_start_matches(['#', '/']).trim().to_string(),
                        body.trim().to_string(),
                    )
                })
                .collect()
        }
        let fixtures = [
            (
                "basic",
                include_str!("../tests/fixtures/shell_parser/basic/input.sh"),
                include_str!("../tests/fixtures/shell_parser/basic/output.txt"),
            ),
            (
                "multipleStatements",
                include_str!("../tests/fixtures/shell_parser/multipleStatements/input.sh"),
                include_str!("../tests/fixtures/shell_parser/multipleStatements/output.txt"),
            ),
            (
                "primaryExpressions",
                include_str!("../tests/fixtures/shell_parser/primaryExpressions/input.sh"),
                include_str!("../tests/fixtures/shell_parser/primaryExpressions/output.txt"),
            ),
            (
                "variables",
                include_str!("../tests/fixtures/shell_parser/variables/input.sh"),
                include_str!("../tests/fixtures/shell_parser/variables/output.txt"),
            ),
        ];
        let mut failures = Vec::new();
        let mut count = 0;
        for (fixture, input, output) in fixtures {
            let expected: HashMap<String, String> = cases(output).into_iter().collect();
            for (case, src) in cases(input) {
                count += 1;
                let p = P::new(&src, Shell::Bash);
                let want = project(&serde_json::from_str(&expected[&case]).unwrap());
                let got = mine(&p, &p.parse());
                if got != want {
                    failures.push(format!(
                        "{fixture}/{case} {src:?}\n got: {got}\nwant: {want}"
                    ));
                }
            }
        }
        assert!(count > 60, "only {count} fixture cases");
        assert!(failures.is_empty(), "{}", failures.join("\n\n"));
    }
}
