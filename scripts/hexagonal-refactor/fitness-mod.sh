#!/bin/bash
# fitness-mod.sh [--worktree] [module ...]
#
# Runs the architecture fitness test (src/architecture_tests.rs) on a COPY of the repository in which the named
# modules are added to REGISTERED_MODULES, so a step can be checked against a module that is not registered yet, or a
# candidate file can be dropped into a layer folder of the copy without touching the real tree.
#
#   fitness-mod.sh                      HEAD, nothing added (after Phase 9's registration: the committed test)
#   fitness-mod.sh terminal_session     HEAD with terminal_session registered (before it is committed)
#   fitness-mod.sh --worktree           the working tree (tracked and untracked, not ignored files), nothing added
#   fitness-mod.sh --worktree dropdown  the working tree with dropdown added
#
# Environment: FITNESS_COPY (default target/hexagonal-refactor/fitness-copy) is the copy, FITNESS_TARGET (default
# $FITNESS_COPY/target) the cargo target dir. The copy keeps its target dir between runs, so a rerun only recompiles the
# crate (about 25 s); the first run builds every dependency (about 70 s).
# Exit status: the test's. On failure prints the violations (one `src/<file>:<line>: <rule>: <code>` per line).
# Run it from anywhere inside the repository. It never writes outside target/ and never builds the real tree.
set -u
REPO="$(git rev-parse --show-toplevel)" || exit 2
COPY="${FITNESS_COPY:-$REPO/target/hexagonal-refactor/fitness-copy}"
TARGET="${FITNESS_TARGET:-$COPY/target}"
MODE=head
if [ "${1:-}" = "--worktree" ]; then MODE=worktree; shift; fi
MODULES=("$@")

mkdir -p "$COPY"
# Everything but the cargo target dir goes, so a file deleted since the last run is gone from the copy too.
find "$COPY" -mindepth 1 -maxdepth 1 ! -name target -exec rm -rf {} +
if [ "$MODE" = head ]; then
  git -C "$REPO" archive HEAD | tar -x -C "$COPY"
else
  (cd "$REPO" && git ls-files -z --cached --others --exclude-standard |
    while IFS= read -r -d '' f; do [ -e "$f" ] && printf '%s\0' "$f"; done |
    tar --null -T - -cf -) | tar -x -C "$COPY"
fi

python3 -I - "$COPY/src/architecture_tests.rs" "${MODULES[@]}" <<'EOF' || exit 2
import re, sys
path, add = sys.argv[1], sys.argv[2:]
text = open(path, encoding="utf8").read()
m = re.search(r"(const REGISTERED_MODULES: &\[&str\] =\s*&\[)([^\]]*)(\];)", text)
if not m:
    sys.exit("fitness-mod.sh: REGISTERED_MODULES not found in " + path)
names = re.findall(r'"([a-z_]+)"', m.group(2))
for a in add:
    if a not in names:
        names.append(a)
names.sort()
text = text[:m.start()] + m.group(1) + ", ".join(f'"{n}"' for n in names) + m.group(3) + text[m.end():]
open(path, "w", encoding="utf8").write(text)
print("registered: " + " ".join(names))
EOF

cd "$COPY" || exit 2
LOG="$(mktemp)"
CARGO_TARGET_DIR="$TARGET" cargo test --locked architecture >"$LOG" 2>&1
STATUS=$?
if [ $STATUS -eq 0 ]; then
  grep -E "^test result" "$LOG" | head -1
  echo "FITNESS PASS"
else
  if grep -q "architecture rules broken" "$LOG"; then
    sed -n '/architecture rules broken/,/^stack backtrace\|^note: run with/p' "$LOG" | grep -v '^stack backtrace\|^note: run with'
  else
    tail -40 "$LOG"   # a compile error in the copy, not a rule
  fi
  echo "FITNESS FAIL"
fi
rm -f "$LOG"
exit $STATUS
