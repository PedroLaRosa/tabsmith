#!/bin/bash
# Usage: doctor-diff.sh <tabsmith binary>
# Prints ONE scrubbed transcript of `tabsmith doctor` (and of `diagnostic` / `debug logs`, the other two commands
# Phase 10 moves) over a scripted scenario matrix: stdout, stderr, exit code, every process spawned, and the files
# left behind with their modes. Phase 10 diffs it, the output must be EMPTY:
#   diff <(doctor-diff.sh target/hexagonal-refactor/baseline-tree/target/debug/tabsmith) \
#        <(doctor-diff.sh target/debug/tabsmith)
# Takes about 3.5 minutes (one second boundary per command that may back up a dotfile, three 5 s spawn timeouts).
# Needs: bash, a C compiler (cc), python3, GNU or BSD coreutils. Linux and macOS; run as root or as a user.
#
# How it makes `doctor` deterministic (docs: target/p10/recon-verification.md):
# * `doctor` reads its PARENT process (parent_exe: /proc/<ppid>/exe), so every command runs under a tiny compiled
#   stand-in ("shim", copied to $WORK/parent/{bash,zsh,fish,pwsh,cmd,cmd.exe,python}) that forks the real command and
#   waits: `shim --run-nobody tabsmith doctor`. The same shim, symlinked as bash/zsh/fish/pwsh into a scenario's
#   PATH dir, answers `--version` (FAKE_VERSION[_BASH|_FISH]) and the pwsh `$PROFILE` query (FAKE_PROFILE), sleeps
#   (SHIM_SLEEP_MS[_<NAME>]) and logs every such spawn to $SHIM_LOG (argv exactly as spawned): the "--- spawns"
#   section pins which processes doctor starts, how often and with which arguments.
# * Running as root, doctor refuses without --all. So commands drop to uid 65534 inside the shim (`--run-nobody`,
#   which keeps /proc/<ppid>/exe readable); ROOT=1 runs a command as root for the root-check scenarios.
# * The env is cleared (`env -i`); PATH is the scenario's own dir holding only the fake shells it asks for.
# * The sandbox is always $WORK/sb, the binary always $WORK/bin/tabsmith ($WORK/bin2/tabsmith = "the binary that was
#   moved"), so the data they print is identical for two binaries and scrubbed ($SANDBOX, $BIN, $BIN2, $PARENTS).
# * Times: `.bak-<secs>`, `backups/<secs>`, log timestamps are scrubbed. Every command that may back up a dotfile
#   (brun) starts in a second of its own, early in that second, so two backups never share or straddle a directory.
# * tty paths (colours, the spinner, Ctrl-C) run on a pty from an embedded python driver (prun); the spinner frames are
#   removed from the transcript (their number depends on timing) and counted instead.
# Scenario groups: a fresh HOME, b parents x shells on PATH, c dotfile states, d stale integration scripts, e corrupt
# settings, f specs, g data dir, h terminal env, i session, j settings-driven checks, k system checks, l restart
# reminder, a3 arguments, m root, n no HOME, o logging (-v), p diagnostic, q debug logs, r pty.
export LC_ALL=C
umask 022
[ -n "$1" ] && [ -f "$1" ] || { echo "usage: $0 <tabsmith binary>" >&2; exit 2; }
SRC="$(cd "$(dirname "$1")" && pwd -P)/$(basename "$1")"
command -v cc >/dev/null || { echo "doctor-diff.sh needs a C compiler (cc)" >&2; exit 2; }
IS_ROOT=0; [ "$(id -u)" = 0 ] && IS_ROOT=1
WORK=$(cd "$(mktemp -d /tmp/doctor-diff.XXXXXX)" && pwd -P) || exit 2
chmod 755 "$WORK"
cleanup() { chmod -R u+rwX "$WORK" 2>/dev/null; rm -rf "$WORK"; }
trap cleanup EXIT
trap 'exit 130' INT TERM HUP   # so the EXIT trap also runs when the script is killed
SB="$WORK/sb"
mkdir -p "$WORK/bin" "$WORK/bin2" "$WORK/parent"
cp "$SRC" "$WORK/bin/tabsmith"; cp "$SRC" "$WORK/bin2/tabsmith"; chmod 755 "$WORK/bin" "$WORK/bin2" "$WORK/parent"
BIN="$WORK/bin/tabsmith"; BIN2="$WORK/bin2/tabsmith"
BIN3="$WORK/bin3'q/tabsmith"; mkdir -p "$WORK/bin3'q"; cp "$SRC" "$BIN3"; chmod 755 "$WORK/bin3'q"   # a quote in the path
VERSION=$("$BIN" version | awk '{print $2}')
TERM_OK="TABSMITH_TERM=$VERSION"

cat >"$WORK/shim.c" <<'EOF'
#include <ctype.h>
#include <grp.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/prctl.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>
/* VAR_<BASENAME> if set, else VAR. */
static const char *env_for(const char *stem, const char *base) {
  char name[160]; size_t n = (size_t)snprintf(name, sizeof name, "%s_", stem);
  for (const char *p = base; *p && n < 150; p++) name[n++] = (char)toupper((unsigned char)*p);
  name[n] = 0;
  const char *v = getenv(name);
  return v ? v : getenv(stem);
}
int main(int argc, char **argv) {
  const char *base = strrchr(argv[0], '/'); base = base ? base + 1 : argv[0];
  if (argc >= 3 && (!strcmp(argv[1], "--run") || !strcmp(argv[1], "--run-nobody"))) {
    signal(SIGINT, SIG_IGN);                       /* like a shell waiting for its foreground job */
    if (!strcmp(argv[1], "--run-nobody") && geteuid() == 0) {
      if (setgroups(0, NULL) || setgid(65534) || setuid(65534)) return 126;
      prctl(PR_SET_DUMPABLE, 1);                   /* keep /proc/<this pid>/exe readable by the child */
    }
    pid_t pid = fork();
    if (pid < 0) return 127;
    if (pid == 0) { signal(SIGINT, SIG_DFL); execv(argv[2], argv + 2); _exit(127); }
    int st; waitpid(pid, &st, 0);
    return WIFEXITED(st) ? WEXITSTATUS(st) : 128 + WTERMSIG(st);
  }
  const char *log = getenv("SHIM_LOG");
  if (log) {
    FILE *f = fopen(log, "a");
    if (f) { fputs(argv[0], f); for (int i = 1; i < argc; i++) { fputc(' ', f); fputs(argv[i], f); } fputc('\n', f); fclose(f); }
  }
  const char *ms = env_for("SHIM_SLEEP_MS", base);
  if (ms) { signal(SIGINT, SIG_IGN); struct timespec t = { atoi(ms) / 1000, (atoi(ms) % 1000) * 1000000L }; nanosleep(&t, NULL); }
  if (argc == 2 && !strcmp(argv[1], "--version")) {
    const char *v = env_for("FAKE_VERSION", base);
    if (v) { puts(v); return 0; }
    if (!strcmp(base, "bash")) puts("GNU bash, version 5.2.21(1)-release (x86_64-pc-linux-gnu)");
    else if (!strcmp(base, "fish")) puts("fish, version 3.7.1");
    else puts("version 1.0.0");
    return 0;
  }
  const char *p = getenv("FAKE_PROFILE");              /* the pwsh `$PROFILE` query */
  if (p) { fputs(p, stdout); return 0; }
  return 1;
}
EOF
cc -O1 -o "$WORK/shim" "$WORK/shim.c" || { echo "could not compile the shim" >&2; exit 2; }
for n in bash zsh fish pwsh cmd cmd.exe python; do cp "$WORK/shim" "$WORK/parent/$n"; done
chmod 755 "$WORK/shim"

