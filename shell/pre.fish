# tabsmith pre (fish): replace this interactive shell with the tabsmith PTY wrapper (SPEC §5.4.4).
if test "$SHOULD_TABSMITH_LAUNCH" = 0; and status is-interactive; and test -t 0; and test -t 1
    set -l login 0; status is-login; and set login 1
    set -gx TABSMITH_SHELL (status fish-path)
    set -gx TABSMITH_IS_LOGIN_SHELL $login
    # fish's exec has no -a; argv0 is cosmetic. `fish --private` stays private inside the wrapper.
    # ponytail: only --private is forwarded; -C/--init-command, --features and -d are not.
    exec {{BIN}} _ pty -- (test -n "$fish_private_mode"; and echo --private)
end
