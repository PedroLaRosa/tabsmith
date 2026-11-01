#!/bin/bash
# Usage: theme-diff.sh <tabsmith binary>
# Prints a scrubbed transcript of what the golden capture never shows of `tabsmith theme`: the listing over every kind
# of file and folder, --folder, valid custom themes (the banner and the saved settings.json), every `invalid theme
# file` text of the strict parser, the built-in names against theme files of the same name, every settings.json state,
# the save-after-banner failure, the display of non-string values, the bold escapes on a pty and the clap conflicts.
# Phase 8 diffs it at every step:
#   diff <(scripts/hexagonal-refactor/theme-diff.sh target/hexagonal-refactor/baseline-tree/target/debug/tabsmith) \
#        <(scripts/hexagonal-refactor/theme-diff.sh target/debug/tabsmith)
# macOS (`stat -f`, bash 3.2). Each group starts from a fresh sandbox; every command runs with a cleared env (`env -i`)
# and stdin from /dev/null, except the pty group. After each command: settings.json (bytes, mode) and the entries of
# the data directory, which show that nothing is created lazily.
export LC_ALL=C   # bytes, not characters: sed, ls, sort and cat -v behave the same on every machine
BIN="$(cd "$(dirname "$1")" && pwd -P)/$(basename "$1")"
BINDIR="$(dirname "$BIN")"
SANDBOXES=()
cleanup() { for s in "${SANDBOXES[@]}"; do chmod -R u+rwX "$s" 2>/dev/null; rm -rf "$s"; done; }
trap cleanup EXIT

# fresh <title>: a new empty sandbox (HOME = $SB/home, data directory = $DATA), cwd = the sandbox
fresh() {
  local t; t=$(mktemp -d) || exit 1
  SB=$(cd "$t" && pwd -P) || exit 1
  case "$SB" in */tmp.*) ;; *) echo "unexpected sandbox: $SB"; exit 1 ;; esac
  SANDBOXES+=("$SB"); DATA="$SB/home/.tabsmith"; mkdir -p "$DATA"; cd "$SB" || exit 1
  echo; echo "======== $1"
}
scrub() { sed -e "s#$SB#\$SANDBOX#g" -e "s#$BIN#\$BIN#g" -e "s#$BINDIR#\$BINDIR#g"; }
# show <file>: its scrubbed content, and whether it lacks a final newline
show() { scrub <"$1"; if [ -s "$1" ] && [ -n "$(tail -c 1 "$1")" ]; then echo; echo "(no newline at end)"; fi; }
not_root() { [ "$(id -u)" != 0 ] || { echo "(skipped: running as root)"; return 1; }; }
# snapfile <label> <file>: the state of one file
snapfile() {
  echo "--- $1"
  if [ -d "$2" ]; then echo "(directory)"
  elif [ -e "$2" ]; then show "$2"; echo "mode $(stat -f %Lp "$2")"
  else echo "(none)"; fi
}
# snap: settings.json and the entries of the data directory
snap() { snapfile settings.json "$DATA/settings.json"; echo "--- ls -A"; ls -A "$DATA" 2>&1 | sort | scrub; }
# arg display: an empty or spaced argument is quoted
shown() { local a out=""; for a in "$@"; do case "$a" in ''|*' '*) a="'$a'" ;; esac; out="$out $a"; done; echo "$out"; }