# ---------------------------------------------------------------------------------------------------------- helpers
scrub() {
  sed -E -e "s#$SB#\$SANDBOX#g" -e "s#$BIN3#\$BIN3#g" -e "s#$BIN2#\$BIN2#g" -e "s#$BIN#\$BIN#g" -e "s#$WORK/parent#\$PARENTS#g" \
      -e "s#$WORK/shim#\$SHIM#g" -e "s#$WORK#\$WORK#g" \
      -e 's#\.bak-[0-9]+#.bak-SECS#g' -e 's#backups/[0-9]+#backups/SECS#g' -e 's/tmp-[0-9]+/tmp-PID/g' \
      -e 's/^[0-9]{13} (Error|Warn|Info|Debug|Trace) /TS \1 /'
}
fmode() { stat -c %a "$1" 2>/dev/null || stat -f %Lp "$1"; }
# show <file>: its scrubbed content, and whether it lacks a final newline
show() { scrub <"$1"; if [ -s "$1" ] && [ -n "$(tail -c 1 "$1")" ]; then echo; echo "(no newline at end)"; fi; }
as_nobody() { [ $IS_ROOT = 1 ]; }
need_root() { [ $IS_ROOT = 1 ] || { echo "(skipped: not running as root)"; return 1; }; }

# fresh <title>: an empty sandbox (HOME = $SB/home), no fake shell on PATH
fresh() {
  chmod -R u+rwX "$SB" 2>/dev/null; rm -rf "$SB"
  mkdir -p "$SB/home" "$SB/pathbin"; chmod 755 "$SB" "$SB/home" "$SB/pathbin"
  : >"$SB/shim.log"; chmod 666 "$SB/shim.log"
  cd "$SB" || exit 1
  echo; echo "======== $1"
}
# paths <shell>...: the fake shells on PATH
paths() { rm -f "$SB"/pathbin/*; local n; for n in "$@"; do ln -s "$WORK/shim" "$SB/pathbin/$n"; done; }

# align: wait until a second of its own, early in it (every brun starts in a different second than the last one)
align() {
  python3 - "$WORK/last_sec" <<'EOF'
import sys, time
f = sys.argv[1]
try: last = int(open(f).read() or 0)
except OSError: last = 0
while True:
    t = time.time(); s = int(t)
    if s != last and t - s < 0.3: break
    time.sleep(0.01)
open(f, "w").write(str(s))
EOF
}

# run [VAR=value ...] -- <args...>: one command, stdin from /dev/null. Variables of the caller: PARENT (the process
# doctor sees as its parent, default bash), ROOT (run as root), TBIN (the binary, default $BIN), MERGE (stdout and
# stderr into one stream, for the interleaving), BACKUP (start in a second of its own).
run() {
  local envs=(); while [ "$1" != "--" ]; do envs+=("$1"); shift; done; shift
  local parent=${PARENT:-bash} mode=--run-nobody who=nobody tbin=${TBIN:-$BIN}
  if [ -n "$ROOT" ]; then mode=--run; who=root; fi
  [ -n "$BACKUP" ] && align
  echo "### parent=$parent user=$who ${TBIN:+tbin=$TBIN }${envs[*]} $*" | scrub
  as_nobody && chown -hR 65534:65534 "$SB"
  : >"$SB/shim.log"; as_nobody && chown 65534:65534 "$SB/shim.log"
  if [ -n "$MERGE" ]; then
    env -i PATH="$SB/pathbin" HOME="$SB/home" TABSMITH_HOME="$SB/home/.tabsmith" USERPROFILE="$SB/home" \
      SHIM_LOG="$SB/shim.log" "${envs[@]}" "$WORK/parent/$parent" $mode "$tbin" "$@" </dev/null >"$SB/out" 2>&1
    local code=$?
    echo "--- merged"; show "$SB/out"
  else
    env -i PATH="$SB/pathbin" HOME="$SB/home" TABSMITH_HOME="$SB/home/.tabsmith" USERPROFILE="$SB/home" \
      SHIM_LOG="$SB/shim.log" "${envs[@]}" "$WORK/parent/$parent" $mode "$tbin" "$@" </dev/null >"$SB/out" 2>"$SB/err"
    local code=$?
    show "$SB/out"; echo "--- stderr"; show "$SB/err"
  fi
  echo "--- exit $code"
  [ -s "$SB/shim.log" ] && { echo "--- spawns"; show "$SB/shim.log"; }
}
brun() { BACKUP=1 run "$@"; }
# prun <where> <sigint-after-spinner 0|1> <expect-spinner 0|1> [VAR=value ...] -- <args...>: on a pty (python driver).
# where: both | stderr-tty (stdout in a file) | stdout-tty (stderr in a file)
prun() {
  local where=$1 sig=$2 spin=$3; shift 3
  local envs=(); while [ "$1" != "--" ]; do envs+=("$1"); shift; done; shift
  local parent=${PARENT:-bash} mode=--run-nobody who=nobody
  if [ -n "$ROOT" ]; then mode=--run; who=root; fi
  align
  echo "### pty($where sigint=$sig) parent=$parent user=$who ${envs[*]} $*" | scrub
  as_nobody && chown -hR 65534:65534 "$SB"
  : >"$SB/shim.log"; as_nobody && chown 65534:65534 "$SB/shim.log"
  python3 "$WORK/ptyrun.py" "$where" "$sig" "$spin" "$SB/pout" "$SB/perr" env -i PATH="$SB/pathbin" HOME="$SB/home" \
    TABSMITH_HOME="$SB/home/.tabsmith" USERPROFILE="$SB/home" SHIM_LOG="$SB/shim.log" "${envs[@]}" \
    "$WORK/parent/$parent" $mode "$BIN" "$@" >"$SB/ptyout"
  local code=$?
  echo "--- pty"; show "$SB/ptyout"
  [ "$where" = stderr-tty ] && { echo "--- stdout file"; show "$SB/pout"; }
  [ "$where" = stdout-tty ] && { echo "--- stderr file"; show "$SB/perr"; }
  echo "--- exit $code"
  [ -s "$SB/shim.log" ] && { echo "--- spawns"; show "$SB/shim.log"; }
}

# dump [dir]: every file (mode, content), directory (mode) and symlink (target) under dir (default HOME), sorted. Spec
# bundles are summarised (file count, mode and checksum of index.json); logs/cli.log has its timestamps scrubbed.
dump() {
  local d=${1:-$SB/home}
  echo "## tree $d" | scrub
  [ -d "$d" ] || { echo "(absent)"; return; }
  (cd "$d" && find . -type f ! -path '*/specs/*' | sort | while IFS= read -r f; do
    echo "# file $f $(fmode "$f")" | scrub
    case "$f" in */logs/cli.log) sed -E 's/^[0-9]{13} /TS /' "$f" | scrub ;; *) show "$f" ;; esac
  done) 2>&1
  (cd "$d" && find . -type d ! -path '*/specs/*/*' | sort | while IFS= read -r x; do echo "# dir $x $(fmode "$x")"; done) 2>&1 | scrub
  (cd "$d" && find . -type l ! -path '*/specs/*' | sort | while IFS= read -r l; do echo "# link $l -> $(readlink "$l")"; done) 2>&1 | scrub
  (cd "$d" && find . -type d -path '*/specs/*' ! -path '*/specs/*/*' | sort | while IFS= read -r v; do
    echo "# specs $v: $(find "$v" -type f | wc -l | tr -d ' ') files, index.json $( [ -f "$v/index.json" ] && echo "$(fmode "$v/index.json") $(cksum <"$v/index.json" | cut -d' ' -f1)" || echo absent)"
  done) 2>&1 | scrub
}
specs_ver() { ls "$SB/home/.tabsmith/specs" 2>/dev/null | head -1; }
# settings <json>: write the data dir's settings.json
settings() { mkdir -p "$SB/home/.tabsmith"; printf '%s\n' "$1" >"$SB/home/.tabsmith/settings.json"; }
# good [shell...]: a healthy installation for the shells (their fake binaries on PATH): setup then a doctor that fixes nothing
good() { paths "$@"; brun -- setup --no-confirm; }
NOHOME=(HOME= TABSMITH_HOME= USERPROFILE=)

