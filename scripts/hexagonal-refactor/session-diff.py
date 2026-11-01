#!/usr/bin/env python3
"""session-diff.py <tabsmith binary> [scenario ...]

Drives `tabsmith debug shell bash` inside a detached tmux server (80x24) with scripted keystrokes and prints what a
user would SEE after each step (tmux capture-pane -p -e), plus the exit code, the log line and the run files.
Timing-tolerant: each step waits for an expected screen text (or its absence), then for the screen to settle, so the
transcript does not depend on how the PTY output was chunked. Diff the baseline binary against the new one.
"""
import os, re, shutil, subprocess, sys, tempfile, time, uuid

BIN = os.path.realpath(sys.argv[1])
ONLY = set(sys.argv[2:])
SB = os.path.realpath(tempfile.mkdtemp(prefix="session-diff-"))
HOME = f"{SB}/home"; TH = f"{HOME}/.tabsmith"; WORK = f"{SB}/work"
os.makedirs(TH); os.makedirs(WORK)
SOCK = f"pd{os.getpid()}{uuid.uuid4().hex[:6]}"
ENV = {"PATH": "/usr/local/bin:/usr/bin:/bin", "HOME": HOME, "TABSMITH_HOME": TH, "USERPROFILE": HOME,
       "TERM": "xterm-256color", "COLORTERM": "truecolor", "LANG": "C.UTF-8", "LC_ALL": "C.UTF-8", "SHELL": "/bin/bash",
       "PS1": "$ "}

def tmux(*a, check=True):
    r = subprocess.run(["tmux", "-L", SOCK, "-f", "/dev/null", *a], capture_output=True, text=True)
    if check and r.returncode:
        raise RuntimeError(f"tmux {a}: {r.stderr}")
    return r.stdout

def screen(target="s", styled=True):
    out = tmux("capture-pane", "-p", *(["-e"] if styled else []), "-t", target)
    return "\n".join(l.rstrip() for l in out.rstrip("\n").split("\n"))

def scrub(text):
    return text.replace(SB, "$SANDBOX").replace(BIN, "$BIN")

def wait(expect=None, absent=None, settle=0.45, timeout=6.0):
    """Until `expect` is on screen (and `absent` is not), then until the screen stops changing for `settle` s."""
    end = time.time() + timeout; last = None; since = time.time()
    while time.time() < end:
        cur = screen()
        plain = screen(styled=False)
        ok = (expect is None or expect in plain) and (absent is None or absent not in plain)
        if cur != last:
            last, since = cur, time.time()
        elif ok and time.time() - since >= settle:
            return cur
        time.sleep(0.05)
    return last + "\n(!! timeout waiting for %r / absent %r)" % (expect, absent)

def snap(label, **kw):
    print(f"--- {label}"); print(scrub(wait(**kw)))

def keys(*k, literal=False):
    tmux("send-keys", "-t", "s", *(["-l"] if literal else []), *k)

def type_slowly(text, gap=0.12):
    """One keystroke per read, like a person typing: the first reply after a prompt only arms the menu."""
    for ch in text:
        tmux("send-keys", "-t", "s", "-l", ch)
        time.sleep(gap)

def start(name, cmd_suffix="-vv debug shell bash"):
    print(f"\n======== {name}")
    tmux("kill-server", check=False)
    envs = " ".join(f"{k}={v!r}" for k, v in ENV.items())
    shell_cmd = f"cd {WORK} && env -i {envs} {BIN} {cmd_suffix}; echo EXIT=$?; sleep 30"
    tmux("new-session", "-d", "-s", "s", "-x", "80", "-y", "24", "sh", "-c", shell_cmd)

def ready():
    """Wait for the first prompt, then make it `$ ` so the transcript names no user, host or directory."""
    wait(expect="work#")
    keys("PS1='$ '", "Enter")
    wait(expect="\n$ ")
    keys("C-l")
    wait(settle=0.5)

def finish():
    tmux("kill-server", check=False)

def scenario_startup():
    start("startup and prompt")
    ready(); snap("prompt")
    keys("exit", "Enter")
    snap("exit 0 propagated", expect="EXIT=")
    finish()

