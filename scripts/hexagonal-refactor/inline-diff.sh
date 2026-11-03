#!/bin/bash
# Usage: inline-diff.sh <tabsmith binary>
# Prints a scrubbed transcript of what the golden capture never shows of `tabsmith inline` and doctor's Inline
# check: valid and invalid `inline.profiles.<name>` (every reason), a selected profile, set-customization by name and
# through the numbered chooser on a pty, a non-bool `inline.enabled`, and every inline command over a corrupted
# settings.json, without a home and with only TABSMITH_SETTINGS_PATH. Phase 7 diffs it at every step:
#   diff <(scripts/hexagonal-refactor/inline-diff.sh target/hexagonal-refactor/baseline-tree/target/debug/tabsmith) \
#        <(scripts/hexagonal-refactor/inline-diff.sh target/debug/tabsmith)
# macOS. Each group starts from a fresh sandbox; commands run with a cleared env (`env -i`) and stdin from /dev/null,
# except the pty group.
BIN="$(cd "$(dirname "$1")" && pwd -P)/$(basename "$1")"
BINDIR="$(dirname "$BIN")"
SANDBOXES=()
cleanup() { for s in "${SANDBOXES[@]}"; do rm -rf "$s"; done; }
trap cleanup EXIT

fresh() {
  local t; t=$(mktemp -d) || exit 1
  SB=$(cd "$t" && pwd -P) || exit 1
  SANDBOXES+=("$SB"); mkdir -p "$SB/home/.tabsmith"; cd "$SB" || exit 1
  echo; echo "======== $1"
}
scrub() { sed -e "s#$SB#\$SANDBOX#g" -e "s#$BIN#\$BIN#g" -e "s#$BINDIR#\$BINDIR#g"; }
show() { scrub <"$1"; if [ -s "$1" ] && [ -n "$(tail -c 1 "$1")" ]; then echo; echo "(no newline at end)"; fi; }
put() { printf '%s' "$1" >"$SB/home/.tabsmith/settings.json"; }   # settings.json holds exactly these bytes
saved() { echo "--- settings.json"
  if [ -e "$SB/home/.tabsmith/settings.json" ]; then show "$SB/home/.tabsmith/settings.json"; else echo "(none)"; fi; }
# run [VAR=value ...] -- <args...>: one command, stdin from /dev/null
run() {
  local envs=(); while [ "$1" != "--" ]; do envs+=("$1"); shift; done; shift
  echo "### ${envs[*]} $*" | scrub
  env -i PATH="$PATH" HOME="$SB/home" TABSMITH_HOME="$SB/home/.tabsmith" USERPROFILE="$SB/home" "${envs[@]}" \
    "$BIN" "$@" </dev/null >"$SB/out" 2>"$SB/err"
  local code=$?
  show "$SB/out"; echo "--- stderr"; show "$SB/err"; echo "--- exit $code"
}
# doctor [VAR=value ...]: the Inline lines of `doctor --all` and its exit code (other checks are not this phase's)
doctor() {
  echo "### $* doctor --all (Inline lines)" | scrub
  env -i PATH="$PATH" HOME="$SB/home" TABSMITH_HOME="$SB/home/.tabsmith" USERPROFILE="$SB/home" "$@" \
    "$BIN" doctor --all </dev/null >"$SB/out" 2>&1
  local code=$?
  grep -E 'Inline|is not set|not a valid profile' "$SB/out" | scrub; echo "--- exit $code"
}
# The pty driver: argv = <answer> <stdout file or -> <stderr file> <command...>. stdin is the pty slave (ECHO off),
# stdout the slave unless a file is given. <answer> (EMPTY an empty line, EOF a Ctrl-D) is written once
# `Select a customization: ` was read from the pty, or right after the start when stdout is a file (the prompt then
# goes to the file; the tty queues the answer until read_line). Prints the pty bytes (\r\n -> \n) and exits with the
# child's code.
PTY='
import os, select, subprocess, sys, termios, time
answer, out_path, err_path, cmd = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4:]
pending = {"EMPTY": b"\n", "EOF": b"\x04"}.get(answer, answer.encode() + b"\n")
master, slave = os.openpty()
attrs = termios.tcgetattr(slave)
attrs[3] &= ~termios.ECHO
termios.tcsetattr(slave, termios.TCSANOW, attrs)
out = slave if out_path == "-" else open(out_path, "wb")
err = open(err_path, "wb")
child = subprocess.Popen(cmd, stdin=slave, stdout=out, stderr=err)
os.close(slave)
if out_path != "-":
    os.write(master, pending)
    pending = None
data, deadline = b"", time.time() + 30
while True:
    ready = select.select([master], [], [], 0.05)[0]
    chunk = b""
    if ready:
        try:
            chunk = os.read(master, 65536)
        except OSError:
            chunk = b""
    if chunk:
        data += chunk
        if pending is not None and b"Select a customization: " in data:
            os.write(master, pending)
            pending = None
        continue
    if child.poll() is not None:
        break
    if time.time() > deadline:
        child.kill()
        child.wait()
        data += b"\n(killed: timed out)\n"
        break