cat >"$WORK/ptyrun.py" <<'EOF'
import os, pty, re, select, signal, sys, time
where, sig, spin, pout, perr = sys.argv[1], sys.argv[2] == "1", sys.argv[3] == "1", sys.argv[4], sys.argv[5]
cmd = sys.argv[6:]
pid, fd = pty.fork()
if pid == 0:
    if where == "stderr-tty":
        os.dup2(os.open(pout, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o666), 1)
    if where == "stdout-tty":
        os.dup2(os.open(perr, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o666), 2)
    os.execvp(cmd[0], cmd)
import termios
a = termios.tcgetattr(fd); a[3] &= ~termios.ECHO; termios.tcsetattr(fd, termios.TCSANOW, a)
data, sent, status, deadline = b"", False, None, time.time() + 60
FRAME = re.compile("\x1b\\[\\?25l\r[\u280b\u2819\u2839\u2838\u283c\u2834\u2826\u2827\u2807\u280f] Running checks\\.\\.\\.")
while time.time() < deadline:
    if select.select([fd], [], [], 0.02)[0]:
        try: chunk = os.read(fd, 65536)
        except OSError: chunk = b""
        if chunk: data += chunk
    if sig and not sent and data.decode("utf-8", "replace").count("Running checks") >= 2:
        os.write(fd, b"\x03"); sent = True
    w, st = os.waitpid(pid, os.WNOHANG)
    if w: status = st; break
if status is None:
    os.kill(pid, 9); _, status = os.waitpid(pid, 0); data += b"<<TIMEOUT>>"
try:
    while select.select([fd], [], [], 0.1)[0]:
        chunk = os.read(fd, 65536)
        if not chunk: break
        data += chunk
except OSError: pass
text = data.decode("utf-8", "replace").replace("\r\n", "\n")
frames = len(FRAME.findall(text))
text = FRAME.sub("", text)
text = text.replace("\x1b", "\\e").replace("\r", "\\r")
sys.stdout.write(text)
if not text.endswith("\n"): sys.stdout.write("\n(no newline at end)\n")
if spin: sys.stdout.write("(spinner frames >= 3: %s)\n" % ("yes" if frames >= 3 else "NO, %d" % frames))
sys.stdout.flush()
code = os.waitstatus_to_exitcode(status)
sys.exit(code if code >= 0 else 128 - code)
EOF

echo "# doctor-diff: tabsmith $VERSION, as root: $IS_ROOT"

# =================================================================================================== a. fresh HOME
fresh "a. a fresh HOME, bash is the parent and the only shell on PATH"
paths bash
run -- doctor --all
brun -- doctor
brun -- doctor
brun "$TERM_OK" -- doctor
run "$TERM_OK" -- doctor --all
run "$TERM_OK" -- doctor --strict
dump

fresh "a2. a fresh HOME with the terminal variable set: every fix, then Everything looks good, restart reminder"
paths bash
brun "$TERM_OK" -- doctor
brun "$TERM_OK" -- doctor
dump

# ================================================================================ b. parents x shells on PATH
fresh "b1. zsh is the parent, bash zsh fish on PATH (zsh Normal, bash and fish Soft)"
paths bash zsh fish
PARENT=zsh brun "$TERM_OK" -- doctor
PARENT=zsh brun "$TERM_OK" -- doctor --strict
PARENT=zsh run "$TERM_OK" -- doctor --all
dump

fresh "b2. fish is the parent and the only shell on PATH"
paths fish
PARENT=fish brun "$TERM_OK" -- doctor
PARENT=fish run "$TERM_OK" -- doctor --all
dump

fresh "b3. pwsh is the parent and on PATH (its \$PROFILE under HOME)"
paths pwsh
PARENT=pwsh brun "FAKE_PROFILE=$SB/home/Documents/PowerShell/Microsoft.PowerShell_profile.ps1" "$TERM_OK" -- doctor
PARENT=pwsh run "FAKE_PROFILE=$SB/home/Documents/PowerShell/Microsoft.PowerShell_profile.ps1" "$TERM_OK" -- doctor --all
dump

fresh "b4. an unknown parent (python): every shell on PATH is Soft, none is Normal"
paths bash zsh fish
PARENT=python brun "$TERM_OK" -- doctor
PARENT=python brun "$TERM_OK" -- doctor --strict
dump

fresh "b5. zsh is the parent but not on PATH (Normal anyway); bash on PATH (Soft); no fish, no pwsh (no check)"
paths bash
PARENT=zsh brun "$TERM_OK" -- doctor
PARENT=zsh run "$TERM_OK" -- doctor --all
dump

fresh "b6. no shell on PATH, and no PATH at all"
PARENT=python run "$TERM_OK" -- doctor --all
PARENT=python run "$TERM_OK" PATH= -- doctor --all
PARENT=python brun "$TERM_OK" -- doctor
dump

fresh "b7. pwsh on PATH, parent python: pwsh \$PROFILE failures"
paths pwsh
PARENT=python run "$TERM_OK" -- doctor --all
PARENT=python run "$TERM_OK" FAKE_PROFILE= -- doctor --all
PARENT=python run "$TERM_OK" "FAKE_PROFILE=  $SB/home/p.ps1  " -- doctor --all
PARENT=python brun "$TERM_OK" "FAKE_PROFILE=$SB/home/p.ps1" -- doctor --strict
dump

fresh "b8. pwsh on PATH, \$PROFILE query times out after 5 s"
paths pwsh
PARENT=python run "$TERM_OK" SHIM_SLEEP_MS_PWSH=6000 "FAKE_PROFILE=$SB/home/p.ps1" -- doctor --all

fresh "b9. pwsh is the parent but not on PATH (Normal: the \$PROFILE lookup error is the check's reason, no fix, abort)"
paths bash
PARENT=pwsh run "$TERM_OK" -- doctor
PARENT=pwsh run "$TERM_OK" -- doctor --all
PARENT=pwsh run "$TERM_OK" -- doctor --strict

# ======================================================================================== c. dotfile states
fresh "c1. bash: a missing, an unreadable, a read-only, a directory dotfile; a deleted one after an install"
good bash
run "$TERM_OK" -- doctor
rm "$SB/home/.profile"
brun "$TERM_OK" -- doctor
chmod 000 "$SB/home/.bashrc"
brun "$TERM_OK" -- doctor
chmod 000 "$SB/home/.bashrc"
run "$TERM_OK" -- doctor --all
chmod 444 "$SB/home/.bashrc"
run "$TERM_OK" -- doctor
rm "$SB/home/.bashrc"; mkdir "$SB/home/.bashrc"
brun "$TERM_OK" -- doctor
dump

fresh "c2. bash: an existing dotfile without tabsmith, with only the pre line, with the lines reversed, opted out"
good bash
printf 'export A=1\n' >"$SB/home/.bashrc"
brun "$TERM_OK" -- doctor
grep -v 'post' "$SB/home/.bashrc" >"$SB/x"; cp "$SB/x" "$SB/home/.bashrc"
brun "$TERM_OK" -- doctor
printf '%s\n' "$(tail -1 "$SB/home/.profile")" "$(head -1 "$SB/home/.profile")" >"$SB/home/.profile"
brun "$TERM_OK" -- doctor
printf '  #  TABSMITH ignore\n' >"$SB/home/.bashrc"
brun "$TERM_OK" -- doctor
run "$TERM_OK" -- doctor --all
dump

fresh "c3. bash: the integration uninstalled, a script file edited"
good bash
brun -- integrations uninstall dotfiles bash
brun "$TERM_OK" -- doctor
printf 'echo hi\n' >"$SB/home/.tabsmith/shell/bashrc.pre.bash"
brun "$TERM_OK" -- doctor
dump

fresh "c4. zsh: ZDOTDIR"
good zsh
mkdir -p "$SB/zdot"
PARENT=zsh brun "ZDOTDIR=$SB/zdot" "$TERM_OK" -- doctor
PARENT=zsh run "ZDOTDIR=$SB/zdot" "$TERM_OK" -- doctor --all
dump; dump "$SB/zdot"