def scenario_menu():
    start("menu: show, navigate, tab, esc")
    ready()
    type_slowly("git ch")
    snap("git ch shows the menu", expect="> checkout")
    keys("Down", "Down")
    snap("selection moved twice", settle=0.6)
    keys("Tab")
    snap("tab inserted the selection", settle=0.6)
    keys("C-u"); wait(absent="git")
    type_slowly("git ch"); wait(expect="> checkout")
    keys("Escape")
    snap("esc hides the menu", absent="> checkout")
    keys("C-u"); keys("exit", "Enter"); snap("exit", expect="EXIT=")
    finish()

def scenario_ghost():
    start("ghost text from history")
    ready()
    keys("echo hi", "Enter"); wait(expect="hi\n")
    time.sleep(0.5)
    type_slowly("ec")
    snap("ec ghosts ho hi", expect="ho hi")
    keys("Right")
    snap("right accepts the ghost", settle=0.6)
    keys("C-u"); keys("exit", "Enter"); snap("exit", expect="EXIT=")
    finish()

def scenario_bottom_row_and_resize():
    start("prompt on the bottom row, resize")
    ready()
    keys("seq 1 40", "Enter"); wait(expect="40")
    type_slowly("git ch")
    snap("menu above the bottom prompt", expect="> checkout")
    tmux("resize-window", "-t", "s", "-x", "60", "-y", "18")
    snap("resized to 60x18 (menu stays drawn once)", settle=0.8)
    tmux("resize-window", "-t", "s", "-x", "80", "-y", "24")
    snap("resized back", settle=0.8)
    keys("Escape"); keys("C-u"); keys("exit", "Enter"); snap("exit", expect="EXIT=")
    finish()

def scenario_vim_and_exit_code():
    start("vim draws no overlay, exit code")
    ready()
    keys("vim -u NONE -N", "Enter"); wait(expect="~")
    keys("ihello world")
    snap("vim insert mode", settle=0.6)
    keys("Escape"); keys(":q!", "Enter")
    snap("back at the prompt", expect="$")
    keys("exit 3", "Enter")
    snap("exit 3 propagated", expect="EXIT=3")
    finish()

def scenario_settings_reload():
    start("settings reload (poll_files, 2 s)")
    ready()
    type_slowly("git ch"); snap("default height", expect="> checkout")
    keys("Escape"); keys("C-u")
    subprocess.run([BIN, "settings", "autocomplete.height", "2"], env=ENV, cwd=WORK, capture_output=True)
    time.sleep(2.6)
    type_slowly("git ch")
    snap("height 2 after the reload", expect="> checkout")
    subprocess.run([BIN, "theme", "light"], env=ENV, cwd=WORK, capture_output=True)
    time.sleep(2.6)
    keys("Escape"); keys("C-u"); type_slowly("git ch")
    snap("light theme after the reload", expect="> checkout")
    keys("Escape"); keys("C-u"); keys("exit", "Enter"); snap("exit", expect="EXIT=")
    finish()

def scenario_child():
    start("child environment, cwd, title", "-vv _ pty --shell bash -- --noprofile --norc")
    wait(expect="work#")
    keys("PS1='$ '", "Enter"); wait(expect="\n$ ")
    keys("pwd; echo $0; env | grep ^TABSMITH_ | sed -E 's/=[0-9a-f]{32}$/=SID/' | sort", "Enter")
    snap("pwd, argv0 and the TABSMITH_ variables the child sees", expect="TABSMITH_TERM=")
    print("title:", tmux("display-message", "-p", "-t", "s", "#{pane_title}").strip())
    keys("exit 5", "Enter"); snap("exit 5 propagated", expect="EXIT=5")
    finish()