sys.stdout.buffer.write(data.replace(b"\r\n", b"\n"))
sys.stdout.flush()
code = child.returncode
sys.exit(code if code >= 0 else 128 - code)
'
# prun <answer> <pty|file> [VAR=value ...] -- <args...>: one command with stdin on a pty (stdout on it, or in a file)
prun() {
  local answer=$1 where=$2; shift 2
  local envs=(); while [ "$1" != "--" ]; do envs+=("$1"); shift; done; shift
  echo "### pty(answer=$answer, stdout=$where) ${envs[*]} $*" | scrub
  local o=-; [ "$where" = file ] && o="$SB/out"
  python3 -c "$PTY" "$answer" "$o" "$SB/err" env -i PATH="$PATH" HOME="$SB/home" \
    TABSMITH_HOME="$SB/home/.tabsmith" USERPROFILE="$SB/home" "${envs[@]}" "$BIN" "$@" >"$SB/pty"
  local code=$?
  echo "--- pty"; show "$SB/pty"
  [ "$where" = file ] && { echo "--- stdout"; show "$SB/out"; }
  echo "--- stderr"; show "$SB/err"; echo "--- exit $code"
}

VALID='"inline.profiles.a":{"provider":"openai","model":"gpt"},"inline.profiles.b":{"provider":"anthropic","model":"claude","effort":"low","base_url":"http://localhost:8080/","api_key_env":"","max_tokens":5,"extra":1},"inline.profiles.c":{"provider":"gemini","model":"x"}'
REASONS='"inline.profiles.history":{"provider":"openai","model":"x"},"inline.profiles.str":"openai","inline.profiles.noprov":{"model":"x"},"inline.profiles.numprov":{"provider":1,"model":"x"},"inline.profiles.nomodel":{"provider":"openai"},"inline.profiles.empty":{"provider":"openai","model":""},"inline.profiles.nummodel":{"provider":"openai","model":2},"inline.profiles.zero":{"provider":"openai","model":"x","max_tokens":0},"inline.profiles.float":{"provider":"openai","model":"x","max_tokens":1.5},"inline.profiles.big":{"provider":"openai","model":"x","max_tokens":4294967296},"inline.profiles.strtok":{"provider":"openai","model":"x","max_tokens":"5"},"inline.profiles.url":{"provider":"openai","model":"x","base_url":1},"inline.profiles.key":{"provider":"openai","model":"x","api_key_env":true},"inline.profiles.eff":{"provider":"openai","model":"x","effort":3},"inline.profiles.nulls":{"provider":"openai","model":"x","base_url":null,"max_tokens":null,"effort":null},"inline.profiles.a.b":{"provider":"openai","model":"dot"},"inline.profiles.":{"provider":"openai","model":"e"}'
CMDS=("status" "enable" "disable" "show-customizations" "show-customizations --format json"
      "set-customization history" "set-customization")

fresh "show-customizations"
for s in "{\"inline.source\":\"b\",$VALID,$REASONS}" "{\"inline.source\":\"c\",$VALID}" '{"inline.source":"nope"}' \
         '{"inline.profiles.c":{"provider":"gemini","model":"x"}}' '{"inline.source":5}' '{}'; do
  put "$s"; run -- inline show-customizations; run -- inline show-sources --format json
done

fresh "set-customization by name"
for name in a b c nope history specs history+specs ''; do
  put "{\"inline.source\":\"a\",\"autocomplete.height\":5,$VALID}"; run -- inline set-customization "$name"; saved
done
put "{$VALID}"; run -- inline set-source b; saved
put "{\"inline.source\":\"a\",$VALID}"; run -- inline set-customization; saved

fresh "the chooser on a pty"
for answer in 1 4 5 6 0 7 x EMPTY EOF; do
  put "{\"inline.source\":\"a\",\"autocomplete.height\":5,$VALID}"; prun "$answer" pty -- inline set-customization; saved
done
put "{$VALID}"; prun 4 file -- inline set-customization; saved
put '{}'; prun 2 pty -- inline set-source; saved

fresh "status, enable, disable"
run -- inline status; run -- inline disable; saved; run -- inline status; run -- inline enable; saved
for v in true false '"yes"' 0 null '{}'; do put "{\"inline.enabled\":$v}"; run -- inline status; done
put '{"inline.enabled":"yes","autocomplete.height":5}'; run -- inline disable; saved

fresh "corrupted settings.json"
for c in "${CMDS[@]}"; do put '{oops'; run -- inline $c; saved; done

fresh "no home"
for c in "${CMDS[@]}"; do run HOME= TABSMITH_HOME= USERPROFILE= -- inline $c; done

fresh "TABSMITH_SETTINGS_PATH only"
for c in "${CMDS[@]}"; do
  run HOME= TABSMITH_HOME= USERPROFILE= "TABSMITH_SETTINGS_PATH=$SB/s.json" -- inline $c
done
echo "--- s.json"; [ -e "$SB/s.json" ] && show "$SB/s.json" || echo "(none)"

fresh "doctor's Inline check"
put '{"inline.enabled":false}'; doctor
put '{"inline.source":"nope"}'; doctor
put "{\"inline.source\":\"c\",$VALID}"; doctor
put "{\"inline.source\":\"a\",$VALID}"; doctor; doctor OPENAI_API_KEY=; doctor OPENAI_API_KEY=k
put "{\"inline.source\":\"b\",$VALID}"; doctor
for u in 'http://127.0.0.1:1/v1' 'http://[::1]:1/v1' 'https://example.invalid/v1'; do
  put "{\"inline.source\":\"l\",\"inline.profiles.l\":{\"provider\":\"openai\",\"model\":\"m\",\"base_url\":\"$u\"}}"
  doctor
done
put '{"inline.enabled":"yes","inline.source":"nope"}'; doctor