fresh "c5. fish: XDG_CONFIG_HOME, a deleted conf.d file"
good fish
mkdir -p "$SB/xdg"
PARENT=fish brun "XDG_CONFIG_HOME=$SB/xdg" "$TERM_OK" -- doctor
rm "$SB/xdg/fish/conf.d/99_tabsmith_post.fish"
PARENT=fish brun "XDG_CONFIG_HOME=$SB/xdg" "$TERM_OK" -- doctor
dump; dump "$SB/xdg"

fresh "c6. pwsh: a profile that exists without tabsmith, then installed"
paths pwsh
export_profile="FAKE_PROFILE=$SB/home/prof.ps1"
printf 'Write-Host hi\n' >"$SB/home/prof.ps1"
brun "$export_profile" -- setup --no-confirm
PARENT=pwsh brun "$export_profile" "$TERM_OK" -- doctor
PARENT=pwsh run "$export_profile" "$TERM_OK" -- doctor --all
dump

# ========================================================================================== d. stale scripts
fresh "d1. every script points at a moved binary, parent python (dotfile checks are Soft, the stale-binary check fixes)"
paths bash zsh fish
TBIN=$BIN2 brun -- integrations install --silent dotfiles
PARENT=python run "$TERM_OK" -- doctor --all
PARENT=python brun "$TERM_OK" -- doctor
PARENT=python brun "$TERM_OK" -- doctor
dump

fresh "d2. the same with the parent bash: the dotfile check of the current shell fixes it first"
paths bash zsh fish
TBIN=$BIN2 brun -- integrations install --silent dotfiles
brun "$TERM_OK" -- doctor
dump

fresh "d3. stale scripts, --strict (the Soft dotfile errors are fixed first)"
paths bash zsh fish
TBIN=$BIN2 brun -- integrations install --silent dotfiles
PARENT=python brun "$TERM_OK" -- doctor --strict
dump

fresh "d4. stale scripts of zsh only, a deleted .zshrc, a script file without quotes, an unreadable one"
paths bash zsh fish
brun -- integrations install --silent dotfiles
TBIN=$BIN2 brun -- integrations install --silent dotfiles zsh
rm "$SB/home/.zshrc"
printf 'no quotes here\n' >"$SB/home/.tabsmith/shell/zprofile.post.zsh"
PARENT=python brun "$TERM_OK" -- doctor
printf "x '/other/tabsmith' y\n" >"$SB/home/.config/fish/conf.d/00_tabsmith_pre.fish"
PARENT=python run "$TERM_OK" -- doctor --all
PARENT=python brun "$TERM_OK" -- doctor
dump

fresh "d5. stale scripts, the fix fails (a directory where a dotfile must be written)"
paths bash zsh
TBIN=$BIN2 brun -- integrations install --silent dotfiles
rm "$SB/home/.zprofile"; mkdir "$SB/home/.zprofile"
PARENT=python brun "$TERM_OK" -- doctor
dump

fresh "d6. stale scripts with pwsh on PATH (no second pwsh spawn for the script files)"
paths bash pwsh
PROF="FAKE_PROFILE=$SB/home/prof.ps1"
TBIN=$BIN2 brun "$PROF" -- integrations install --silent dotfiles
PARENT=python run "$PROF" "$TERM_OK" -- doctor --all
PARENT=python brun "$PROF" "$TERM_OK" -- doctor
dump

fresh "d7. a single quote in the binary path (the script's first quoted text is cut at the quote)"
paths bash zsh
TBIN=$BIN3 brun -- integrations install --silent dotfiles
TBIN=$BIN3 brun "$TERM_OK" -- doctor
TBIN=$BIN3 run "$TERM_OK" -- doctor --all
PARENT=python TBIN=$BIN3 brun "$TERM_OK" -- doctor
brun "$TERM_OK" -- doctor
dump

# ===================================================================================== e. corrupt settings
corrupt() { # corrupt <title> <how: shell snippet that writes $SB/home/.tabsmith/settings.json>
  fresh "$1"; good bash; eval "$2"
}
corrupt "e1. settings.json is {oops: --all warns in the log, then the fix backs it up and writes {}" \
  "printf '{oops' >\$SB/home/.tabsmith/settings.json"
run "$TERM_OK" -- doctor --all
brun "$TERM_OK" -- doctor
run "$TERM_OK" -- doctor --all
dump
corrupt "e2. settings.json is [1]" "printf '[1]' >\$SB/home/.tabsmith/settings.json"
brun "$TERM_OK" -- doctor
dump
corrupt "e3. settings.json is empty, then only whitespace (healthy)" ": >\$SB/home/.tabsmith/settings.json"
brun "$TERM_OK" -- doctor
printf '  \n\n' >"$SB/home/.tabsmith/settings.json"
brun "$TERM_OK" -- doctor
dump
corrupt "e4. settings.json is a directory" "rm \$SB/home/.tabsmith/settings.json 2>/dev/null; mkdir \$SB/home/.tabsmith/settings.json"
brun "$TERM_OK" -- doctor
dump
corrupt "e5. settings.json is corrupt and mode 0600; then unreadable (0000)" \
  "printf '{oops' >\$SB/home/.tabsmith/settings.json; chmod 600 \$SB/home/.tabsmith/settings.json"
brun "$TERM_OK" -- doctor
printf '{oops' >"$SB/home/.tabsmith/settings.json"; chmod 000 "$SB/home/.tabsmith/settings.json"
brun "$TERM_OK" -- doctor
dump
corrupt "e6. settings.json is a symlink to a corrupt file elsewhere" \
  "mkdir -p \$SB/elsewhere; printf '{oops' >\$SB/elsewhere/real.json; rm -f \$SB/home/.tabsmith/settings.json; ln -s \$SB/elsewhere/real.json \$SB/home/.tabsmith/settings.json"
brun "$TERM_OK" -- doctor
dump; dump "$SB/elsewhere"
corrupt "e7. TABSMITH_SETTINGS_PATH: corrupt override with a healthy settings.json; extensions replaced in the backup name" \
  "mkdir -p \$SB/over; printf '{oops' >\$SB/over/s.conf; printf '{oops' >\$SB/over/noext"
brun "TABSMITH_SETTINGS_PATH=$SB/over/s.conf" "$TERM_OK" -- doctor
brun "TABSMITH_SETTINGS_PATH=$SB/over/noext" "$TERM_OK" -- doctor
brun "TABSMITH_SETTINGS_PATH=$SB/over/new/s.json" "$TERM_OK" -- doctor
printf '{oops' >"$SB/home/.tabsmith/settings.json"
run "TABSMITH_SETTINGS_PATH=$SB/over/s.conf" "$TERM_OK" -- doctor --all
dump; dump "$SB/over"
corrupt "e8. settings.json is corrupt and the data dir is read-only (the fix cannot write the backup)" \
  "printf '{oops' >\$SB/home/.tabsmith/settings.json; chmod 555 \$SB/home/.tabsmith"
brun "$TERM_OK" -- doctor
chmod 755 "$SB/home/.tabsmith"
dump

# ============================================================================================== f. specs
fresh "f1. specs missing: the fix extracts them; an index.json missing; a read-only specs dir"
good bash
V=$(specs_ver)
rm -rf "$SB/home/.tabsmith/specs"
brun "$TERM_OK" -- doctor
rm "$SB/home/.tabsmith/specs/$V/index.json"
brun "$TERM_OK" -- doctor
rm -rf "$SB/home/.tabsmith/specs"; mkdir "$SB/home/.tabsmith/specs"; chmod 555 "$SB/home/.tabsmith/specs"
brun "$TERM_OK" -- doctor
chmod 755 "$SB/home/.tabsmith/specs"
dump

