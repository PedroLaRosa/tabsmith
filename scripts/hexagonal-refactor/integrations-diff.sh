#!/bin/bash
# Usage: integrations-diff.sh <tabsmith binary>
# Prints a scrubbed transcript of the integration commands the golden capture does not cover (status JSON and pwsh
# errors, install/reinstall/uninstall, setup, uninstall, no home, init and the launch decision, doctor's fixes, the
# [Y/n] prompts and painted output on a pty, a data root outside HOME), with every file, directory (and its mode) and
# symlink left under HOME after each scenario. Phase 4 diffs it:
#   diff <(integrations-diff.sh target/hexagonal-refactor/baseline-tree/target/debug/tabsmith) \
#        <(integrations-diff.sh target/debug/tabsmith)
# macOS (`stat -f`). Each scenario runs in a fresh sandbox with a cleared env (`env -i`) and stdin from /dev/null,
# except the pty group. `sleep 1` precedes every command that may back up an existing dotfile: backups go to
# `backups/<now_secs>` and two backups in one second share that directory. One command that backs up several dotfiles
# reads the clock per dotfile and can, rarely, straddle a second: a diff only in `backups/SECS` lines is rerun once.
BIN="$(cd "$(dirname "$1")" && pwd -P)/$(basename "$1")"
BINDIR="$(dirname "$BIN")"
SANDBOXES=()
cleanup() { for s in "${SANDBOXES[@]}"; do chmod -R u+rwX "$s" 2>/dev/null; rm -rf "$s"; done; }
trap cleanup EXIT

# fresh <title>: a new empty sandbox (HOME = $SB/home), cwd = the sandbox
fresh() {
  local t; t=$(mktemp -d) || exit 1
  SB=$(cd "$t" && pwd -P) || exit 1
  case "$SB" in */tmp.*) ;; *) echo "unexpected sandbox: $SB"; exit 1 ;; esac
  SANDBOXES+=("$SB"); mkdir -p "$SB/home"; cd "$SB" || exit 1
  echo; echo "======== $1"
}
scrub() {
  sed -e "s#$SB#\$SANDBOX#g" -e "s#$BIN#\$BIN#g" -e "s#$BINDIR#\$BINDIR#g" \
      -e 's#backups/[0-9][0-9]*#backups/SECS#g' -e 's/tmp-[0-9][0-9]*/tmp-PID/g'
}
# show <file>: its scrubbed content, and whether it lacks a final newline
show() { scrub <"$1"; if [ -s "$1" ] && [ -n "$(tail -c 1 "$1")" ]; then echo; echo "(no newline at end)"; fi; }
not_root() { [ "$(id -u)" != 0 ] || { echo "(skipped: running as root)"; return 1; }; }

# run [VAR=value ...] -- <args...>: one command, stdin from /dev/null; later assignments win over the defaults
run() {
  local envs=(); while [ "$1" != "--" ]; do envs+=("$1"); shift; done; shift
  echo "### ${envs[*]} $*" | scrub
  env -i PATH="$PATH" HOME="$SB/home" TABSMITH_HOME="$SB/home/.tabsmith" USERPROFILE="$SB/home" "${envs[@]}" \
    "$BIN" "$@" </dev/null >"$SB/out" 2>"$SB/err"
  local code=$?
  show "$SB/out"; echo "--- stderr"; show "$SB/err"; echo "--- exit $code"
}
# brun: run a command that may back up a dotfile, one second after the previous one
brun() { sleep 1; run "$@"; }

