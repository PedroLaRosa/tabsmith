//! Command-line tokenizer (SPEC §6.1). Pure, no I/O.
//!
//! Scans bytes: every syntax char is ASCII, so node boundaries always fall on UTF-8 char boundaries.
use crate::modules::completion::domain::value_objects::alias_table::AliasTable;
use crate::modules::completion::domain::value_objects::command_line::CommandLine;
use crate::modules::completion::domain::value_objects::token::Token;
use crate::modules::shared::domain::value_objects::shell::Shell;

/// Parse `buffer_before_cursor` and return the command containing the end of it (SPEC §6.1.3), with aliases
/// expanded (SPEC §6.1.6). `None` when there is no command at the cursor (e.g. `ls |`, empty, only spaces).
/// Token offsets are relative to `CommandLine::text`.
pub(crate) fn current_command(
    buffer_before_cursor: &str,
    shell: Shell,
    aliases: &AliasTable,
) -> Option<CommandLine> {
    let p = P::new(buffer_before_cursor, shell);
    let tree = p.parse();
    let mut cmd = p.command_info(descendant_at(&tree, buffer_before_cursor.len())?);
    // `expandCommand`: only the first token, only once the user typed past it, each alias once.
    let mut used: Vec<String> = Vec::new();
    while cmd.tokens().len() > 1 {
        let name = cmd.tokens()[0].text().to_string();
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
    (!cmd.tokens().is_empty()).then_some(cmd)
}

/// Re-tokenize `command` with the token at `index` replaced by `replacement` (parserDirectives.alias, SPEC §6.4).
/// `None` unless the result is exactly one plain command (so `|`, `&&`, `;` in `replacement` fail). Tokens that
/// come from `replacement` keep the replaced token's `raw` (what is on screen).
pub(crate) fn substitute_token(
    command: &CommandLine,
    index: usize,
    replacement: &str,
    shell: Shell,
) -> Option<CommandLine> {
    let tok = command.tokens().get(index)?;
    let text = format!(
        "{}{}{}",
        command.text().get(..tok.start())?,
        replacement,
        command.text().get(tok.end()..)?
    );
    let p = P::new(&text, shell);
    let tree = p.parse();
    let [node] = tree.children.as_slice() else {
        return None;
    };
    if node.kind != Kind::Command {
        return None;
    }
    let new = p.command_info(node);
    let (alias_start, alias_end) = (tok.start(), tok.start() + replacement.len());
    let (mut diff, mut last_in) = (0, false);
    let mut tokens = Vec::with_capacity(new.tokens().len());
    for (k, t) in new.tokens().iter().enumerate() {
        let in_alias = alias_start < node.start + t.end() && node.start + t.start() < alias_end;
        if in_alias && last_in {
            diff += 1;
        }
        tokens.push(t.clone().with_raw(command.tokens().get(k - diff)?.raw().to_string()));
        last_in = in_alias;
    }
    (tokens.len() == command.tokens().len() + diff)
        .then(|| CommandLine::new(tokens, new.text().to_string(), command.original_text().to_string()))
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
            Kind::Program
            | Kind::AssignmentList
            | Kind::Assignment
            | Kind::Command
            | Kind::Compound
            | Kind::Subshell
            | Kind::Pipeline
            | Kind::List
            | Kind::Concatenation
            | Kind::Word
            | Kind::Expansion
            | Kind::CmdSubst
            | Kind::SimpleExp
            | Kind::SpecialExp
            | Kind::Arith => (0, 0),
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
                // The word starts at the escape char, so `\ x` has raw `\ x` (SPEC D22 needs raw).
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
            // SPEC D23: an empty value (`FOO= cmd`) is a complete empty assignment.
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
    fn command_info(&self, cmd: &Node) -> CommandLine {
        let text = self.text(cmd).to_string();
        let mut tokens: Vec<Token> = cmd
            .children
            .iter()
            .map(|c| Token::new(self.inner(c), self.text(c).to_string(), c.start - cmd.start, c.end - cmd.start))
            .collect();
        // Trailing empty token ("typed a space, completing a new argument"). The `1` is an absolute offset.
        if cmd.children.last().map_or(1, |c| c.end) < cmd.end && text.ends_with(' ') {
            tokens.push(Token::new(String::new(), String::new(), text.len(), text.len()));
        }
        CommandLine::new(tokens, text.clone(), text)
    }
}

#[cfg(test)]
mod tests;
