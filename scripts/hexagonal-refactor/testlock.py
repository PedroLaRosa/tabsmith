#!/usr/bin/env python3
"""Compare every #[test] fn body between a baseline git revision and the working tree.

Usage: testlock.py <repo> <baseline-rev> [--diff]

Keys tests by fn name, so a name defined twice hides one of the two tests: DUPLICATE lists every such name in the
tree (it does not fail the run). Bodies are normalised twice:
  - "exact": comments dropped, whitespace collapsed;
  - "loose": additionally every `a::b::c` path is reduced to its last segment, so pure moves
    (`crate::settings::Settings` -> `crate::settings::domain::settings::Settings`) do not count.
Reports MISSING (in baseline, not in tree), NEW, CHANGED (loose bodies differ, with a diff when --diff),
and PATH-ONLY (only the exact form differs).
"""
import difflib
import pathlib
import re
import subprocess
import sys


def lex(src):
    """Yield (kind, text) tokens; kind in {'ws','comment','str','char','code'}."""
    i, n = 0, len(src)
    while i < n:
        c = src[i]
        if src.startswith("//", i):
            j = src.find("\n", i)
            j = n if j < 0 else j
            yield "comment", src[i:j]
            i = j
        elif src.startswith("/*", i):
            depth, j = 1, i + 2
            while j < n and depth:
                if src.startswith("/*", j):
                    depth, j = depth + 1, j + 2
                elif src.startswith("*/", j):
                    depth, j = depth - 1, j + 2
                else:
                    j += 1
            yield "comment", src[i:j]
            i = j
        elif re.match(r'b?r(#*)"', src[i:i + 300]):
            m = re.match(r'b?r(#*)"', src[i:])
            close = '"' + m.group(1)
            j = src.find(close, i + m.end())
            j = n if j < 0 else j + len(close)
            yield "str", src[i:j]
            i = j
        elif c == '"' or src.startswith('b"', i):
            j = i + (2 if c == "b" else 1)
            while j < n and src[j] != '"':
                j += 2 if src[j] == "\\" else 1
            yield "str", src[i:j + 1]
            i = j + 1
        elif c == "'":
            m = re.match(r"'(\\.[^']*|[^\\'])'", src[i:])
            if m:
                yield "char", m.group(0)
                i += m.end()
            else:
                yield "code", c
                i += 1
        elif c.isspace():
            j = i
            while j < n and src[j].isspace():
                j += 1
            yield "ws", src[i:j]
            i = j
        else:
            yield "code", c
            i += 1


def tests_in(src):
    toks = list(lex(src))
    text = "".join(t for _, t in toks)
    # positions of code chars only, to brace-match safely
    out = []
    pos = 0
    spans = []
    for kind, t in toks:
        spans.append((pos, kind, t))
        pos += len(t)
    code_mask = [False] * len(text)
    for p, kind, t in spans:
        if kind == "code":
            code_mask[p] = True
    for m in re.finditer(r"#\[test\]", text):
        if not code_mask[m.start()]:
            continue
        fm = re.compile(r"\s*(?:#\[[^\]]*\]\s*|//[^\n]*\n\s*)*(?:async\s+)?fn\s+(\w+)").match(text, m.end())
        if not fm:
            continue
        name = fm.group(1)
        k = text.find("{", fm.end())
        while k >= 0 and not code_mask[k]:
            k = text.find("{", k + 1)
        depth, j = 0, k
        while j < len(text):
            if code_mask[j]:
                if text[j] == "{":
                    depth += 1
                elif text[j] == "}":
                    depth -= 1
                    if depth == 0:
                        break
            j += 1
        body = text[k:j + 1]
        out.append((name, body))
    return out


def normalise(body, loose):
    parts = []
    for kind, t in lex(body):
        if kind == "comment":
            continue
        if kind == "ws":
            parts.append(" ")
            continue
        parts.append(t)
    s = re.sub(r" +", " ", "".join(parts)).strip()
    s = re.sub(r" ?([(){}\[\],;.:&<>=!+\-*/|]) ?", r"\1", s)
    if loose:
        s = re.sub(r"(?:\b\w+::)+(\w+)", r"\1", s)
    return s


def collect_tree(repo):
    """The tests by name, and the names defined more than once."""
    out, duplicates = {}, set()
    for p in sorted(pathlib.Path(repo, "src").rglob("*.rs")):
        for name, body in tests_in(p.read_text()):
            if name in out:
                duplicates.add(name)
            out[name] = (str(p.relative_to(repo)), body)
    return out, sorted(duplicates)


def collect_rev(repo, rev):
    files = subprocess.run(["git", "-C", repo, "ls-tree", "-r", "--name-only", rev, "src"],
                           capture_output=True, text=True, check=True).stdout.split()
    out = {}
    for f in files:
        if not f.endswith(".rs"):
            continue
        src = subprocess.run(["git", "-C", repo, "show", f"{rev}:{f}"], capture_output=True, text=True,
                             check=True).stdout
        for name, body in tests_in(src):
            out[name] = (f, body)
    return out


def main():
    repo, rev = sys.argv[1], sys.argv[2]
    show_diff = "--diff" in sys.argv
    base, (tree, duplicates) = collect_rev(repo, rev), collect_tree(repo)
    missing = sorted(set(base) - set(tree))
    new = sorted(set(tree) - set(base))
    changed, path_only = [], []
    for name in sorted(set(base) & set(tree)):
        b, t = base[name][1], tree[name][1]
        if normalise(b, True) != normalise(t, True):
            changed.append(name)
        elif normalise(b, False) != normalise(t, False):
            path_only.append(name)
    print(f"baseline tests: {len(base)}  tree tests: {len(tree)}")
    print(f"MISSING ({len(missing)}): {' '.join(missing)}")
    print(f"NEW ({len(new)}): {' '.join(new)}")
    print(f"PATH-ONLY ({len(path_only)})")
    print(f"CHANGED ({len(changed)}): {' '.join(changed)}")
    print(f"DUPLICATE ({len(duplicates)}): {' '.join(duplicates)}")
    if show_diff:
        for name in changed:
            print(f"\n===== {name}  {base[name][0]} -> {tree[name][0]}")
            a = base[name][1].splitlines()
            b = tree[name][1].splitlines()
            sys.stdout.writelines(l + "\n" for l in difflib.unified_diff(a, b, "baseline", "tree", lineterm="", n=1))
    sys.exit(1 if missing or changed else 0)


if __name__ == "__main__":
    main()
