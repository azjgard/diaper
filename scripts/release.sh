#!/usr/bin/env bash
set -euo pipefail

if [ -z "${1:-}" ]; then
  echo "Usage: ./scripts/release.sh <version>"
  echo "Example: ./scripts/release.sh 0.2.0-beta"
  exit 1
fi

VERSION="$1"
TAG="v${VERSION}"

# Bump version in Cargo.toml
echo "Bumping Cargo.toml version to ${VERSION}..."
sed -i '' "s/^version = \".*\"/version = \"${VERSION}\"/" Cargo.toml

# Commit the version bump
git add Cargo.toml
git commit -m "Bump version to ${VERSION}"

# Find the previous tag for generating release notes
PREV_TAG=$(git describe --tags --abbrev=0 2>/dev/null || true)

# Build release notes from commit log
if [ -n "$PREV_TAG" ]; then
  echo "Generating release notes from ${PREV_TAG}..HEAD..."
  NOTES=$(git log --format="- %s" "${PREV_TAG}..HEAD")
else
  echo "No previous tag found, generating release notes from all commits..."
  NOTES=$(git log --format="- %s")
fi

# Create local tag
echo "Creating local tag ${TAG}..."
git tag "$TAG"

# Push commit and tag to remote
echo "Pushing to remote..."
git push
git push origin "$TAG"

echo "Building release binary..."
make release

TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

# Create zip asset matching install.sh naming convention
ASSET="diaper-darwin-arm64.zip"
cp target/release/diaper "$TMPDIR/diaper"
(cd "$TMPDIR" && zip "$ASSET" diaper)

echo "Creating GitHub release ${TAG}..."
gh release create "$TAG" \
  --title "$TAG" \
  --notes "$NOTES" \
  "${TMPDIR}/${ASSET}"

echo ""
echo "Release ${TAG} created: https://github.com/azjgard/diaper/releases/tag/${TAG}"