# The pty driver: argv = <answer> <stdout file or -> <stderr file or -> <command...>. stdin is the pty slave (ECHO
# off, so only the program's bytes are recorded), stdout/stderr the slave unless a file is given. <answer> ('-' none,
# EMPTY an empty line) is written only after the prompt's `[Y/n] ` was read. Prints the pty bytes (\r\n -> \n) and
# exits with the child's code.
PTY='
import os, select, subprocess, sys, termios, time
answer, out_path, err_path, cmd = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4:]
pending = None if answer == "-" else (b"\n" if answer == "EMPTY" else answer.encode() + b"\n")
master, slave = os.openpty()
attrs = termios.tcgetattr(slave)
attrs[3] &= ~termios.ECHO
termios.tcsetattr(slave, termios.TCSANOW, attrs)
out = slave if out_path == "-" else open(out_path, "wb")
err = slave if err_path == "-" else open(err_path, "wb")
child = subprocess.Popen(cmd, stdin=slave, stdout=out, stderr=err)
os.close(slave)
data, deadline = b"", time.time() + 300
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
        if pending is not None and b"[Y/n] " in data:
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
# prun <answer> <pty|files> [VAR=value ...] -- <args...>: one command on a pty (stdout and stderr on it, or in files)
prun() {
  local answer=$1 where=$2; shift 2
  local envs=(); while [ "$1" != "--" ]; do envs+=("$1"); shift; done; shift
  echo "### pty(answer=$answer, stdout=$where) ${envs[*]} $*" | scrub
  local o=- e=-; [ "$where" = files ] && { o="$SB/out"; e="$SB/err"; }
  python3 -c "$PTY" "$answer" "$o" "$e" env -i PATH="$PATH" HOME="$SB/home" TABSMITH_HOME="$SB/home/.tabsmith" \
    USERPROFILE="$SB/home" "${envs[@]}" "$BIN" "$@" >"$SB/pty"
  local code=$?
  echo "--- pty"; show "$SB/pty"
  [ "$where" = files ] && { echo "--- stdout"; show "$SB/out"; echo "--- stderr"; show "$SB/err"; }
  echo "--- exit $code"
}

# dump <dir> <data root inside it>: every file (mode, content), directory (mode) and symlink (target), the data
# root's specs/** and logs/** excepted. Sorted before scrubbing, so backups/SECS entries keep their order.
dump() {
  local dir=$1 data=$2
  echo "## tree $dir" | scrub
  [ -d "$dir" ] || { echo "(absent)"; return; }
  (cd "$dir" && find . -type f ! -path "$data/specs/*" ! -path "$data/logs/*" | sort | while IFS= read -r f; do
    echo "# file $f $(stat -f %Lp "$f")" | scrub; show "$f"; done) 2>&1
  (cd "$dir" && find . -type d ! -path "$data/specs*" ! -path "$data/logs*" | sort | while IFS= read -r d; do
    echo "# dir $d $(stat -f %Lp "$d")"; done) 2>&1 | scrub | uniq
  (cd "$dir" && find . -type l | sort | while IFS= read -r l; do echo "# link $l -> $(readlink "$l")"; done) 2>&1 | scrub
}
home_dump() { dump "$SB/home" ./.tabsmith; }
NOHOME=(HOME= TABSMITH_HOME= USERPROFILE=)

fresh "a. status"
run -- integrations status dotfiles
run -- integrations status --format json all
run -- integrations status --format json-pretty dotfiles bash
run -- integrations status dotfiles pwsh
run -- integrations status --format json dotfiles pwsh
home_dump

fresh "b. install, reinstall, uninstall"
brun -- integrations install --silent dotfiles zsh
brun -- integrations install dotfiles
brun -- integrations install dotfiles
brun -- integrations reinstall dotfiles bash
brun -- integrations reinstall --silent all
home_dump
run -- integrations uninstall dotfiles
run -- integrations uninstall dotfiles
home_dump

fresh "c1. a directory at ~/.bashrc"
mkdir "$SB/home/.bashrc"
brun -- integrations install dotfiles bash
home_dump

fresh "c2. uninstall from a read-only ~/.zshrc"
if not_root; then
  brun -- integrations install dotfiles zsh
  chmod 444 "$SB/home/.zshrc"
  run -- integrations uninstall dotfiles zsh
  home_dump
fi

fresh "c3. explicit pwsh (not on PATH)"
brun -- integrations install dotfiles pwsh
home_dump

fresh "c4. ZDOTDIR"
brun "ZDOTDIR=$SB/zdot" -- integrations install dotfiles zsh
home_dump; dump "$SB/zdot" ./.tabsmith

fresh "c5. XDG_CONFIG_HOME"
brun "XDG_CONFIG_HOME=$SB/xdg" -- integrations install dotfiles fish
home_dump; dump "$SB/xdg" ./.tabsmith

fresh "c6. an opted-out ~/.bashrc"
printf '  #  TABSMITH ignore\n' >"$SB/home/.bashrc"
brun -- integrations install dotfiles bash
run -- integrations status dotfiles bash
head -2 "$SB/home/.profile" | sed 's/profile\.pre/bashrc.pre/g' >>"$SB/home/.bashrc"
run -- integrations uninstall dotfiles bash
home_dump

fresh "d1. setup"
run -- setup
brun -- setup --no-confirm
brun -- setup --no-confirm
home_dump

fresh "d2. setup over a directory at ~/.bashrc (manual fallback)"
mkdir "$SB/home/.bashrc"
brun -- setup --no-confirm
home_dump

