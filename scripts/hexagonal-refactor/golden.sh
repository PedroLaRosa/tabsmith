#!/bin/bash
# Usage: golden.sh <binary> <out-file>
# Runs non-interactive tabsmith commands in an isolated HOME and records stdout/stderr/exit,
# with the sandbox and binary paths scrubbed so two runs can be diffed. macOS (`stat -f`); see docs/hexagonal-refactor.
BIN="$(cd "$(dirname "$1")" && pwd -P)/$(basename "$1")"; OUT="$2"
BINDIR="$(dirname "$BIN")"
BIN_U="${BIN//\/$USER\//\/USER\/}"   # `diagnostic` rewrites /$USER/ to /USER/
SB=$(mktemp -d)
export HOME="$SB/home" TABSMITH_HOME="$SB/home/.tabsmith" USERPROFILE="$SB/home"
unset TABSMITH_SETTINGS_PATH TABSMITH_SESSION_ID TABSMITH_TERM TABSMITH_TERM_TMUX
mkdir -p "$HOME"; cd "$SB" || exit 1
: > "$OUT"
scrub() { sed -e "s#$SB#\$SANDBOX#g" -e "s#$BIN#\$BIN#g" -e "s#$BIN_U#\$BIN#g" -e "s#$BINDIR#\$BINDIR#g"; }
run() {
  echo "### $*" >> "$OUT"
  "$BIN" "$@" </dev/null >"$SB/o" 2>"$SB/e"; code=$?
  scrub < "$SB/o" >> "$OUT"
  echo "--- stderr" >> "$OUT"
  scrub < "$SB/e" >> "$OUT"
  echo "--- exit $code" >> "$OUT"
}
dump() {
  echo "### files under HOME (logs excluded)" >> "$OUT"
  (cd "$HOME" && find . -type f ! -path './.tabsmith/logs/*' ! -path './.tabsmith/specs/*' | sort | while read -r f; do
    echo "## $f $(stat -f %Lp "$f")"; cat "$f"; done) 2>&1 | scrub >> "$OUT"
}
run --help
run version
for c in settings theme inline integrations init doctor setup uninstall diagnostic debug; do run $c --help; done
run _ --help
run _ pty --help
run _ pre-cmd --help
run _ should-launch --help
run _ get-shell --help
run debug shell --help
run debug key-tester --help
run debug logs --help
run settings list --help
run settings open --help
run inline set-customization --help
run integrations install --help
run integrations status dotfiles --help
run completion --help
for s in bash zsh fish pwsh; do for w in pre post; do run init $s $w; done; done
run init bash pre --rcfile .bashrc
run settings list
run settings list --all
run settings list --all --format json
run settings list --all --format json-pretty
run settings autocomplete.height
run settings autocomplete.height 12
run settings autocomplete.height
run settings autocomplete.height --format json
run settings 'autocomplete.theme' '"light"'
run settings 'inline.llm.profiles' '{"a":{"provider":"openai","model":"m"}}'
run settings list
run settings list --format json
run settings -d 'autocomplete.*'
run settings -d nope.key
run settings list
run settings no.such.key
run settings
run settings -d
run settings autocomplete.height 3 -d
run settings ""
run settings "" v
run settings ""
run settings -d ""
run settings list --all --format json
run theme
run theme --list
run theme --folder
run theme light
run theme
run theme nope
run inline status
run inline show-customizations
run inline show-customizations --format json
run inline disable
run inline status
run inline enable
run inline set-customization history
run inline show-customizations
run integrations status dotfiles
dump
run integrations status all
run integrations status all --format json
run integrations install dotfiles
run integrations status dotfiles
dump
run integrations uninstall dotfiles
run integrations status dotfiles
dump
run _ should-launch
run completion bash
run completion zsh
run completion fish
run debug logs
run diagnostic
run diagnostic --format json
run doctor --all

# ---- error paths and env overrides
ST="$TABSMITH_HOME/settings.json"
printf '{oops' > "$ST"
run settings list
run settings autocomplete.height
run settings autocomplete.height 3
run theme
run inline status
run doctor --all
printf '[1]' > "$ST"
run settings list
run settings autocomplete.height 4
rm -f "$ST"; mkdir -p "$ST"
run settings list
run settings autocomplete.height 5
rmdir "$ST"
printf '' > "$ST"
run settings list
mkdir -p "$SB/elsewhere"; printf '{"autocomplete.width": 50}\n' > "$SB/elsewhere/real.json"
rm -f "$ST"; ln -s "$SB/elsewhere/real.json" "$ST"
run settings autocomplete.height 9
run settings list
echo "### symlink after set" >> "$OUT"; (cd "$HOME" && find . -type l | sort | while read -r l; do echo "$l -> $(readlink "$l")"; done) | sed "s#$SB#\$SANDBOX#g" >> "$OUT"
cat "$SB/elsewhere/real.json" >> "$OUT"; stat -f %Lp "$SB/elsewhere/real.json" >> "$OUT"
export TABSMITH_SETTINGS_PATH="$SB/override/s.json"
run settings autocomplete.height 11
run settings list
echo "### override file" >> "$OUT"; cat "$SB/override/s.json" >> "$OUT" 2>&1; stat -f %Lp "$SB/override/s.json" >> "$OUT" 2>&1
unset TABSMITH_SETTINGS_PATH
(export HOME="" TABSMITH_HOME="" USERPROFILE=""; run settings list; run settings autocomplete.height 1; run inline status; run theme --folder; run debug logs)
(export TABSMITH_HOME=""; run theme --folder; run debug logs)
(export HOME="" TABSMITH_HOME="" USERPROFILE=""; run --help; run version; run init zsh post; run init bash pre; run _ should-launch; run completion bash; run integrations status dotfiles; run doctor --all; run diagnostic)
(export HOME="" TABSMITH_HOME="" USERPROFILE="" TABSMITH_SETTINGS_PATH="$SB/nohome/s.json"; run settings autocomplete.height 6; run settings list; run settings list --all; run theme; run inline status)
echo "### nohome override file" >> "$OUT"; cat "$SB/nohome/s.json" >> "$OUT" 2>&1

echo "### settings.json" >> "$OUT"; cat "$TABSMITH_HOME/settings.json" >> "$OUT" 2>&1
rm -rf "$SB"
