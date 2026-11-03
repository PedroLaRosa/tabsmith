#!/usr/bin/env python3
"""Drive `tabsmith _ pty` on a real pty and print a scrubbed transcript. usage: pty-diff.py <binary>"""
import os, pty, re, select, signal, sys, tempfile, time

BIN = sys.argv[1]

def scrub(b, home):
    s = b.decode("latin-1")
    s = s.replace(home, "<HOME>")
    s = re.sub(r"[0-9a-f]{32}", "<SID>", s)
    s = re.sub(r"\d+\.\d+\.\d+", "<VER>", s)
    return s.encode("latin-1")

def run_pipe(name, args):
    import subprocess
    home = tempfile.mkdtemp(prefix="ptydiff")
    env = {"PATH": os.environ["PATH"], "HOME": home, "TABSMITH_HOME": home}
    p = subprocess.run([BIN] + args, env=env, input=b"", capture_output=True, timeout=20)
    print(f"== {name}: exit {p.returncode}")
    print(repr(scrub(p.stdout, home)), repr(scrub(p.stderr, home)))

def run(name, args, env_extra=None, script=None, settings=None, gone_cwd=False, timeout=15):
    home = tempfile.mkdtemp(prefix="ptydiff")
    if settings is not None:
        open(os.path.join(home, "settings.json"), "w").write(settings)
    env = {"PATH": os.environ["PATH"], "TERM": "xterm-256color", "HOME": home, "TABSMITH_HOME": home}
    env.update(env_extra or {})
    pid, fd = pty.fork()
    if pid == 0:
        if gone_cwd:
            d = os.path.join(home, "gone"); os.mkdir(d); os.chdir(d); os.rmdir(d)
        else:
            os.chdir(home)
        os.execve(BIN, [BIN] + args, env)
    out = b""
    start = time.time()
    sent = set()
    status = None
    while time.time() - start < timeout:
        r, _, _ = select.select([fd], [], [], 0.05)
        if r:
            try:
                d = os.read(fd, 65536)
            except OSError:
                d = b""
            if not d:
                break
            out += d
        for i, (trigger, reply) in enumerate(script or []):
            if i not in sent and trigger in out:
                sent.add(i)
                if reply == "TERM":
                    os.kill(pid, signal.SIGTERM)
                else:
                    os.write(fd, reply)
        w, st = os.waitpid(pid, os.WNOHANG)
        if w:
            status = st
            try:
                while select.select([fd], [], [], 0.2)[0]:
                    d = os.read(fd, 65536)
                    if not d: break
                    out += d
            except OSError:
                pass
            break
    if status is None:
        t0 = time.time()
        while status is None and time.time() - t0 < 3:
            w, st = os.waitpid(pid, os.WNOHANG)
            if w: status = st
            else: time.sleep(0.05)
    if status is None:
        try: os.kill(pid, 9)
        except OSError: pass
        _, status = os.waitpid(pid, 0)
        out += b"<<TIMEOUT>>"
    code = os.waitstatus_to_exitcode(status)
    print(f"== {name}: exit {code}")
    print(repr(scrub(out, home)))
    files = sorted(os.listdir(os.path.join(home, "run"))) if os.path.isdir(os.path.join(home, "run")) else []
    print("run files:", files)
    log = os.path.join(home, "logs", "pty.log")
    if os.path.exists(log):
        lines = [re.sub(r"^\S+\s+", "", l) for l in open(log).read().splitlines()]
        print("pty.log:", scrub("\n".join(lines).encode(), home).decode())

CPR = b"\x1b[5;1R"
BG = b"\x1b]11;rgb:ffff/ffff/ffff\x1b\\"
run("answers-probe-exit3", ["_", "pty", "--shell", "/bin/sh", "--", "-c", "echo hi; exit 3"],
    script=[(b"\x1b[6n", BG + CPR)])
run("no-answer-typeahead", ["_", "pty", "--shell", "/bin/sh", "--", "-c", "echo hi; exit 0"], script=[])
run("env-and-cwd", ["_", "pty", "--shell", "/bin/sh", "--", "-c", "env | grep -E '^(TABSMITH|PWD)' | sort; pwd; exit 0"],
    script=[(b"\x1b[6n", CPR)])
run("sigterm", ["_", "pty", "--shell", "/bin/sh", "--", "-c", "echo up; sleep 30"],
    script=[(b"\x1b[6n", CPR), (b"up", "TERM")])
run("no-shell", ["_", "pty"], env_extra={"SHELL": ""}, script=[(b"# ", b"exit 5\n")])
run_pipe("not-a-tty-fallback", ["_", "pty", "--shell", "/bin/sh", "--", "-c", "echo disabled=$TABSMITH_TERM_DISABLED; exit 4"])
run("typeahead-during-probe", ["_", "pty", "--shell", "/bin/sh", "--", "-c", "read a; echo got=$a"],
    script=[(b"\x1b[6n", b"xy" + CPR), (b"\x1b]0;sh\x07", b"\n")])
run("cwd-gone-falls-back-to-home", ["_", "pty", "--shell", "/bin/sh", "--", "-c", "pwd"], script=[(b"\x1b[6n", CPR)], gone_cwd=True)
run("corrupt-settings-warns", ["_", "pty", "--shell", "/bin/sh", "--", "-c", "exit 0"], script=[(b"\x1b[6n", CPR)], settings="{oops")
run("log-line-at-vv", ["-vv", "_", "pty", "--shell", "/bin/sh", "--", "-c", "exit 0"], script=[(b"\x1b[6n", CPR)])