fresh "e1. uninstall"
run -- uninstall
brun -- integrations install dotfiles
run -- uninstall -y
home_dump

fresh "e2. uninstall with a read-only ~/.zshrc"
if not_root; then
  brun -- integrations install dotfiles
  chmod 444 "$SB/home/.zshrc"
  run -- uninstall -y
  home_dump
fi

fresh "e3. uninstall --purge"
brun -- integrations install dotfiles
run -- uninstall -y --purge
[ -e "$SB/home/.tabsmith" ] && echo "HOME/.tabsmith still exists" || echo "HOME/.tabsmith removed"
home_dump

fresh "f. no home"
run "${NOHOME[@]}" -- setup --no-confirm
run "${NOHOME[@]}" -- uninstall -y
run "${NOHOME[@]}" -- integrations install dotfiles
run "${NOHOME[@]}" -- integrations reinstall dotfiles
home_dump

fresh "g. init and launch"
run TABSMITH_INIT_SNAPSHOT_TEST= -- init bash pre
run TABSMITH_INIT_SNAPSHOT_TEST= -- init pwsh pre
run TABSMITH_INIT_SNAPSHOT_TEST= -- init fish post
run -- init zsh pre
run -- init bash post --rcfile .bashrc
run -- _ should-launch
run TABSMITH_FORCE_LAUNCH= -- _ should-launch
run CI= -- _ should-launch
run TERM=dumb -- _ should-launch
run TERM=xterm-256color -- _ should-launch
run TERM=xterm-256color TABSMITH_TERM=1 TMUX=x -- _ should-launch
run -- _ get-shell
home_dump

# doctor fixes only without --all; this script's bash is its parent: bash checks are Normal (fixed), zsh/fish Soft
fresh "h1. doctor on a fresh HOME"
brun -- doctor
home_dump

fresh "h2. doctor with a zsh script pointing at another binary"
brun -- integrations install dotfiles bash
brun -- integrations install dotfiles zsh
sed -i '' "s#$BIN#/old/tabsmith#g" "$SB/home/.tabsmith/shell/zshrc.pre.zsh"
brun -- doctor
home_dump

fresh "h3. doctor over a directory at ~/.bashrc"
mkdir "$SB/home/.bashrc"
brun -- doctor
home_dump

fresh "h4. doctor over a read-only, then unreadable ~/.bashrc"
if not_root; then
  printf 'export A=1\n' >"$SB/home/.bashrc"; chmod 444 "$SB/home/.bashrc"
  brun -- doctor
  chmod 000 "$SB/home/.bashrc"
  brun -- doctor
  chmod 644 "$SB/home/.bashrc"
  home_dump
fi

fresh "i1. setup on a pty, declined"
sleep 1; prun n pty -- setup
home_dump

fresh "i2. setup on a pty, accepted; uninstall declined, then accepted"
sleep 1; prun EMPTY pty -- setup
prun n pty -- uninstall
prun y pty -- uninstall
home_dump

fresh "i3. the same without a home"
prun n pty "${NOHOME[@]}" -- setup
prun EMPTY pty "${NOHOME[@]}" -- setup
prun n pty "${NOHOME[@]}" -- uninstall
prun y pty "${NOHOME[@]}" -- uninstall
home_dump

fresh "i4. stdin on a pty, stdout and stderr in files"
prun - files TERM=xterm-256color -- _ should-launch
prun - files TERM=xterm-256color -- init bash pre

fresh "i5. painted output over a directory at ~/.bashrc"
mkdir "$SB/home/.bashrc"
sleep 1; prun - pty -- integrations install dotfiles bash
sleep 1; prun EMPTY pty -- setup
prun - pty -- integrations status dotfiles
home_dump

fresh "j. a data root outside HOME"
DATA=("TABSMITH_HOME=$SB/data")
brun "${DATA[@]}" -- integrations install dotfiles bash
run "${DATA[@]}" -- integrations status dotfiles bash
mkdir "$SB/home/.zshrc"
brun "${DATA[@]}" -- setup --no-confirm
rmdir "$SB/home/.zshrc"
home_dump; dump "$SB/data" .
mkdir -p "$SB/home/.tabsmith"; : >"$SB/home/.tabsmith/keep"
run "${DATA[@]}" -- uninstall -y --purge
home_dump; dump "$SB/data" .
run HOME= USERPROFILE= "TABSMITH_HOME=$SB/data" -- integrations status dotfiles