# run [VAR=value ...] -- <args...>: one command, stdin from /dev/null; later assignments win over the defaults
run() {
  local envs=(); while [ "$1" != "--" ]; do [ $# -gt 0 ] || { echo "no -- in a run call"; exit 1; }; envs+=("$1"); shift; done; shift
  echo "###${envs[*]:+ ${envs[*]}} theme$(shown "$@")" | scrub
  env -i PATH="$PATH" HOME="$SB/home" TABSMITH_HOME="$DATA" USERPROFILE="$SB/home" "${envs[@]}" \
    "$BIN" theme "$@" </dev/null >"$SB/out" 2>"$SB/err"
  local code=$?
  show "$SB/out"; echo "--- stderr"; show "$SB/err"; echo "--- exit $code"; snap
}
put() { printf '%s' "$1" >"$DATA/settings.json"; }              # settings.json holds exactly these bytes
theme_file() { mkdir -p "$(dirname "$DATA/themes/$1.json")"; printf '%s' "$2" >"$DATA/themes/$1.json"; }
clean() { chmod -R u+rwX "$DATA" 2>/dev/null; rm -rf "$DATA/themes" "$DATA/settings.json" "$DATA/up.json"; }
NOHOME=(HOME= TABSMITH_HOME= USERPROFILE=)

# The pty driver: argv = <out|both> <stderr file> <command...>. stdin is /dev/null, stdout (and stderr when `both`)
# the pty slave. Prints the pty bytes (\r\n -> \n) and exits with the child's code.
PTY='
import os, select, subprocess, sys, time
mode, err_path, cmd = sys.argv[1], sys.argv[2], sys.argv[3:]
master, slave = os.openpty()
err = slave if mode == "both" else open(err_path, "wb")
child = subprocess.Popen(cmd, stdin=subprocess.DEVNULL, stdout=slave, stderr=err)
os.close(slave)
data, deadline = b"", time.time() + 30
def drain():
    global data
    while select.select([master], [], [], 0.05)[0]:
        try:
            chunk = os.read(master, 65536)
        except OSError:
            return
        if not chunk:
            return
        data += chunk
while child.poll() is None:
    drain()
    if time.time() > deadline:
        child.kill()
        child.wait()
        data += b"\n(killed: timed out)\n"
        break
drain()
sys.stdout.buffer.write(data.replace(b"\r\n", b"\n"))
sys.stdout.flush()
code = child.returncode
sys.exit(code if code >= 0 else 128 - code)
'
# prun <out|both> [VAR=value ...] -- <args...>: one command with stdout (or both) on a pty, shown through `cat -v`
prun() {
  local mode=$1; shift
  local envs=(); while [ "$1" != "--" ]; do [ $# -gt 0 ] || { echo "no -- in a run call"; exit 1; }; envs+=("$1"); shift; done; shift
  echo "### pty($mode)${envs[*]:+ ${envs[*]}} theme$(shown "$@")" | scrub
  : >"$SB/err"   # `both` leaves it untouched: do not show the previous command's stderr
  python3 -c "$PTY" "$mode" "$SB/err" env -i PATH="$PATH" HOME="$SB/home" TABSMITH_HOME="$DATA" USERPROFILE="$SB/home" \
    "${envs[@]}" "$BIN" theme "$@" >"$SB/pty"
  local code=$?
  echo "--- pty"; cat -v "$SB/pty" | scrub
  echo "--- stderr"; show "$SB/err"; echo "--- exit $code"; snap
}

AUTHOR='{"author":{"name":"Ann Lee","twitter":"@ann","github":"annlee"}}'

fresh "A list"
run -- --list                                              # the folder does not exist
mkdir -p "$DATA/themes"; run -- --list                     # empty
for f in zed alpha dark light system "a b" a.b .hidden é 日本 noext; do
  case "$f" in noext) : >"$DATA/themes/$f" ;; *) printf '{}' >"$DATA/themes/$f.json" ;; esac
done
: >"$DATA/themes/notes.txt"; : >"$DATA/themes/UP.JSON"; : >"$DATA/themes/x.json.bak"; mkdir "$DATA/themes/dirtheme.json"
if { printf '{}' >"$DATA/themes/$(printf '\377').json"; } 2>/dev/null; then echo "(non-UTF-8 name created)"
else echo "(skipped: the filesystem refuses a non-UTF-8 name)"; fi
run -- --list                                              # every kind of entry
rm -rf "$DATA/themes"; : >"$DATA/themes"; run -- --list    # `themes` is a regular file
rm -f "$DATA/themes"; mkdir "$SB/real"; printf '{}' >"$SB/real/zed.json"; ln -s "$SB/real" "$DATA/themes"
run -- --list                                              # `themes` is a symlink to a directory
rm -f "$DATA/themes"; mkdir "$DATA/themes"; printf '{}' >"$DATA/themes/zed.json"
if not_root; then chmod 000 "$DATA/themes"; run -- --list; chmod 755 "$DATA/themes"; fi
mkdir -p "$SB/rel/themes"; printf '{}' >"$SB/rel/themes/zed.json"; run TABSMITH_HOME=rel -- --list   # a relative home
put '{oops'; run -- --list                                 # never reads settings.json
clean; run "${NOHOME[@]}" -- --list                        # no home: the error comes first

