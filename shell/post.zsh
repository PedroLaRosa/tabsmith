# tabsmith post (zsh): OSC 697 markers around the prompt + precmd/preexec hooks (SPEC §5.4.2).
if [[ -n "${ZSH_NAME:-}" && -n "${TABSMITH_SESSION_ID:-}" && -z "${_TS_POST_LOADED:-}" ]]; then
  _TS_POST_LOADED=1
  _ts_osc() { builtin printf '\e]697;%s\a' "$1"; }
  _ts_restore() {
    if [[ ${_ts_wrapped:-0} == 1 ]]; then
      PS1=$_ts_u1; PS2=$_ts_u2; RPS1=$_ts_ur1; RPS2=$_ts_ur2; _ts_wrapped=0
    fi
  }
  _ts_preexec() { _ts_restore; _ts_osc "OSCLock=$TABSMITH_SESSION_ID"; _ts_osc PreExec; }
  _ts_precmd() {
    local st=$?
    _ts_restore                                   # Ctrl-C at the prompt: preexec never ran
    _ts_osc "OSCUnlock=$TABSMITH_SESSION_ID"; _ts_osc "Dir=$PWD"; _ts_osc "Shell=zsh"
    _ts_osc "ShellPath=${TABSMITH_SHELL:-$SHELL}"; _ts_osc "PID=$$"; _ts_osc "ExitCode=$st"
    _ts_osc "TTY=$TTY"; _ts_osc "User=${USER:-root}"
    (( ${+functions[_zsh_autosuggest_accept]} )) && _ts_osc "ZshAutosuggestionColor=${ZSH_AUTOSUGGEST_HIGHLIGHT_STYLE:-fg=8}"
    _ts_u1=$PS1 _ts_u2=$PS2 _ts_ur1=$RPS1 _ts_ur2=$RPS2
    local sp=$'%{\e]697;StartPrompt\a%}' ep=$'%{\e]697;EndPrompt\a%}'
    local nc=$'%{\e]697;NewCmd='"$TABSMITH_SESSION_ID"$'\a%}'
    PS1="$sp$PS1$ep$nc"; PS2="$sp$PS2$ep"
    [[ -n $RPS1 ]] && RPS1="$sp$RPS1$ep"; [[ -n $RPS2 ]] && RPS2="$sp$RPS2$ep"
    _ts_wrapped=1
    _ts_reset_hooks
    ( {{BIN}} _ pre-cmd --alias "$(builtin alias)" >/dev/null 2>&1 & )
  }
  _ts_reset_hooks() {   # precmd LAST (wraps the final PS1), preexec FIRST; re-asserted every prompt
    [[ ${precmd_functions[-1]} == _ts_precmd ]] || precmd_functions=(${precmd_functions:#_ts_precmd} _ts_precmd)
    [[ ${preexec_functions[1]} == _ts_preexec ]] || preexec_functions=(_ts_preexec ${preexec_functions:#_ts_preexec})
  }
  _ts_reset_hooks
fi
