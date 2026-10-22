# shellcheck shell=bash
# tabsmith pre (bash, zsh): replace this interactive shell with the tabsmith PTY wrapper (SPEC §5.4.1).
if [ "${SHOULD_TABSMITH_LAUNCH:-1}" = 0 ] && [ -t 0 ] && [ -t 1 ] \
   && [ -z "${BASH_EXECUTION_STRING:-}${ZSH_EXECUTION_STRING:-}" ]; then
  case $- in *i*)
    if [ -n "${BASH_VERSION:-}" ] && { [ "${BASH_VERSINFO[0]}" -lt 4 ] || { [ "${BASH_VERSINFO[0]}" -eq 4 ] && [ "${BASH_VERSINFO[1]}" -lt 4 ]; }; }; then
      :   # bash < 4.4 unsupported (no PS0)
    else
      TABSMITH_IS_LOGIN_SHELL=0
      if { [ -n "${BASH_VERSION:-}" ] && shopt -q login_shell; } || [[ -n "${ZSH_NAME:-}" && -o login ]]; then
        TABSMITH_IS_LOGIN_SHELL=1
      fi
      export TABSMITH_SHELL="${TABSMITH_SHELL:-$SHELL}" TABSMITH_IS_LOGIN_SHELL
      exec -a "${TABSMITH_SHELL##*/} (tabsmith)" {{BIN}} _ pty
    fi ;;
  esac
fi
