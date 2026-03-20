#!/usr/bin/env bash
set -euo pipefail

if [ -z "${1:-}" ]; then
  echo "Usage: ./scripts/release.sh <version>"
  echo "Example: ./scripts/release.sh v0.2.0-beta"
  exit 1
fi

VERSION="$1"

echo "Building release binary..."
make release

TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

# Create zip asset matching install.sh naming convention
ASSET="diaper-darwin-arm64.zip"
cp target/release/diaper "$TMPDIR/diaper"
(cd "$TMPDIR" && zip "$ASSET" diaper)

echo "Creating GitHub release ${VERSION}..."
gh release create "$VERSION" \
  --title "$VERSION" \
  --generate-notes \
  "${TMPDIR}/${ASSET}"

echo ""
echo "Release ${VERSION} created: https://github.com/azjgard/diaper/releases/tag/${VERSION}"