fresh "f2. specs is a file; specs/<version> is a file"
good bash
V=$(specs_ver)
rm -rf "$SB/home/.tabsmith/specs"; printf 'x' >"$SB/home/.tabsmith/specs"
brun "$TERM_OK" -- doctor
rm "$SB/home/.tabsmith/specs"; mkdir "$SB/home/.tabsmith/specs"; printf 'x' >"$SB/home/.tabsmith/specs/$V"
brun "$TERM_OK" -- doctor
dump

fresh "f3. the data root is outside HOME and the settings elsewhere: the specs fix extracts under TABSMITH_HOME"
paths bash
D="TABSMITH_HOME=$SB/data"
brun "$D" "TABSMITH_SETTINGS_PATH=$SB/over/s.json" "$TERM_OK" -- doctor
brun "$D" "TABSMITH_SETTINGS_PATH=$SB/over/s.json" "$TERM_OK" -- doctor
rm -rf "$SB/data/specs"
brun "$D" "TABSMITH_SETTINGS_PATH=$SB/over/s.json" "$TERM_OK" -- doctor
dump; dump "$SB/data"; dump "$SB/over"

# ========================================================================================== g. data directory
fresh "g1. the data dir is missing: a nested TABSMITH_HOME is created; a file; a dangling symlink; HOME/.tabsmith a file"
paths bash
brun "TABSMITH_HOME=$SB/deep/a/b" "$TERM_OK" -- doctor
printf 'x' >"$SB/afile"
brun "TABSMITH_HOME=$SB/afile" "$TERM_OK" -- doctor
ln -s "$SB/nowhere" "$SB/dangling"
brun "TABSMITH_HOME=$SB/dangling" "$TERM_OK" -- doctor
printf 'x' >"$SB/home/.tabsmith"
brun "$TERM_OK" -- doctor
run "$TERM_OK" -- doctor --all
dump; dump "$SB/deep"; dump "$SB/dangling"

fresh "g3. data roots that are relative, end in a slash, have .. or spaces (printed exactly as given)"
paths bash
brun "TABSMITH_HOME=reldata" "$TERM_OK" -- doctor
brun "TABSMITH_HOME=$SB/trail/" "$TERM_OK" -- doctor
brun "TABSMITH_HOME=$SB/with space/../dd" "$TERM_OK" -- doctor
run "TABSMITH_HOME=$SB/trail/" "$TERM_OK" -- doctor --all
brun HOME=relhome TABSMITH_HOME= "$TERM_OK" -- doctor
run HOME=relhome TABSMITH_HOME= "$TERM_OK" -- doctor --all
dump; dump "$SB/reldata"; dump "$SB/trail"; dump "$SB/dd"; dump "$SB/relhome"

fresh "g2. the data dir is missing, umask 077 (mode of the created directories)"
paths bash
(umask 077; brun "$TERM_OK" -- doctor)
dump

# ====================================================================================== h. terminal env
fresh "h. terminal integrations check: every variable, its order"
good bash
for assign in "TABSMITH_TERM=" "TABSMITH_TERM=0.0.1" "TABSMITH_TERM=$VERSION" "TABSMITH_TERM=${VERSION}x" \
  "TERM_PROGRAM=WarpTerminal" "TERM_PROGRAM=Apple_Terminal" "INSIDE_EMACS=" "INSIDE_EMACS=29.1,comint" \
  "__CFBundleIdentifier=com.vandyke.SecureCRT" "__CFBundleIdentifier=com.apple.Terminal" \
  "PROCESS_LAUNCHED_BY_TABSMITH=1" "PROCESS_LAUNCHED_BY_TABSMITH=" "CI=1" "CI=" "CI=false"; do
  run "$assign" -- doctor --all
  run "$assign" "$TERM_OK" -- doctor --all
done
run "TERM_PROGRAM=WarpTerminal" INSIDE_EMACS=1 CI=1 "$TERM_OK" -- doctor --all
run INSIDE_EMACS=1 CI=1 "$TERM_OK" -- doctor --all
run INSIDE_EMACS=1 __CFBundleIdentifier=com.vandyke.SecureCRT "$TERM_OK" -- doctor --all
run __CFBundleIdentifier=com.vandyke.SecureCRT PROCESS_LAUNCHED_BY_TABSMITH=1 CI=1 "$TERM_OK" -- doctor --all
run PROCESS_LAUNCHED_BY_TABSMITH=1 CI=1 "$TERM_OK" -- doctor --all
run CI=1 -- doctor --all
run "TERM_PROGRAM=WarpTerminal" -- doctor
run INSIDE_EMACS=1 -- doctor
run CI=1 -- doctor
run -- doctor
run -- doctor --strict
PARENT=cmd run "$TERM_OK" -- doctor --all
PARENT=cmd.exe run "$TERM_OK" -- doctor --all
PARENT=cmd.exe run "TERM_PROGRAM=WarpTerminal" "$TERM_OK" -- doctor --all
PARENT=cmd.exe run INSIDE_EMACS=1 "$TERM_OK" -- doctor --all
PARENT=cmd.exe run -- doctor
PARENT=python run "$TERM_OK" -- doctor
run "TABSMITH_TERM=$(printf 'bad\377')" -- doctor --all
run "TERM_PROGRAM=$(printf 'Warp\377')" "$TERM_OK" -- doctor --all
dump

# ================================================================================================ i. session
fresh "i. session check: severity and the run file"
good bash zsh
run "$TERM_OK" -- doctor --all
run "$TERM_OK" TABSMITH_SESSION_ID= -- doctor --all
run "$TERM_OK" TABSMITH_SESSION_ID=abc -- doctor --all
run "$TERM_OK" TABSMITH_SESSION_ID=abc -- doctor
run "$TERM_OK" TABSMITH_SESSION_ID=abc -- doctor --strict
mkdir -p "$SB/home/.tabsmith/run"; printf '{}' >"$SB/home/.tabsmith/run/abc.json"
run "$TERM_OK" TABSMITH_SESSION_ID=abc -- doctor --all
run "$TERM_OK" TABSMITH_SESSION_ID=abc -- doctor
mkdir "$SB/home/.tabsmith/run/dir.json"
run "$TERM_OK" TABSMITH_SESSION_ID=dir -- doctor --all
run "$TERM_OK" TABSMITH_SESSION_ID=../abc -- doctor --all
printf '{}' >"$SB/home/.tabsmith/abc.json"
run "$TERM_OK" TABSMITH_SESSION_ID=../abc -- doctor --all
run "$TERM_OK" TABSMITH_SESSION_ID=a/b -- doctor --all
PARENT=python run "$TERM_OK" TABSMITH_SESSION_ID=abc -- doctor --all
PARENT=pwsh run "$TERM_OK" TABSMITH_SESSION_ID=abc -- doctor --all
PARENT=python run "$TERM_OK" TABSMITH_SESSION_ID=abc TABSMITH_SHELL=pwsh -- doctor --all
PARENT=python run "$TERM_OK" TABSMITH_SESSION_ID=abc TABSMITH_SHELL=/usr/bin/zsh -- doctor --all
PARENT=zsh run "$TERM_OK" TABSMITH_SESSION_ID=abc TABSMITH_SHELL=pwsh -- doctor --all
PARENT=pwsh run "$TERM_OK" TABSMITH_SESSION_ID=abc TABSMITH_SHELL=zsh -- doctor --all
PARENT=python run "$TERM_OK" TABSMITH_SESSION_ID=abc TABSMITH_SHELL=fish -- doctor --all
PARENT=python run "$TERM_OK" TABSMITH_SESSION_ID=abc TABSMITH_SHELL=nope -- doctor --all
run "TABSMITH_SESSION_ID=$(printf 'a\377')" "$TERM_OK" -- doctor --all
dump

