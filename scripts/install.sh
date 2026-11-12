#!/bin/sh
# tabsmith installer for macOS and Linux (WSL and Alpine included): installs the release binary as
# ~/.local/bin/tabsmith, where it then updates itself in the background.
#
#   curl -fsSL https://github.com/PedroLaRosa/tabsmith/releases/latest/download/install.sh | sh
#   curl -fsSL https://github.com/PedroLaRosa/tabsmith/releases/latest/download/install.sh | sh -s 0.2.0
#
# POSIX sh, so `| sh` works without bash. Everything runs from `main` on the last line: a download cut short
# defines functions and runs nothing.
set -eu

RELEASES=https://github.com/PedroLaRosa/tabsmith/releases

say() {
    printf '%s\n' "$*"
}

fail() {
    printf 'tabsmith install: %s\n' "$*" >&2
    exit 1
}

# fetch URL FILE: HTTPS only, retried, failing on an HTTP error instead of saving the error page.
fetch() {
    curl --proto '=https' --tlsv1.2 -fsSL --retry 3 -o "$2" "$1" || fail "could not download $1"
}

# Sets os and arch to the release's names for this machine.
detect_platform() {
    case "$(uname -s)" in
        Darwin) os=darwin ;;
        Linux) os=linux ;;
        *) fail "$(uname -s) is not supported here; on Windows, run install.ps1 in PowerShell" ;;
    esac
    case "$(uname -m)" in
        x86_64 | amd64) arch=x64 ;;
        arm64 | aarch64) arch=arm64 ;;
        *) fail "the $(uname -m) architecture has no tabsmith release" ;;
    esac
    # An x64 shell under Rosetta on Apple Silicon still gets the native arm64 binary.
    if [ "$os" = darwin ] && [ "$arch" = x64 ] && [ "$(sysctl -n sysctl.proc_translated 2>/dev/null)" = 1 ]; then
        arch=arm64
    fi
}

sha256_of() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | cut -d ' ' -f 1
    else
        shasum -a 256 "$1" | cut -d ' ' -f 1
    fi
}

# The digest SHA256SUMS (file $1) lists for asset $2, in text (`<hex>  <name>`) or binary (`<hex> *<name>`) mode.
listed_digest() {
    awk -v asset="$2" '$2 == asset || $2 == "*" asset { print $1; exit }' "$1"
}

main() {
    command -v curl >/dev/null 2>&1 || fail "curl is needed to download tabsmith"
    detect_platform
    bin_dir="$HOME/.local/bin"
    mkdir -p "$bin_dir"
    # Next to the final file, so the last step is a rename on one filesystem.
    staged="$bin_dir/.tabsmith-install-$$"
    trap 'rm -f "$staged" "$staged.sums" "$staged.version"' EXIT
    trap 'exit 1' HUP INT TERM

    version=${1:-latest}
    if [ "$version" = latest ]; then
        fetch "$RELEASES/latest/download/VERSION" "$staged.version"
        version=$(tr -d ' \r\n' <"$staged.version")
    fi
    version=${version#v}
    asset="tabsmith-$os-$arch"

    say "Downloading tabsmith $version for $os-$arch..."
    fetch "$RELEASES/download/v$version/$asset" "$staged"
    fetch "$RELEASES/download/v$version/SHA256SUMS" "$staged.sums"
    expected=$(listed_digest "$staged.sums" "$asset")
    [ -n "$expected" ] || fail "the SHA256SUMS of tabsmith $version does not list $asset"
    actual=$(sha256_of "$staged")
    [ "$actual" = "$expected" ] || fail "$asset does not match its checksum (expected $expected, got $actual)"

    chmod 755 "$staged"
    # Every new terminal runs this file: one that does not start never takes the old one's place.
    installed_version=$("$staged" --version) || fail "the downloaded tabsmith does not run on this machine"
    first_install=true
    [ -e "$bin_dir/tabsmith" ] && first_install=false
    # A rename, never a write into the old file: running sessions keep it, and macOS kills a signed binary that is
    # overwritten in place.
    mv -f "$staged" "$bin_dir/tabsmith"
    say "Installed $installed_version at $bin_dir/tabsmith"

    case ":$PATH:" in
        *":$bin_dir:"*) ;;
        *)
            say ""
            say "$bin_dir is not on your PATH. Autocompletion works without it; to run tabsmith by name, add:"
            say "  export PATH=\"\$HOME/.local/bin:\$PATH\""
            ;;
    esac

    if [ "$first_install" = false ]; then
        return
    fi
    say ""
    # stdin is this script when piped into sh, so setup asks its questions on the terminal itself. The probe runs in
    # a subshell: a failed redirection of a special builtin would end the whole script.
    if (: </dev/tty) 2>/dev/null; then
        "$bin_dir/tabsmith" setup </dev/tty || say "Run: tabsmith setup"
    else
        say "Run: tabsmith setup"
    fi
}

main "$@"
