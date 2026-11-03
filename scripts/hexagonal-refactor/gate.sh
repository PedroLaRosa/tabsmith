#!/bin/bash
# Refactor gate for the hexagonal refactor (docs/hexagonal-refactor-plan.md). Run from the repo root before every
# commit:
#   scripts/hexagonal-refactor/gate.sh          build, tests (>= baseline), clippy (<= baseline), golden CLI output,
#                                                test-lock report
#   scripts/hexagonal-refactor/gate.sh --fast   the same without the golden capture
# Exit code 0 only when every check passes. Test-lock CHANGED entries do not fail the gate (a split or a
# restructuring may be legitimate) but are listed so the step can justify each one.
# Needs the golden baseline once per machine: scripts/hexagonal-refactor/make-baseline.sh
HERE="$(cd "$(dirname "$0")" && pwd)"
[ "$(uname)" = Linux ] && export PATH="$HERE/compat:$PATH"   # macOS `stat -f` / `sed -i ''` shims for golden.sh
WORK="$PWD/target/hexagonal-refactor"
BASE="$WORK/baseline"
BASELINE_REV=ef807c1
BASELINE_PASSED=283
BASELINE_CLIPPY=${BASELINE_CLIPPY:-3}   # Linux with clippy 1.97 reports 4 (one new lint in completion_result_builder.rs): BASELINE_CLIPPY=4
fail=0
LOG="$WORK/gate-runs/$$"   # per run, so concurrent gates never share logs; gate-last points at the newest
mkdir -p "$LOG"; ln -sfn "$LOG" "$WORK/gate-last"

if [ "$1" != "--fast" ] && [ ! -f "$BASE/golden.txt" ]; then
  echo "GATE FAIL: no golden baseline; run scripts/hexagonal-refactor/make-baseline.sh first"; exit 1
fi

echo "== build"
if ! cargo build --locked >"$LOG/build.txt" 2>&1; then
  grep -E "^(error|warning)" -A6 "$LOG/build.txt" | head -80; echo "GATE FAIL: build"; exit 1
fi
grep -cE "^warning" "$LOG/build.txt" | sed 's/^/build warnings: /'

echo "== test"
cargo test --locked >"$LOG/test.txt" 2>&1
res=$(grep -E "^test result" "$LOG/test.txt" | head -1)
echo "$res"
passed=$(echo "$res" | sed -E 's/.* ([0-9]+) passed.*/\1/')
failed=$(echo "$res" | sed -E 's/.* ([0-9]+) failed.*/\1/')
if [ -z "$res" ] || [ "$failed" != "0" ] || [ "$passed" -lt "$BASELINE_PASSED" ]; then
  grep -E "^test .*FAILED|panicked|^error" -A10 "$LOG/test.txt" | head -80
  mkdir -p "$WORK/test-failures"; keep="$WORK/test-failures/$(date +%Y%m%d-%H%M%S)-$$.txt"; cp "$LOG/test.txt" "$keep"
  echo "GATE FAIL: tests (passed=$passed, baseline=$BASELINE_PASSED, failed=$failed); full output kept at $keep"; fail=1
fi

echo "== clippy"
cargo clippy --all-targets --locked >"$LOG/clippy.txt" 2>&1
warnings=$(grep -E "^warning" "$LOG/clippy.txt" | grep -vc "generated [0-9]* warning")
errors=$(grep -cE "^error" "$LOG/clippy.txt")
echo "clippy warnings: $warnings (baseline $BASELINE_CLIPPY), errors: $errors"
if [ "$warnings" -gt "$BASELINE_CLIPPY" ] || [ "$errors" -gt 0 ]; then
  grep -E "^(warning|error)" -A8 "$LOG/clippy.txt" | head -120; echo "GATE FAIL: clippy"; fail=1
fi

# cfg(windows) code is never built by the host build: check it when the target and a mingw C compiler are installed
# (rustup target add x86_64-pc-windows-gnu; apt-get install gcc-mingw-w64-x86-64).
if rustup target list --installed 2>/dev/null | grep -q '^x86_64-pc-windows-gnu$' && command -v x86_64-w64-mingw32-gcc >/dev/null; then
  echo "== windows check"
  if cargo check --locked --all-targets --target x86_64-pc-windows-gnu >"$LOG/windows.txt" 2>&1; then echo "windows: ok"
  else grep -E "^(error|warning)" -A8 "$LOG/windows.txt" | head -60; echo "GATE FAIL: windows check"; fail=1; fi
fi

if [ "$1" != "--fast" ]; then
  echo "== golden CLI output"
  # run a copy at a fixed depth under target/, like the baseline binary (golden.sh scrubs both paths)
  GB="$WORK/gate-bin/$$/target/debug"; mkdir -p "$GB"; cp "$PWD/target/debug/tabsmith" "$GB/tabsmith"
  "$HERE/golden.sh" "$GB/tabsmith" "$LOG/golden.txt"; rm -rf "$WORK/gate-bin/$$"
  if diff "$BASE/golden.txt" "$LOG/golden.txt" >"$LOG/golden.diff"; then echo "golden: identical"
  else head -60 "$LOG/golden.diff"; echo "GATE FAIL: golden output differs (full diff: $LOG/golden.diff)"; fail=1; fi
fi

echo "== test lock (baseline $BASELINE_REV)"
python3 "$HERE/testlock.py" . "$BASELINE_REV" >"$LOG/testlock.txt"
head -6 "$LOG/testlock.txt"
if grep -q "^MISSING ([1-9]" "$LOG/testlock.txt"; then echo "GATE FAIL: baseline tests missing"; fail=1; fi
echo "(diffs of CHANGED tests: python3 $HERE/testlock.py . $BASELINE_REV --diff)"

[ $fail = 0 ] && echo "GATE PASS" || echo "GATE FAIL"
exit $fail