# ======================================================================================== j. settings checks
fresh "j. settings-driven checks (healthy installation, settings.json rewritten per case)"
good bash
case_() { # case_ <settings json> [VAR=value ...] -- <doctor args>: rewrite settings.json, run doctor
  local j=$1; shift; settings "$j"; run "$TERM_OK" "$@"
}
case_ '{}' -- doctor --all
case_ '{"autocomplete.disable": true}' -- doctor --all
case_ '{"autocomplete.disable": true}' -- doctor
case_ '{"autocomplete.disable": "true"}' -- doctor --all
case_ '{"autocomplete.disable": 1}' -- doctor --all
case_ '{"autocomplete.disable": false, "autocomplete.developerMode": true}' -- doctor --all
case_ '{"autocomplete.developerMode": true}' -- doctor
case_ '{"autocomplete.developerMode": true}' -- doctor --strict
case_ '{"autocomplete.developerMode": "yes"}' -- doctor --strict
case_ '{"autocomplete.disable": true, "autocomplete.developerMode": true}' -- doctor --all
case_ '{"inline.enabled": false}' -- doctor --all
case_ '{"inline.enabled": false}' -- doctor
case_ '{"inline.enabled": false}' -- doctor --strict
case_ '{"inline.enabled": "false"}' -- doctor --all
case_ '{"inline.enabled": 0}' -- doctor --all
case_ '{"inline.enabled": false, "inline.source": "nope"}' -- doctor --all
case_ '{"inline.source": "nope"}' -- doctor --all
case_ '{"inline.source": "nope"}' -- doctor
case_ '{"inline.source": "nope"}' -- doctor --strict
case_ '{"inline.source": 123}' -- doctor --all
case_ '{"inline.source": ""}' -- doctor --all
case_ '{"inline.source": "history"}' -- doctor --all
case_ '{"inline.source": "specs"}' -- doctor --all
case_ '{"inline.source": "history+specs"}' -- doctor --all
P='"inline.profiles.p": {"provider": "openai", "model": "m"}'
case_ "{\"inline.source\": \"p\", $P}" -- doctor --all
case_ "{\"inline.source\": \"p\", $P}" -- doctor
case_ "{\"inline.source\": \"p\", $P}" OPENAI_API_KEY=sk -- doctor --all
case_ "{\"inline.source\": \"p\", $P}" OPENAI_API_KEY= -- doctor --all
case_ "{\"inline.source\": \"p\", $P}" ANTHROPIC_API_KEY=sk -- doctor --all
case_ '{"inline.source": "p", "inline.profiles.p": {"provider": "anthropic", "model": "m"}}' -- doctor --all
case_ '{"inline.source": "p", "inline.profiles.p": {"provider": "anthropic", "model": "m"}}' ANTHROPIC_API_KEY=sk -- doctor --all
case_ '{"inline.source": "p", "inline.profiles.p": {"provider": "openai", "model": "m", "api_key_env": "MY_KEY"}}' -- doctor --all
case_ '{"inline.source": "p", "inline.profiles.p": {"provider": "openai", "model": "m", "api_key_env": "MY_KEY"}}' MY_KEY=1 -- doctor --all
for url in http://localhost:11434/v1 http://127.0.0.1:8080 'http://[::1]:8080/v1' https://example.com/localhost \
  'https://evil.example/x?u=http://localhost' 'localhost:8080' 'HTTP://LOCALHOST:1' http://localhost.example.com; do
  case_ "{\"inline.source\": \"p\", \"inline.profiles.p\": {\"provider\": \"openai\", \"model\": \"m\", \"base_url\": \"$url\"}}" -- doctor --all
done
case_ '{"inline.source": "p", "inline.profiles.p": {"provider": "gemini", "model": "m"}}' -- doctor --all
case_ '{"inline.source": "p", "inline.profiles.p": {"provider": "openai"}}' -- doctor --all
case_ '{"inline.source": "p", "inline.profiles.p": "openai"}' -- doctor --all
case_ '{"inline.source": "p", "inline.profiles.p": {"provider": "openai", "model": "m", "api_key_env": "NONE"}}' -- doctor --all
case_ '{"inline.source": "it'"'"'s", "inline.profiles.p": {}}' -- doctor --all
case_ '{"inline.source": "p", "inline.profiles.p": {"provider": "openai", "model": "m"}, "autocomplete.disable": true}' -- doctor
case_ '{"inline.source": "p"' -- doctor --all
settings '{}'
run "$TERM_OK" TABSMITH_SETTINGS_PATH="$SB/over.json" -- doctor --all
printf '{"autocomplete.disable": true, "inline.enabled": false}\n' >"$SB/over.json"
run "$TERM_OK" TABSMITH_SETTINGS_PATH="$SB/over.json" -- doctor --all
run "$TERM_OK" TABSMITH_SETTINGS_PATH="$SB/over.json" -- doctor
dump

# ======================================================================================== k. system checks
fresh "k1. bash version (bash is the parent: its path is probed, not PATH)"
good bash
for v in "GNU bash, version 3.2.57(1)-release (arm64-apple-darwin24)" "GNU bash, version 4.3.48(1)-release" \
  "GNU bash, version 4.4.0(1)-release" "GNU bash, version 4.4(1)" "GNU bash 4" "GNU bash, version 5.2.21(1)-release" \
  "bash" "" "version 3." "version 99999999999.1" "3.9 4.9" "GNU bash, version 4.10.1" "  version 4.3.9  "; do
  run "$TERM_OK" "FAKE_VERSION_BASH=$v" -- doctor
  run "$TERM_OK" "FAKE_VERSION_BASH=$v" -- doctor --all
done
run "$TERM_OK" "FAKE_VERSION_BASH=GNU bash, version 3.2.57(1)-release" -- doctor --strict
run "$TERM_OK" "FAKE_VERSION_BASH=GNU bash, version 3.2.57(1)-release" -- doctor --all --strict
PARENT=zsh run "$TERM_OK" "FAKE_VERSION_BASH=GNU bash, version 3.2.57(1)-release" -- doctor --all
PARENT=python run "$TERM_OK" "FAKE_VERSION_BASH=GNU bash, version 3.2.57(1)-release" -- doctor --all
run "$TERM_OK" SHIM_SLEEP_MS_BASH=6000 "FAKE_VERSION_BASH=GNU bash, version 3.2.57(1)-release" -- doctor --all

fresh "k2. fish version (fish from PATH; Normal; an error aborts the group but not the exit code)"
good bash fish
for v in "fish, version 3.2.2" "fish, version 3.3" "fish, version 3.3.0" "fish, version 3.3.1" "fish, version 3.10.0" \
  "fish, version 4.0.0" "fish, version 4" "fish" "fish, version 2.7.1" "fish, version 3.7.1-12-gabcdef"; do
  run "$TERM_OK" "FAKE_VERSION_FISH=$v" -- doctor
  run "$TERM_OK" "FAKE_VERSION_FISH=$v" -- doctor --all
done
run "$TERM_OK" "FAKE_VERSION_FISH=fish, version 3.2.2" -- doctor --strict
PARENT=fish run "$TERM_OK" "FAKE_VERSION_FISH=fish, version 3.2.2" -- doctor --all
PARENT=zsh run "$TERM_OK" "FAKE_VERSION_FISH=fish, version 3.2.2" -- doctor
run "$TERM_OK" SHIM_SLEEP_MS_FISH=6000 -- doctor --all

fresh "k3. old bash and old fish together, --strict: the Soft bash check is an error and aborts, fish never runs"
good bash fish
run "$TERM_OK" "FAKE_VERSION=GNU bash, version 3.2.57(1)" -- doctor
run "$TERM_OK" "FAKE_VERSION=GNU bash, version 3.2.57(1)" -- doctor --strict
run "$TERM_OK" "FAKE_VERSION=GNU bash, version 3.2.57(1)" -- doctor --all --strict
dump

# ============================================================================================ l. restart reminder
fresh "l1. the reminder is printed once, other state keys are kept"
good bash
ST="$SB/home/.tabsmith/state.json"
printf '{"doctor.prompt-restart-terminal": true, "zzz": 1, "aaa": {"b": [1, 2]}}' >"$ST"
run "$TERM_OK" -- doctor
run "$TERM_OK" -- doctor
dump
printf '{"doctor.prompt-restart-terminal": true}\n' >"$ST"; chmod 644 "$ST"
run "$TERM_OK" -- doctor --all
cat "$ST"; fmode "$ST"