fresh "B folder"
run -- --folder; run -- --folder                           # creates and prints, then prints again
rm -rf "$DATA/themes"; : >"$DATA/themes"; run -- --folder  # File exists (os error 17)
rm -f "$DATA/themes"; run TABSMITH_HOME=rel -- --folder; echo "== the relative home"; DATA="$SB/rel"; snap; DATA="$SB/home/.tabsmith"
run "${NOHOME[@]}" -- --folder
put '{oops'; run -- --folder                               # never reads settings.json

fresh "C custom valid"
for c in "$AUTHOR" '{"author":["Ann","@t","gh"]}' '{"author":{"name":"Ann"}}' '{"author":{"twitter":"@ann"}}' \
         '{"author":{"github":"ann"}}' '{"author":{}}' '{"author":null}' '{"colors":1}' \
         '{"version":null,"theme":null}' '{}' \
         '{"author":{"name":"Ann","github":"ann"},"version":"1.0","shade0":"#000000"}' \
         '{"version":"2","theme":{"backgroundColor":"red"},"shade1":"nope","accent3":""}' '{"a":1,"a":2}'; do
  clean; theme_file zed "$c"; echo "== zed.json: $c"; run -- zed
done
clean; theme_file zed "$AUTHOR"; put '{"autocomplete.height":5,"autocomplete.theme":"light","zzz":[1,{"a":null}]}'
run -- zed                                                 # the other settings are kept
for n in sub/x ../up 日本 ''; do clean; theme_file "$n" "$AUTHOR"; echo "== name '$n'"; run -- "$n"; done
clean; theme_file zed "$AUTHOR"; run -- zed; run -- zed; run -- --list

fresh "D custom invalid"
BOM=$(printf '\357\273\277')
for c in 'not json' '' '{} x' "$BOM{}" '[1]' '"x"' 'null' '5' '{"author":"Ann"}' '{"author":{"name":5}}' \
         '{"author":{"twitter":true}}' '{"author":5}' '{"author":["Ann"]}' '{"author":[]}' \
         '{"author":["a","b","c","d"]}' '{"author":["a",1,"c"]}' '{"version":1}' '{"version":["x"]}' '{"theme":"x"}' \
         '{"theme":[1]}' '{"author":{"name":"a"},"author":{"name":"b"}}' '{"shade3":5}' \
         '{"shade9":null,"accent1":1,"accent0":2}' '{"shadeX":true}' '{"accent":1}' '{"accent2":{"a":1}}' \
         '{"author":{"name":"a"},"shade0":"#000","accent1":false}'; do
  clean; theme_file bad "$c"; put '{"autocomplete.height":5}'; echo "== bad.json: $c"; run -- bad
done

fresh "E built-ins and collisions"
for n in dark light system; do
  clean; echo "== $n, no file"; run -- "$n"
  clean; theme_file "$n" "$AUTHOR"; echo "== $n, valid file"; run -- "$n"
  clean; theme_file "$n" '[1]'; echo "== $n, invalid file"; run -- "$n"
  if not_root; then
    clean; theme_file "$n" "$AUTHOR"; chmod 000 "$DATA/themes/$n.json"; echo "== $n, unreadable file"; run -- "$n"
    chmod 644 "$DATA/themes/$n.json"
  fi
  clean; mkdir -p "$DATA/themes/$n.json"; echo "== $n, directory named $n.json"; run -- "$n"
done
clean; mkdir -p "$DATA/themes/zed.json"; echo "== zed, directory named zed.json"; run -- zed
if not_root; then
  clean; theme_file zed "$AUTHOR"; chmod 000 "$DATA/themes/zed.json"; echo "== zed, unreadable file"; run -- zed
  chmod 644 "$DATA/themes/zed.json"
