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

TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

# Detect host architecture
HOST_ARCH="$(uname -m)"
case "$HOST_ARCH" in
  arm64|aarch64) ARCH="arm64" ;;
  x86_64)        ARCH="amd64" ;;
  *)             echo "Unsupported architecture: $HOST_ARCH"; exit 1 ;;
esac

# Build macOS binary
echo "Building macOS release binary..."
make release

DARWIN_ASSET="diaper-darwin-${ARCH}.zip"
cp target/release/diaper "$TMPDIR/diaper"
(cd "$TMPDIR" && zip "$DARWIN_ASSET" diaper && rm diaper)

# Build Linux binary via Docker
echo "Building Linux release binary..."
make release-linux

LINUX_ASSET="diaper-linux-${ARCH}.zip"
cp target/linux-release/diaper "$TMPDIR/diaper"
(cd "$TMPDIR" && zip "$LINUX_ASSET" diaper && rm diaper)

echo "Creating GitHub release ${TAG}..."
gh release create "$TAG" \
  --title "$TAG" \
  --notes "$NOTES" \
  "${TMPDIR}/${DARWIN_ASSET}" \
  "${TMPDIR}/${LINUX_ASSET}"

echo ""
echo "Release ${TAG} created: https://github.com/azjgard/diaper/releases/tag/${TAG}"