fresh "l2. the reminder after a failed run, after a strict run, with the wrong types"
good bash
ST="$SB/home/.tabsmith/state.json"
printf '{"doctor.prompt-restart-terminal": true}' >"$ST"
run -- doctor
printf '{"doctor.prompt-restart-terminal": true}' >"$ST"
run "$TERM_OK" -- doctor --strict
for v in '"true"' 1 null '[true]' false; do
  printf '{"doctor.prompt-restart-terminal": %s}' "$v" >"$ST"
  run "$TERM_OK" -- doctor
  cat "$ST"; echo
done

fresh "l3. a corrupt, empty, missing or directory state.json: silently skipped"
good bash
ST="$SB/home/.tabsmith/state.json"
printf '{oops' >"$ST"; run "$TERM_OK" -- doctor; cat "$ST"; echo
printf '[true]' >"$ST"; run "$TERM_OK" -- doctor; cat "$ST"; echo
: >"$ST"; run "$TERM_OK" -- doctor; cat "$ST"; echo
rm "$ST"; run "$TERM_OK" -- doctor; [ -e "$ST" ] && echo "state.json created" || echo "state.json still absent"
mkdir "$ST"; run "$TERM_OK" -- doctor
dump

fresh "l4. the data dir is read-only: the reminder is printed and not cleared (the save error is ignored)"
good bash
ST="$SB/home/.tabsmith/state.json"
printf '{"doctor.prompt-restart-terminal": true}' >"$ST"
chmod 555 "$SB/home/.tabsmith"
run "$TERM_OK" -- doctor
run "$TERM_OK" -- doctor
cat "$ST"; echo
chmod 755 "$SB/home/.tabsmith"
dump

fresh "l5. the fixes mark the reminder: printed at the end of the same run, once; the stale-binary and the settings fix too"
paths bash zsh
TBIN=$BIN2 brun -- integrations install --silent dotfiles
PARENT=python brun "$TERM_OK" -- doctor
printf '{"doctor.prompt-restart-terminal": false}' >"$SB/home/.tabsmith/state.json"
printf '{oops' >"$SB/home/.tabsmith/settings.json"
brun "$TERM_OK" -- doctor
dump

fresh "l6. the state file is outside the root when only the settings are overridden (state stays under the root)"
good bash
printf '{"doctor.prompt-restart-terminal": true}' >"$SB/home/.tabsmith/state.json"
run "$TERM_OK" "TABSMITH_SETTINGS_PATH=$SB/over.json" -- doctor
dump

fresh "a3. arguments: short flags, unknown flags, extra arguments"
paths bash
run "$TERM_OK" -- doctor -a
run "$TERM_OK" -- doctor -s
run "$TERM_OK" -- doctor -as
run "$TERM_OK" -- doctor --all --strict
run "$TERM_OK" -- doctor --bogus
run "$TERM_OK" -- doctor extra
run "$TERM_OK" -- doctor --all=1
run "$TERM_OK" -- doctor --help
run "$TERM_OK" -- doctor -h
dump

# =================================================================================================== m. root
fresh "m. running as root"
if need_root; then
  good bash
  ROOT=1 run "$TERM_OK" -- doctor
  ROOT=1 run "$TERM_OK" -- doctor --strict
  ROOT=1 run -- doctor
  ROOT=1 run "$TERM_OK" -- doctor --all
  ROOT=1 MERGE=1 run "$TERM_OK" -- doctor --all
  ROOT=1 MERGE=1 run "$TERM_OK" -- doctor
  ROOT=1 run "${NOHOME[@]}" -- doctor
  ROOT=1 run "${NOHOME[@]}" -- doctor --all
  ROOT=1 MERGE=1 run "${NOHOME[@]}" -- doctor --all
  ROOT=1 run "TABSMITH_HOME=$SB/rootdata" "$TERM_OK" -- doctor --all
  printf '{"doctor.prompt-restart-terminal": true}' >"$SB/home/.tabsmith/state.json"
  ROOT=1 run "$TERM_OK" -- doctor --all
  ROOT=1 run "$TERM_OK" -- doctor
  cat "$SB/home/.tabsmith/state.json"; echo
  dump
fi

# ============================================================================================== n. no HOME
fresh "n. no HOME (non-root)"
paths bash
run "${NOHOME[@]}" -- doctor
run "${NOHOME[@]}" -- doctor --all
run "${NOHOME[@]}" "$TERM_OK" -- doctor --strict
run HOME= USERPROFILE= "TABSMITH_HOME=$SB/data" "$TERM_OK" -- doctor
run HOME= USERPROFILE= "TABSMITH_HOME=$SB/data" "$TERM_OK" -- doctor
run "${NOHOME[@]}" "TABSMITH_SETTINGS_PATH=$SB/over.json" -- doctor --all
run HOME= -- doctor --all
run HOME="$SB/home" TABSMITH_HOME= -- doctor
dump

# =========================================================================================== o. logging (-v)
fresh "o. -v: the logger's records and the doctor output on one stream (order of the writes)"
good bash
printf '{oops' >"$SB/home/.tabsmith/settings.json"
MERGE=1 run "$TERM_OK" -- -v doctor --all
MERGE=1 run "$TERM_OK" -- -vvv doctor --all
MERGE=1 run "$TERM_OK" -- -vvvv doctor
MERGE=1 run "$TERM_OK" -- -vv doctor
MERGE=1 run "$TERM_OK" TABSMITH_LOG_LEVEL=trace -- doctor --all
dump

# ============================================================================================== p. diagnostic
fresh "p. diagnostic: every field and every env-driven branch"
paths bash
DENV=("PATH=$SB/pathbin:/usr/sbin" "TABSMITH_HOME=$SB/home/.tabsmith")
run "${DENV[@]}" -- diagnostic
run "${DENV[@]}" -- diagnostic --format json
run "${DENV[@]}" -- diagnostic -f json-pretty
run "${DENV[@]}" -- diagnostic --format toml
run "${DENV[@]}" -- diagnostic --format yaml
run "SHELL=$WORK/parent/bash" "${DENV[@]}" -- diagnostic
run "SHELL=$WORK/parent/bash" FAKE_VERSION_BASH=$'first line\nsecond line' "${DENV[@]}" -- diagnostic --format json
run "SHELL=$WORK/parent/bash" 'FAKE_VERSION_BASH=  padded  ' "${DENV[@]}" -- diagnostic --format json
run "SHELL=$WORK/parent/bash" FAKE_VERSION_BASH= "${DENV[@]}" -- diagnostic --format json
run "SHELL=$WORK/parent/bash" FAKE_VERSION_BASH= "${DENV[@]}" -- diagnostic
run "SHELL=$WORK/parent/fish" "${DENV[@]}" -- diagnostic
run "SHELL=$WORK/nonexistent" "${DENV[@]}" -- diagnostic
run "SHELL=$SB" "${DENV[@]}" -- diagnostic
run "SHELL=" "${DENV[@]}" -- diagnostic
run "SHELL=/bin/zsh" TERM=xterm-256color COLORTERM=truecolor ZDOTDIR=/z "${DENV[@]}" -- diagnostic
run TERM_PROGRAM=iTerm.app WT_SESSION=x TERM=xterm "${DENV[@]}" -- diagnostic
run WT_SESSION=x TERM=xterm "${DENV[@]}" -- diagnostic --format json
run WT_SESSION= TERM=xterm "${DENV[@]}" -- diagnostic --format json
run TERM_PROGRAM= TERM= "${DENV[@]}" -- diagnostic --format json
run SSH_CLIENT=1.2.3.4 "${DENV[@]}" -- diagnostic
run SSH_CONNECTION=a "${DENV[@]}" -- diagnostic --format json
run SSH_TTY=/dev/pts/0 "${DENV[@]}" -- diagnostic --format json
run SSH_CLIENT= SSH_TTY= "${DENV[@]}" -- diagnostic --format json
run CI=1 "${DENV[@]}" -- diagnostic --format json
run CI= "${DENV[@]}" -- diagnostic --format json
run CI=false "${DENV[@]}" -- diagnostic --format json
run WSL_DISTRO_NAME=Ubuntu "${DENV[@]}" -- diagnostic --format json
run WSL_DISTRO_NAME= "${DENV[@]}" -- diagnostic --format json
run USER=sb "${DENV[@]}" -- diagnostic
run USER=sb "SHELL=/home/sb/bin/zsh" "${DENV[@]}" -- diagnostic --format json
run USER=nobody HOME=/home/nobody "SHELL=/home/nobody/zsh" ZDOTDIR=/home/nobody/z "${DENV[@]}" -- diagnostic --format json
run USER= "${DENV[@]}" -- diagnostic --format json
run USER=home "${DENV[@]}" -- diagnostic --format json
run TABSMITH_SESSION_ID=abc TABSMITH_TERM=1 TABSMITH_TERM_TMUX=1 TABSMITH_a=1 TABSMITH__x=2 TABSMITH_Z=3 TABSMITH_= \
  OTHER=1 TABSMITH=1 tabsmith_lower=1 "${DENV[@]}" -- diagnostic
