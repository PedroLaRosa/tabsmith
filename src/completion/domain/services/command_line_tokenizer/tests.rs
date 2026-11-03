use super::*;
use crate::completion::domain::value_objects::alias_table::parse_aliases;
use std::collections::HashMap;

fn cmd(buf: &str, shell: Shell, aliases: &[(&str, &str)]) -> Option<CommandLine> {
    let map = aliases
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    current_command(buf, shell, &map)
}

fn toks_in(buf: &str, shell: Shell, aliases: &[(&str, &str)]) -> Vec<String> {
    cmd(buf, shell, aliases)
        .map(|c| c.tokens().iter().map(|t| t.text().to_string()).collect())
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
    assert_eq!(toks("  "), [""]); // two spaces give one empty token
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
        (c.tokens()[1].text(), c.tokens()[1].raw()),
        ("my fi", "'my fi")
    );
    let c = cmd("cat \"a b\"", Shell::Bash, &[]).unwrap();
    assert_eq!(
        (c.tokens()[1].text(), c.tokens()[1].raw()),
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
        (c.tokens()[1].text(), c.tokens()[1].raw()),
        (" fo", "\\ fo")
    );
    let c = cmd("cd my\\ di", Shell::Bash, &[]).unwrap();
    assert_eq!(c.tokens()[1].raw(), "my\\ di");
}

#[test]
fn concatenation() {
    let c = cmd("cmd --foo=\"a b\"c", Shell::Bash, &[]).unwrap();
    assert_eq!(c.tokens().len(), 2);
    assert_eq!(
        (c.tokens()[1].text(), c.tokens()[1].raw()),
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
    assert_eq!(c.text(), "git  co ");
    for t in c.tokens() {
        assert_eq!(&c.text()[t.start()..t.end()], t.raw());
    }
    assert_eq!((c.tokens()[2].start(), c.tokens()[2].end()), (8, 8));
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
fn alias_expansion_cases() {
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
    assert_eq!(c.text(), "git co ");
    assert_eq!(c.original_text(), "g co "); // pre-expansion text for the menu (alias-command-text-hides-menu)
    let raws: Vec<_> = c.tokens().iter().map(|t| t.raw()).collect();
    assert_eq!(raws, ["g", "co", ""]);
    let ll = cmd("ll ", Shell::Bash, &aliases).unwrap();
    assert_eq!(
        (ll.text(), ll.original_text()),
        ("ls -G -l ", "ll ")
    );
    assert_eq!(
        cmd("echo | g ch", Shell::Bash, &aliases)
            .unwrap()
            .original_text(),
        "g ch"
    );
    // Multi-word alias: every alias token keeps the alias' raw; later tokens map 1:1.
    let c = cmd("ll foo ", Shell::Bash, &aliases).unwrap();
    let pairs: Vec<_> = c
        .tokens()
        .iter()
        .map(|t| (t.text(), t.raw()))
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
    assert_eq!(s.text(), "git checkout -b ");
    let pairs: Vec<_> = s
        .tokens()
        .iter()
        .map(|t| (t.text(), t.raw()))
        .collect();
    assert_eq!(
        pairs,
        [("git", "git"), ("checkout", "co"), ("-b", "-b"), ("", "")]
    );
    let s = substitute_token(&c, 1, "commit -m", Shell::Bash).unwrap();
    let texts: Vec<_> = s.tokens().iter().map(|t| t.text()).collect();
    assert_eq!(texts, ["git", "commit", "-m", "-b", ""]);
    assert_eq!(s.tokens()[2].raw(), "co");
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
    assert_eq!(c.tokens()[1].raw(), "my` di");
}

/// Parse trees as JSON fixtures, compared on type, offsets, complete and innerText.
#[test]
fn parse_tree_fixtures() {
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
        // D22: a word starting with an escape starts at it; the fixture's offset is one past it.
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
            include_str!("../../../../../tests/fixtures/shell_parser/basic/input.sh"),
            include_str!("../../../../../tests/fixtures/shell_parser/basic/output.txt"),
        ),
        (
            "multipleStatements",
            include_str!("../../../../../tests/fixtures/shell_parser/multipleStatements/input.sh"),
            include_str!("../../../../../tests/fixtures/shell_parser/multipleStatements/output.txt"),
        ),
        (
            "primaryExpressions",
            include_str!("../../../../../tests/fixtures/shell_parser/primaryExpressions/input.sh"),
            include_str!("../../../../../tests/fixtures/shell_parser/primaryExpressions/output.txt"),
        ),
        (
            "variables",
            include_str!("../../../../../tests/fixtures/shell_parser/variables/input.sh"),
            include_str!("../../../../../tests/fixtures/shell_parser/variables/output.txt"),
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
