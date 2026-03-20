#!/usr/bin/env bash
set -euo pipefail

REPO="azjgard/diaper"
INSTALL_DIR="$HOME/.diaper/bin"

# Detect platform and architecture
OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"

case "$OS" in
  darwin) OS="darwin" ;;
  linux)  OS="linux" ;;
  *)      echo "Unsupported OS: $OS"; exit 1 ;;
esac

case "$ARCH" in
  arm64|aarch64) ARCH="arm64" ;;
  x86_64)        ARCH="amd64" ;;
  *)             echo "Unsupported architecture: $ARCH"; exit 1 ;;
esac

ASSET="diaper-${OS}-${ARCH}.zip"

echo "Fetching latest release from $REPO..."

# Try /releases/latest first (skips pre-releases), fall back to first entry in /releases
DOWNLOAD_URL=$(curl -sL "https://api.github.com/repos/${REPO}/releases/latest" \
  | grep "browser_download_url.*${ASSET}" \
  | head -1 \
  | cut -d '"' -f 4 || true)

if [ -z "$DOWNLOAD_URL" ]; then
  # Fall back to most recent release (includes pre-releases)
  DOWNLOAD_URL=$(curl -sL "https://api.github.com/repos/${REPO}/releases" \
    | grep "browser_download_url.*${ASSET}" \
    | head -1 \
    | cut -d '"' -f 4 || true)
fi

if [ -z "$DOWNLOAD_URL" ]; then
  echo "Could not find asset '${ASSET}' in any release."
  echo "Available assets:"
  curl -sL "https://api.github.com/repos/${REPO}/releases" \
    | grep "browser_download_url" \
    | cut -d '"' -f 4 || true
  exit 1
fi

TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

echo "Downloading ${ASSET}..."
curl -sL "$DOWNLOAD_URL" -o "${TMPDIR}/${ASSET}"

echo "Extracting..."
unzip -qo "${TMPDIR}/${ASSET}" -d "$TMPDIR"

echo "Installing to ${INSTALL_DIR}..."
mkdir -p "$INSTALL_DIR"
mv "${TMPDIR}/diaper" "${INSTALL_DIR}/diaper"
chmod +x "${INSTALL_DIR}/diaper"

echo ""
echo "diaper installed to ${INSTALL_DIR}/diaper"
echo ""

# Check if already in PATH
if echo "$PATH" | tr ':' '\n' | grep -qx "$INSTALL_DIR"; then
  echo "✓ ${INSTALL_DIR} is already in your PATH."
  echo ""
  echo "Run 'diaper check' to get started."
else
  echo "Add diaper to your PATH by running one of the following:"
  echo ""
  echo "  # bash"
  echo "  echo 'export PATH=\"\$HOME/.diaper/bin:\$PATH\"' >> ~/.bashrc && source ~/.bashrc"
  echo ""
  echo "  # zsh"
  echo "  echo 'export PATH=\"\$HOME/.diaper/bin:\$PATH\"' >> ~/.zshrc && source ~/.zshrc"
  echo ""
  echo "Then run 'diaper check' to get started."
fi
