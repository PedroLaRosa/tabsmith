#!/bin/bash
# Usage: precmd-diff.sh <tabsmith binary>
# Prints a scrubbed transcript of `tabsmith _ pre-cmd` behaviour (exit codes, stdout/stderr, run/ files, modes,
# pruning) in an isolated sandbox. The golden capture never runs `_ pre-cmd`, so Phase 4 diffs this instead:
#   diff <(precmd-diff.sh target/hexagonal-refactor/baseline-tree/target/debug/tabsmith) \
#        <(precmd-diff.sh target/debug/tabsmith)
# macOS (`stat -f`, `touch -t`). The env is cleared (`env -i`) so the reported env is known; ts is printed as a type.
BIN="$(cd "$(dirname "$1")" && pwd -P)/$(basename "$1")"
SB=$(cd "$(mktemp -d)" && pwd -P); trap 'rm -rf "$SB"' EXIT
D="$SB/home/.tabsmith"; mkdir -p "$D"; cd "$SB" || exit 1
scrub() { sed -e "s#$SB#\$SANDBOX#g" -e 's/tmp-[0-9][0-9]*/tmp-PID/g'; }
run() { # run <env assignments...> -- <args...>
  local envs=() ; while [ "$1" != "--" ]; do envs+=("$1"); shift; done; shift
  echo "### ${envs[*]} _ pre-cmd $*" | scrub
  env -i PATH=/usr/bin:/bin "${envs[@]}" "$BIN" _ pre-cmd "$@" >"$SB/o" 2>"$SB/e"; local c=$?
  echo "stdout=[$(cat "$SB/o")] stderr=[$(cat "$SB/e")] exit=$c" | scrub
}
listing() { echo "## run/ $1"; [ -d "$D/run" ] || { echo "(no run dir)"; return; }
  stat -f '%Lp %HT' "$D/run"
  (cd "$D/run" && for g in $(ls -A); do echo "$g $(stat -f '%Lp %HT' "$g")"; done) | scrub | sort; }
context() { python3 -c 'import json,sys; d=json.load(open(sys.argv[1])); print(list(d), sorted(d["env"].items()), repr(d["alias"]), type(d["ts"]).__name__)' "$1" | scrub; }
H=("HOME=$SB/home" "TABSMITH_HOME=$D")

run "${H[@]}" --                                   # no session id
run "${H[@]}" TABSMITH_SESSION_ID= --              # empty id
run "${H[@]}" TABSMITH_SESSION_ID=a.b --           # '.' rejected
run "${H[@]}" TABSMITH_SESSION_ID=a/b --           # path rejected
run "${H[@]}" "TABSMITH_SESSION_ID=a b" --         # space rejected
run "${H[@]}" TABSMITH_SESSION_ID=é --             # non-ASCII rejected
run HOME= TABSMITH_HOME= TABSMITH_SESSION_ID=a.b -- # invalid id without a home: silent exit 1
run HOME= TABSMITH_HOME= TABSMITH_SESSION_ID=abc -- # valid id without a home: the home error
run "HOME=$SB/home" "TABSMITH_HOME=$SB/purged" TABSMITH_SESSION_ID=abc --  # purged root is not recreated
[ -e "$SB/purged" ] && echo "purged root recreated" || echo "purged root still absent"
listing "before any write"
run "${H[@]}" TABSMITH_SESSION_ID=Ab-_9 -- --alias=-x
listing "after the first write"; context "$D/run/Ab-_9.json"
run "${H[@]}" TABSMITH_SESSION_ID=nal --
context "$D/run/nal.json"; head -c 7 "$D/run/nal.json"; echo
run "${H[@]}" TABSMITH_SESSION_ID=emp -- --alias ""
context "$D/run/emp.json"
# stale entries: a regular file, a foreign temp file, a symlink to a stale file, a directory; a future-dated file
printf '{}' > "$D/run/old.json"; touch -t 202001010000 "$D/run/old.json"
printf 'x' > "$D/run/x.tmp-1"; touch -t 202001010000 "$D/run/x.tmp-1"
printf '{}' > "$SB/old-target"; touch -t 202001010000 "$SB/old-target"
ln -s "$SB/old-target" "$D/run/link.json"; touch -h -t 202001010000 "$D/run/link.json"
mkdir "$D/run/debug-shell-1"; touch -t 202001010000 "$D/run/debug-shell-1"
printf '{}' > "$D/run/future.json"; touch -t 209901010000 "$D/run/future.json"
mkdir "$D/run/dir.json"; chmod 755 "$D/run"
run "${H[@]}" TABSMITH_SESSION_ID=dir --           # rename onto a directory fails: no prune, temp file left
listing "after a failed write (nothing pruned, run/ chmod 0700 anyway)"
run "${H[@]}" TABSMITH_SESSION_ID=ok1 --
listing "after a successful write (only stale regular files pruned)"
[ -e "$SB/old-target" ] && echo "symlink target kept" || echo "symlink target deleted"
