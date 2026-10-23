# shellcheck shell=bash
# tabsmith post (bash >= 4.4): OSC 697 markers via PS0 (preexec) and PROMPT_COMMAND (precmd) (SPEC §5.4.3).
if [ -n "${BASH_VERSION:-}" ] && [ -n "${TABSMITH_SESSION_ID:-}" ] && [ -z "${_TS_POST_LOADED:-}" ]; then
  _TS_POST_LOADED=1
  _ts_osc() { builtin printf '\e]697;%s\a' "$1"; }
  : "${TTY:=$(tty 2>/dev/null)}"
  _ts_first() {
    _ts_st=$?
    _ts_osc "OSCUnlock=$TABSMITH_SESSION_ID"; _ts_osc "Dir=$PWD"; _ts_osc "Shell=bash"
    _ts_osc "ShellPath=${TABSMITH_SHELL:-$BASH}"; _ts_osc "PID=$$"; _ts_osc "ExitCode=$_ts_st"
    _ts_osc "TTY=$TTY"; _ts_osc "User=${USER:-root}"
    if [ "${_ts_wrapped:-0}" = 1 ]; then
      [ "$PS1" = "$_ts_last1" ] && PS1=$_ts_u1        # keep a PS1 the user changed meanwhile
      [ "$PS2" = "$_ts_last2" ] && PS2=$_ts_u2
      _ts_wrapped=0
    fi
    ( {{BIN}} _ pre-cmd --alias "$(builtin alias)" >/dev/null 2>&1 & )
    return $_ts_st                                     # user PROMPT_COMMANDs still see $?
  }
  _ts_last() {
    local st=$?
    # PS0 sends NoHistory when the next line does not reach bash's history (its number stays below this one).
    # Taken last, after a user `history -n`. ponytail: PS0 cannot see the line, so under ignoreboth a repeat of
    # the previous command is skipped too.
    case :${HISTCONTROL:-}: in *:ignorespace:* | *:ignoreboth:*) _ts_hn=$HISTCMD ;; *) _ts_hn=0 ;; esac
    # erasedups drops the older copy of a re-run line before adding it, so the number stays put: compare the newest
    # entry (with its time stamp) instead. Only erasedups users pay for the forks.
    _ts_h1=''
    [[ $_ts_hn != 0 && :$HISTCONTROL: == *:erasedups:* ]] && _ts_hn=0 _ts_h1=x$(HISTTIMEFORMAT='%s ' builtin history 1)
    [ "${_ts_wrapped:-0}" = 1 ] && [ "$PS1" = "$_ts_last1" ] && return $st   # already wrapped this prompt
    _ts_u1=$PS1; _ts_u2=$PS2
    PS1="\[\e]697;StartPrompt\a\]$PS1\[\e]697;EndPrompt\a\e]697;NewCmd=$TABSMITH_SESSION_ID\a\]"
    PS2="\[\e]697;StartPrompt\a\]$PS2\[\e]697;EndPrompt\a\]"
    _ts_last1=$PS1; _ts_last2=$PS2; _ts_wrapped=1
    _ts_install
    return $st
  }
  # $1 without its `_ts_first`/`_ts_last` commands, whatever separates them (';' or newline, e.g. bash-preexec),
  # into _ts_kept.
  _ts_strip() {
    local s=$1 kept='' cmd sep
    while [ -n "$s" ]; do
      cmd=${s%%[;$'\n']*}; s=${s:${#cmd}}; sep=${s:0:1}; s=${s:1}
      [[ ${cmd//[[:space:]]/} == _ts_first || ${cmd//[[:space:]]/} == _ts_last ]] || kept+=$cmd$sep
    done
    kept=${kept#"${kept%%[![:space:];]*}"}; _ts_kept=${kept%"${kept##*[![:space:];]}"}   # ';;' is a syntax error
  }
  # shellcheck disable=SC2128,SC2178  # PROMPT_COMMAND is either a string or an array
  _ts_install() {   # PROMPT_COMMAND = _ts_first ; <user commands> ; _ts_last   (string or bash 5.1 array)
    if [[ "$(declare -p PROMPT_COMMAND 2>/dev/null)" == "declare -a"* ]]; then
      local pc=(_ts_first) c
      for c in "${PROMPT_COMMAND[@]}"; do _ts_strip "$c"; [ -n "$_ts_kept" ] && pc+=("$_ts_kept"); done
      pc+=(_ts_last)
      [[ ${#pc[@]} == "${#PROMPT_COMMAND[@]}" && "${pc[*]}" == "${PROMPT_COMMAND[*]}" ]] || PROMPT_COMMAND=("${pc[@]}")
    elif (( BASH_VERSINFO[0] > 5 || (BASH_VERSINFO[0] == 5 && BASH_VERSINFO[1] >= 1) )); then
      # bash 5.1+: as an array, every user command still sees the command's PIPESTATUS (_ts_first clobbers it).
      _ts_strip "${PROMPT_COMMAND:-}"
      PROMPT_COMMAND=(_ts_first ${_ts_kept:+"$_ts_kept"} _ts_last)
    else
      # ponytail: bash 4.4/5.0 user commands see the PIPESTATUS of _ts_first.
      _ts_strip "${PROMPT_COMMAND:-}"
      local kept="_ts_first;${_ts_kept:+$_ts_kept;}_ts_last"
      [[ $PROMPT_COMMAND == "$kept" ]] || PROMPT_COMMAND=$kept
    fi
  }
  # The integration Ghostty, kitty and VS Code inject loads around the rc files of the shell they start, which the
  # pre block replaced with the wrapper: load it again in this one (VSCODE_INJECTION unset: no second ~/.bashrc).
  # ponytail: `code --locate-shell-integration-path` runs the VS Code CLI at every start; cache it if that is slow.
  if [ -r "${GHOSTTY_RESOURCES_DIR:-}/shell-integration/bash/ghostty.bash" ] && ! declare -F __ghostty_precmd >/dev/null; then
    builtin source "$GHOSTTY_RESOURCES_DIR/shell-integration/bash/ghostty.bash"
  fi
  if [ -n "${KITTY_SHELL_INTEGRATION:-}" ] && [ -r "${KITTY_INSTALLATION_DIR:-}/shell-integration/bash/kitty.bash" ]; then
    builtin source "$KITTY_INSTALLATION_DIR/shell-integration/bash/kitty.bash"
  fi
  if [ "${VSCODE_INJECTION:-}" = 1 ] && [ -z "${VSCODE_SHELL_INTEGRATION:-}" ] && command -v code >/dev/null; then
    VSCODE_INJECTION='' builtin source "$(code --locate-shell-integration-path bash)"
  fi
  # bash 4.4+ prints PS0 before running a command. In PS0 `\!` is the newest history number: below _ts_hn when
  # the line was not added. The expansion needs promptvars (the default).
  _ts_nh=('' $'\e]697;NoHistory\a') _ts_hn=0 _ts_ps0='' _ts_h1=''
  _ts_nhck() { [[ x$(HISTTIMEFORMAT='%s ' builtin history 1) == "$_ts_h1" ]] && _ts_osc NoHistory; }
  # shellcheck disable=SC2016  # expanded by PS0 at each command, not here
  shopt -q promptvars && _ts_ps0='${_ts_nh[\! < _ts_hn]}${_ts_h1:+$(_ts_nhck)}'
  PS0="\e]697;OSCLock=$TABSMITH_SESSION_ID\a$_ts_ps0\e]697;PreExec\a${PS0:-}"
  unset _ts_ps0
  _ts_install
fi
