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
    [ "${_ts_wrapped:-0}" = 1 ] && [ "$PS1" = "$_ts_last1" ] && return $st   # already wrapped this prompt
    _ts_u1=$PS1; _ts_u2=$PS2
    PS1="\[\e]697;StartPrompt\a\]$PS1\[\e]697;EndPrompt\a\e]697;NewCmd=$TABSMITH_SESSION_ID\a\]"
    PS2="\[\e]697;StartPrompt\a\]$PS2\[\e]697;EndPrompt\a\]"
    _ts_last1=$PS1; _ts_last2=$PS2; _ts_wrapped=1
    _ts_install
    return $st
  }
  # shellcheck disable=SC2128,SC2178  # PROMPT_COMMAND is either a string or an array
  _ts_install() {   # PROMPT_COMMAND = _ts_first ; <user commands> ; _ts_last   (string or bash 5.1 array)
    if [[ "$(declare -p PROMPT_COMMAND 2>/dev/null)" == "declare -a"* ]]; then
      [[ ${PROMPT_COMMAND[0]} == _ts_first && ${PROMPT_COMMAND[-1]} == _ts_last ]] && return
      local pc=() c; for c in "${PROMPT_COMMAND[@]}"; do [[ $c == _ts_first || $c == _ts_last ]] || pc+=("$c"); done
      PROMPT_COMMAND=(_ts_first "${pc[@]}" _ts_last)
    else   # drop every `_ts_first`/`_ts_last` command, whatever separates it (';' or newline, e.g. bash-preexec)
      local s=${PROMPT_COMMAND:-} kept='' cmd sep
      while [ -n "$s" ]; do
        cmd=${s%%[;$'\n']*}; s=${s:${#cmd}}; sep=${s:0:1}; s=${s:1}
        [[ ${cmd//[[:space:]]/} == _ts_first || ${cmd//[[:space:]]/} == _ts_last ]] || kept+=$cmd$sep
      done
      kept=${kept#"${kept%%[![:space:];]*}"}; kept=${kept%"${kept##*[![:space:];]}"}   # ';;' is a syntax error
      kept="_ts_first;${kept:+$kept;}_ts_last"
      [[ $PROMPT_COMMAND == "$kept" ]] || PROMPT_COMMAND=$kept
    fi
  }
  PS0="\e]697;OSCLock=$TABSMITH_SESSION_ID\a\e]697;PreExec\a${PS0:-}"   # bash 4.4+ prints PS0 before running a command
  _ts_install
fi
