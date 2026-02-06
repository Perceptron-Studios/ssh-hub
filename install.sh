#!/usr/bin/env bash
set -euo pipefail

echo "=== ssh-hub installer ==="
echo ""

# ── Prerequisites ──────────────────────────────────────────────────────

if ! command -v rustc &>/dev/null || ! command -v cargo &>/dev/null; then
    echo "Error: Rust toolchain not found."
    echo ""
    echo "Install it with:"
    echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    echo ""
    echo "Then restart your shell and re-run this script."
    exit 1
fi

echo "Rust toolchain found: $(rustc --version)"

# ── Build & install ────────────────────────────────────────────────────

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo ""
echo "Building and installing ssh-hub..."
cargo install --path "$SCRIPT_DIR"

if ! command -v ssh-hub &>/dev/null; then
    echo ""
    echo "Error: ssh-hub binary not found after install."
    echo ""
    echo "Add this to your shell profile (~/.zshrc or ~/.bashrc):"
    echo "  export PATH=\"\$HOME/.cargo/bin:\$PATH\""
    echo ""
    echo "Then restart your shell and re-run this script."
    exit 1
fi

echo "Installed: $(ssh-hub --version)"
echo ""
echo "Next steps:"
echo "  1. Add a remote server:  ssh-hub add myserver user@host:/path"
echo "  2. Add the MCP to a project:  ssh-hub mcp-install /path/to/project"