run 'TABSMITH_a.b=1' 'TABSMITH_a b=2' 'TABSMITH_é=3' 'TABSMITH_q"uote=4' "${DENV[@]}" -- diagnostic
run 'TABSMITH_NL=a
b' 'TABSMITH_Q=say "hi" \ back' $'TABSMITH_CTL=\x01\x1f\t end' 'TABSMITH_U=héllo ✔ 日本' "${DENV[@]}" -- diagnostic
run 'TABSMITH_NL=a
b' 'TABSMITH_Q=say "hi" \ back' $'TABSMITH_CTL=\x01\x1f\t end' 'TABSMITH_U=héllo ✔ 日本' "${DENV[@]}" -- diagnostic --format json
run 'TABSMITH_NL=a
b' 'TABSMITH_Q=say "hi" \ back' "${DENV[@]}" -- diagnostic --format json-pretty
run "TABSMITH_BAD=$(printf 'x\377y')" "${DENV[@]}" -- diagnostic
run "TABSMITH_BAD=$(printf 'x\377y')" "${DENV[@]}" -- diagnostic --format json
run "${DENV[@]}" -- diagnostic
run "${NOHOME[@]}" "PATH=$SB/pathbin" -- diagnostic
run "${NOHOME[@]}" "PATH=$SB/pathbin" -- diagnostic --format json
run "PATH=" -- diagnostic --format json
run "${DENV[@]}" -- -vv diagnostic
run "${DENV[@]}" -- diagnostic --help
run "${DENV[@]}" -- diagnostic extra
(mkdir "$SB/gone" && cd "$SB/gone" && rmdir "$SB/gone" && run "${DENV[@]}" -- diagnostic)
(mkdir -p "$SB/with space/\"q\"" && cd "$SB/with space/\"q\"" && run "${DENV[@]}" -- diagnostic)
dump

# ============================================================================================ q. debug logs
fresh "q. debug logs: the tail of pty.log"
paths bash
L="$SB/home/.tabsmith/logs"
run -- debug logs
dump
mkdir -p "$L"
run -- debug logs
: >"$L/pty.log"
run -- debug logs
printf 'one\ntwo\nthree\n' >"$L/pty.log"
run -- debug logs
printf 'one\ntwo\nthree' >"$L/pty.log"
run -- debug logs
printf 'one\r\ntwo\r\n\r\n' >"$L/pty.log"
run -- debug logs
printf '\n\n\n' >"$L/pty.log"
run -- debug logs
for n in 49 50 51 52 120; do
  seq 1 "$n" | sed 's/^/line /' >"$L/pty.log"
  echo "## $n lines"; run -- debug logs
done
printf 'bad \377\376 bytes\nok\n' >"$L/pty.log"
run -- debug logs
printf 'x\n' >"$L/pty.log"; chmod 000 "$L/pty.log"
run -- debug logs
rm -f "$L/pty.log"; mkdir "$L/pty.log"
run -- debug logs
rmdir "$L/pty.log"
printf 'a\n' >"$L/pty.log"; : >"$L/cli.log"
run -- debug logs
run -- -vv debug logs
run -- debug logs --help
run "${NOHOME[@]}" -- debug logs
run HOME= "TABSMITH_HOME=$SB/home/.tabsmith" -- debug logs
run "TABSMITH_HOME=$SB/other" -- debug logs
run "TABSMITH_HOME=reldata" -- debug logs
run "TABSMITH_HOME=$SB/other/" -- debug logs
run "TABSMITH_HOME=$SB/./other/../other//x" -- debug logs
mkdir -p "$SB/other"
run "TABSMITH_HOME=$SB/other" -- debug logs
rm -rf "$L"; printf 'x' >"$L"
run -- debug logs
rm "$L"
chmod 555 "$SB/home/.tabsmith"
run -- debug logs
chmod 755 "$SB/home/.tabsmith"
dump; dump "$SB/other"

# ================================================================================================== r. pty
fresh "r1. both streams on a pty: colours (stdout paint and stderr paint), --all and the fix flow"
paths bash zsh
PARENT=bash prun both 0 0 -- doctor --all
PARENT=bash prun both 0 0 "$TERM_OK" -- doctor
PARENT=bash prun both 0 0 "$TERM_OK" -- doctor --strict
PARENT=bash prun both 0 0 "$TERM_OK" -- doctor
PARENT=bash prun both 0 0 "$TERM_OK" -- doctor --all
dump

fresh "r2. the spinner: frames on stderr, cleared and the cursor restored before the first line (slow bash probe)"
good bash
printf '{"doctor.prompt-restart-terminal": true}' >"$SB/home/.tabsmith/state.json"
prun both 0 1 "$TERM_OK" SHIM_SLEEP_MS_BASH=600 -- doctor
prun stderr-tty 0 1 "$TERM_OK" SHIM_SLEEP_MS_BASH=600 -- doctor
prun stdout-tty 0 0 "$TERM_OK" SHIM_SLEEP_MS_BASH=600 -- doctor
prun both 0 0 "$TERM_OK" SHIM_SLEEP_MS_BASH=600 -- doctor --all
prun both 0 1 "$TERM_OK" SHIM_SLEEP_MS_BASH=600 "FAKE_VERSION_BASH=GNU bash, version 3.2.57(1)" -- doctor
prun both 0 1 "$TERM_OK" SHIM_SLEEP_MS_BASH=600 "FAKE_VERSION_BASH=GNU bash, version 3.2.57(1)" -- doctor --strict
prun both 0 0 -- doctor
run "$TERM_OK" -- doctor

fresh "r3. Ctrl-C while the spinner runs: the cursor is restored and the exit code is 1"
good bash
prun both 1 0 "$TERM_OK" SHIM_SLEEP_MS_BASH=3000 -- doctor
prun stderr-tty 1 0 "$TERM_OK" SHIM_SLEEP_MS_BASH=3000 -- doctor

fresh "r4. root on a pty: the stderr lines are painted (stderr is a tty)"
if need_root; then
  good bash
  ROOT=1 prun both 0 0 "$TERM_OK" -- doctor
  ROOT=1 prun both 0 0 "$TERM_OK" -- doctor --all
  ROOT=1 prun stdout-tty 0 0 "$TERM_OK" -- doctor
  ROOT=1 prun stderr-tty 0 0 "$TERM_OK" -- doctor --all
fi

fresh "r5. diagnostic and debug logs on a pty (no colours, same bytes)"
paths bash
prun both 0 0 TERM=xterm "PATH=$SB/pathbin" -- diagnostic
prun both 0 0 -- debug logs
exit 0