def scenario_failures():
    """What no pty shows: start-up failures fall back to a plain shell (stdin is not a terminal, no shell), the
    debug shell's rc file and argv per shell (stub shells print them), and the key tester without a terminal."""
    print("\n======== non-terminal paths")
    stub = f"{SB}/stubs"; os.makedirs(stub, exist_ok=True)
    for name in ("bash", "zsh", "fish", "pwsh"):
        path = f"{stub}/{name}"
        open(path, "w").write('#!/bin/sh\necho "STUB $0 argv:$*"; echo "TABSMITH_DEBUG_SHELL=${TABSMITH_DEBUG_SHELL-unset} ZDOTDIR=${ZDOTDIR-unset} DISABLED=${TABSMITH_TERM_DISABLED-unset}"\n'
                              'show() { echo "-- $1: $(wc -l < "$1") lines, sha $(sed "s#$PD_BIN#BIN#g" "$1" | sha256sum | cut -c1-16)"; }\n'
                              '[ -n "$ZDOTDIR" ] && [ -f "$ZDOTDIR/.zshrc" ] && show "$ZDOTDIR/.zshrc"\n'
                              'for a in "$@"; do case "$a" in *debug-shell-*) f=$(printf "%s" "$a" | sed -n "s/.*[\x27 ]\\([^\x27 ]*debug-shell-[0-9]*\\/[^\x27 ]*\\).*/\\1/p"); [ -f "$f" ] && show "$f";; esac; done\n'
                              'while [ $# -gt 0 ]; do case "$1" in --rcfile) show "$2"; shift;; esac; shift; done\nexit 9\n')
        os.chmod(path, 0o755)
    cases = [
        ["_", "pty", "--", "sh", "-c", "echo A:$0 D:$TABSMITH_TERM_DISABLED S:${TABSMITH_SHELL-unset} L:${TABSMITH_IS_LOGIN_SHELL-unset}; exit 7"],
        ["_", "pty", "--shell", "/nonexistent/shell"],
        ["_", "pty"],
        ["_", "pty", "--shell", "sh", "--", "-c", "echo extra-args-only"],
        ["debug", "shell"],
        ["debug", "shell", "bash"], ["debug", "shell", "zsh"], ["debug", "shell", "fish"], ["debug", "shell", "pwsh"],
        ["debug", "key-tester"],
    ]
    for envx in ({}, {"SHELL": "/bin/zsh"}, {"TABSMITH_SHELL": "bash", "TABSMITH_IS_LOGIN_SHELL": "1"}):
        for c in cases:
            env = {"PD_BIN": BIN, "PATH": f"{stub}:/usr/bin:/bin", "HOME": HOME, "TABSMITH_HOME": TH, "USERPROFILE": HOME, **envx}
            r = subprocess.run([BIN, *c], env=env, cwd=WORK, stdin=subprocess.DEVNULL, capture_output=True, text=True)
            print(f"### {envx} {' '.join(c)}")
            out = re.sub(r"debug-shell-\d+", "debug-shell-PID", scrub(r.stdout))
            err = re.sub(r"debug-shell-\d+", "debug-shell-PID", scrub(r.stderr))
            print(out.rstrip()); print("--- stderr"); print(err.rstrip()); print(f"--- exit {r.returncode}")
    print("## run/ after the exec fallbacks (the debug dirs exec skipped removing):",
          sorted(re.sub(r"\d+", "PID", n) for n in os.listdir(f"{TH}/run")) if os.path.isdir(f"{TH}/run") else None)

def scenario_key_tester():
    start("debug key-tester on a terminal", "debug key-tester")
    wait(expect="Testing Key Input")
    keys("a"); keys("Up"); keys("C-a"); keys("F5"); keys("M-x")
    snap("bytes of a, Up, Ctrl-A, F5, Alt-x", settle=0.6)
    keys("C-d"); snap("ctrl-d quits", expect="EXIT=")
    finish()

def scenario_files():
    print("\n======== files after a session")
    log = os.path.join(TH, "logs", "pty.log")
    if os.path.exists(log):
        for line in open(log, errors="replace"):
            line = re.sub(r"^\S+ ", "TS ", line)             # leading timestamp
            line = re.sub(r"sid [0-9a-f]{32}", "sid SID", line)
            print(scrub(line.rstrip()))
    run = os.path.join(TH, "run")
    print("run/:", sorted(re.sub(r"debug-shell-\d+", "debug-shell-PID", re.sub(r"[0-9a-f]{32}", "SID", n)) for n in os.listdir(run)) if os.path.isdir(run) else None)
    hist = os.path.join(TH, "history.jsonl")
    if os.path.exists(hist):
        for l in open(hist):
            l = re.sub(r'"(start_time|end_time|session_id|pid)":\s*[^,}]+', r'"\1":X', l)
            print(scrub(l.rstrip()))

SCENARIOS = [("startup", scenario_startup), ("menu", scenario_menu), ("ghost", scenario_ghost),
             ("child", scenario_child), ("failures", scenario_failures), ("keytester", scenario_key_tester),
             ("bottom", scenario_bottom_row_and_resize), ("vim", scenario_vim_and_exit_code),
             ("reload", scenario_settings_reload), ("files", scenario_files)]
try:
    for name, fn in SCENARIOS:
        if not ONLY or name in ONLY:
            fn()
finally:
    tmux("kill-server", check=False)
    shutil.rmtree(SB, ignore_errors=True)
