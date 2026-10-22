#!/usr/bin/env bash
# Re-bundle the Fig completion specs (@withfig/autocomplete) into assets/specs.tar.zst.
# Usage: scripts/update-specs.sh [version]   (default: assets/SPECS_VERSION)
set -euo pipefail
cd "$(dirname "$0")/.."
ver="${1:-$(cat assets/SPECS_VERSION)}"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
curl -fsSL "https://registry.npmjs.org/@withfig/autocomplete/-/autocomplete-$ver.tgz" | tar xz -C "$tmp"
tar -C "$tmp/package/build" -cf - . | zstd -19 -q -o assets/specs.tar.zst -f
cp "$tmp/package/LICENSE" assets/SPECS_LICENSE
printf '%s\n' "$ver" > assets/SPECS_VERSION
echo "bundled @withfig/autocomplete@$ver ($(du -h assets/specs.tar.zst | cut -f1))"
