#!/bin/sh
# Draw the brand icons listed in assets/icons/simple-icons.txt from simple-icons, pinned below, into assets/icons.
# Usage: scripts/brand-icons.sh   (the Amazon Q PNGs are never touched; a second run changes nothing)
set -eu
cd "$(dirname "$0")/.."

SIMPLE_ICONS_VERSION=16.38.1
SIMPLE_ICONS_SHA256=8ad81af0009aa0935df4411080feb4547b72e13458bbcf4d803b259b1f078804

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
tarball="$tmp/simple-icons.tgz"
curl -fsSL -o "$tarball" "https://registry.npmjs.org/simple-icons/-/simple-icons-$SIMPLE_ICONS_VERSION.tgz"

if command -v sha256sum >/dev/null 2>&1; then
    actual="$(sha256sum "$tarball" | cut -d ' ' -f 1)"
else
    actual="$(shasum -a 256 "$tarball" | cut -d ' ' -f 1)"
fi
if [ "$actual" != "$SIMPLE_ICONS_SHA256" ]; then
    echo "simple-icons $SIMPLE_ICONS_VERSION: sha256 is $actual, expected $SIMPLE_ICONS_SHA256" >&2
    exit 1
fi

tar xzf "$tarball" -C "$tmp"
cargo run --locked --release --quiet --manifest-path tools/brand-icons/Cargo.toml -- \
    "$tmp/package" assets/icons/simple-icons.txt assets/icons/template.rgba assets/icons
