#!/usr/bin/env bash
set -euo pipefail

if [ -z "${1:-}" ]; then
  echo "Usage: ./scripts/release.sh <version>"
  echo "Example: ./scripts/release.sh v0.2.0-beta"
  exit 1
fi

VERSION="$1"

# Find the previous tag for generating release notes
PREV_TAG=$(git describe --tags --abbrev=0 2>/dev/null || true)

# Create local tag
echo "Creating local tag ${VERSION}..."
git tag "$VERSION"

# Push tag to remote
echo "Pushing tag ${VERSION} to remote..."
git push origin "$VERSION"

# Build release notes from commit log
if [ -n "$PREV_TAG" ]; then
  echo "Generating release notes from ${PREV_TAG}..${VERSION}..."
  NOTES=$(git log --format="- %s" "${PREV_TAG}..${VERSION}")
else
  echo "No previous tag found, generating release notes from all commits..."
  NOTES=$(git log --format="- %s" "$VERSION")
fi

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
  --notes "$NOTES" \
  "${TMPDIR}/${ASSET}"

echo ""
echo "Release ${VERSION} created: https://github.com/azjgard/diaper/releases/tag/${VERSION}"
