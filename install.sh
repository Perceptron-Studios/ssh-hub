#!/usr/bin/env bash
set -euo pipefail

REPO="Perceptron-Studios/ssh-hub"
INSTALL_DIR="${SSH_HUB_INSTALL_DIR:-$HOME/.cargo/bin}"

echo "=== ssh-hub installer ==="
echo ""

# ── Detect platform ───────────────────────────────────────────────────

OS=$(uname -s)
ARCH=$(uname -m)

case "${OS}-${ARCH}" in
    Darwin-arm64)   TARGET="aarch64-apple-darwin" ;;
    Darwin-x86_64)  TARGET="x86_64-apple-darwin" ;;
    Linux-x86_64)   TARGET="x86_64-unknown-linux-gnu" ;;
    Linux-aarch64)  TARGET="aarch64-unknown-linux-gnu" ;;
    *)
        echo "Error: Unsupported platform: ${OS}-${ARCH}"
        echo ""
        echo "Supported platforms:"
        echo "  macOS: arm64 (Apple Silicon), x86_64 (Intel)"
        echo "  Linux: x86_64, aarch64"
        exit 1
        ;;
esac

echo "Platform: ${OS} ${ARCH} (${TARGET})"

# ── Fetch latest release ──────────────────────────────────────────────

echo "Fetching latest release..."

LATEST=$(curl -sL "https://api.github.com/repos/${REPO}/releases/latest" \
    | grep '"tag_name"' | head -1 | cut -d'"' -f4)

if [ -z "${LATEST}" ]; then
    echo "Error: Could not determine latest release."
    echo ""
    echo "You can install from source instead:"
    echo "  cargo install --path ."
    exit 1
fi

ASSET="ssh-hub-${LATEST}-${TARGET}.tar.gz"
URL="https://github.com/${REPO}/releases/download/${LATEST}/${ASSET}"

echo "Latest version: ${LATEST}"

# ── Download and install ──────────────────────────────────────────────

echo ""
echo "Downloading ${ASSET}..."

mkdir -p "${INSTALL_DIR}"

if ! curl -fsSL "${URL}" | tar xz -C "${INSTALL_DIR}"; then
    echo ""
    echo "Error: Download failed. The release may not have a binary for ${TARGET}."
    echo ""
    echo "You can install from source instead:"
    echo "  cargo install --path ."
    exit 1
fi

chmod +x "${INSTALL_DIR}/ssh-hub"

echo ""
echo "Installed: ${INSTALL_DIR}/ssh-hub (${LATEST})"

# ── Verify PATH ───────────────────────────────────────────────────────

if ! command -v ssh-hub &>/dev/null; then
    echo ""
    echo "Note: ${INSTALL_DIR} is not in your PATH."
    echo ""
    echo "Add this to your shell profile (~/.zshrc or ~/.bashrc):"
    echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
    echo ""
    echo "Then restart your shell."
else
    echo ""
    echo "Next steps:"
    echo "  1. Add a remote server:    ssh-hub add myserver user@host:/path"
    echo "  2. Register MCP in project: ssh-hub mcp-install /path/to/project"
fi