fi
for n in Dark 'dark ' nope ''; do clean; echo "== '$n', no file"; run -- "$n"; done
clean; theme_file '' "$AUTHOR"; echo "== '', file .json"; run -- ''
clean; theme_file '' '[1]'; echo "== '', invalid file .json"; run -- ''
clean; theme_file Dark "$AUTHOR"; echo "== Dark with Dark.json"; run -- Dark
clean; run -- light; run -- ''; run -- dark; run --

fresh "F settings states"
setstate() {   # setstate <state>: settings.json in one of the states below, nothing else changed
  rm -rf "$DATA/settings.json"
  case "$1" in
    corrupted) put '{oops' ;; array) put '[1]' ;; dir) mkdir "$DATA/settings.json" ;; missing) ;;
    blank) put '' ;; spaces) put $'  \n' ;; ok) put '{"autocomplete.height":5}' ;;
  esac
}
for st in corrupted array dir missing blank spaces; do
  echo "== settings.json: $st"
  for a in zed light nope --list --folder; do
    clean; theme_file zed "$AUTHOR"; setstate "$st"; run -- "$a"
  done
  clean; theme_file zed "$AUTHOR"; setstate "$st"; run --
done
echo "== the data directory is not writable"
for a in light zed nope; do
  for st in missing ok; do
    clean; theme_file zed "$AUTHOR"; setstate "$st"; chmod 555 "$DATA"; echo "== $a, settings.json: $st"; run -- $a; chmod 755 "$DATA"
  done
done
echo "== TABSMITH_SETTINGS_PATH with a home: the themes stay under the home"
clean; theme_file zed "$AUTHOR"; put '{"autocomplete.height":5}'
for a in zed light nope --list --folder; do
  run "TABSMITH_SETTINGS_PATH=$SB/s.json" -- $a; snapfile s.json "$SB/s.json"
done
run "TABSMITH_SETTINGS_PATH=$SB/s.json" -- ; snapfile s.json "$SB/s.json"
echo "== TABSMITH_SETTINGS_PATH alone (no home)"
rm -f "$SB/s.json"
for a in zed --list --folder; do run "${NOHOME[@]}" "TABSMITH_SETTINGS_PATH=$SB/s.json" -- $a; done
run "${NOHOME[@]}" "TABSMITH_SETTINGS_PATH=$SB/s.json" --; snapfile s.json "$SB/s.json"
echo "== no home"
for a in light --list --folder; do clean; run "${NOHOME[@]}" -- $a; done
run "${NOHOME[@]}" --
echo "== which variable names the home"
run HOME= USERPROFILE= -- --folder; run TABSMITH_HOME= USERPROFILE= -- --folder
run TABSMITH_HOME= HOME= -- --folder; run TABSMITH_HOME= -- --folder

fresh "G show"
clean; run --                                              # unset: the catalog default
for v in '"light"' '""' '"a\nb"' '"dark "' 'null' '5' '5.0' '-3' '0.1' '1e300' '18446744073709551615' \
         '-9223372036854775808' 'true' '["a",1]' '{"b":1,"a":[1,2]}' '{}' '[]' '"zed"'; do
  clean; put "{\"autocomplete.theme\":$v}"; echo "== autocomplete.theme = $v"; run --
done
clean; put '{"autocomplete.theme":"light","autocomplete.height":5}'; run --; run -- light; run --
put '{oops'; run --
clean; run "${NOHOME[@]}" --

fresh "H tty"
theme_file zed "$AUTHOR"; theme_file plain '{}'; theme_file bad '[1]'
prun out -- zed; prun out -- plain; prun out -- light; prun out -- --list; prun out -- --folder; prun out --
prun out -- bad; prun out -- nope; prun both -- nope; prun both -- bad; prun both -- zed

fresh "I clap"
for a in "--list --folder" "--list zed" "--folder zed" "--list --list" "a b" "-x" "--nope" "--folder --list" "--list --folder zed"; do
  run -- $a
done
