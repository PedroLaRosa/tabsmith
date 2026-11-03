#!/usr/bin/env python3
"""phase10-literals.py <base-rev>: the string literals (8+ chars, raw source text) of the production part of
src/doctor.rs and src/cli.rs at <base-rev> that appear nowhere in src/diagnostics or src/bootstrap. One per line.
Comments, char literals and lifetimes are skipped; the test module (everything after the first #[cfg(test)]) is cut."""
import pathlib
import subprocess
import sys


def literals(src):
    out, i, n = [], 0, len(src)
    while i < n:
        if src.startswith("//", i):
            i = src.find("\n", i)
            i = n if i < 0 else i
        elif src.startswith("/*", i):
            i = src.find("*/", i)
            i = n if i < 0 else i + 2
        elif src[i] == "'":
            j = i + 1
            if j < n and src[j] == "\\":
                j = src.find("'", j + 2)
                i = n if j < 0 else j + 1
            elif j + 1 < n and src[j + 1] == "'":
                i = j + 2
            else:
                i += 1  # a lifetime
        elif src[i] == '"':
            j = i + 1
            while j < n and src[j] != '"':
                j += 2 if src[j] == "\\" else 1
            if j - i - 1 >= 8:
                out.append(src[i + 1:j])
            i = j + 1
        else:
            i += 1
    return out


def old_source(base, path):
    text = subprocess.run(["git", "show", f"{base}:{path}"], capture_output=True, text=True).stdout
    return text.split("#[cfg(test)]")[0]


base = sys.argv[1]
old = "\n".join(old_source(base, f) for f in ("src/doctor.rs", "src/cli.rs"))
new = "".join(
    p.read_text()
    for d in ("src/diagnostics", "src/bootstrap")
    if pathlib.Path(d).exists()
    for p in sorted(pathlib.Path(d).rglob("*.rs"))
)
for lit in sorted(set(literals(old))):
    if lit not in new:
        print(lit.replace("\n", "\\n")[:140])
