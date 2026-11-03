#!/bin/bash
# Build tabsmith at the pre-refactor commit and capture its CLI output as the golden baseline the gate diffs
# against. Run once per machine from the repo root; everything lands in target/hexagonal-refactor/ (git-ignored).
set -e
HERE="$(cd "$(dirname "$0")" && pwd)"
[ "$(uname)" = Linux ] && export PATH="$HERE/compat:$PATH"   # macOS `stat -f` / `sed -i ''` shims for golden.sh
WORK="$PWD/target/hexagonal-refactor"
REV=af1b304   # ef807c1 + the Phase 0 baseline note; same src/ as ef807c1
TREE="$WORK/baseline-tree"
mkdir -p "$WORK/baseline"
if [ ! -d "$TREE" ]; then git worktree add --detach "$TREE" "$REV"; fi
(cd "$TREE" && cargo build --locked)
"$HERE/golden.sh" "$TREE/target/debug/tabsmith" "$WORK/baseline/golden.txt"
echo "baseline: $(grep -c '^### ' "$WORK/baseline/golden.txt") commands captured in $WORK/baseline/golden.txt"
