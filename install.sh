#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SSH_HUB_BIN="ssh-hub"

usage() {
    echo "Usage: install.sh <command>"
    echo ""
    echo "Commands:"
    echo "  install    Build and install the ssh-hub binary"
    echo "  add-mcp    Add ssh-hub MCP config to a project directory"
    echo ""
    echo "Run 'install' once, then 'add-mcp' for each project that needs it."
}

# ── install: build and install the binary ─────────────────────────────

cmd_install() {
    echo "=== ssh-hub installer ==="
    echo ""

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
    echo "Next: run './install.sh add-mcp' to configure a project."
}

# ── add-mcp: write MCP config into a project directory ────────────────

cmd_add_mcp() {
    if ! command -v ssh-hub &>/dev/null; then
        echo "Error: ssh-hub is not installed. Run './install.sh install' first."
        exit 1
    fi

    TARGET_DIR="${1:-}"
    if [ -z "$TARGET_DIR" ]; then
        read -rp "Project directory for MCP configs [$(pwd)]: " TARGET_DIR
        TARGET_DIR="${TARGET_DIR:-$(pwd)}"
    fi

    # Expand tilde
    TARGET_DIR="${TARGET_DIR/#\~/$HOME}"

    if [ ! -d "$TARGET_DIR" ]; then
        echo "Error: directory '$TARGET_DIR' does not exist."
        exit 1
    fi

    # ── Claude Code (.mcp.json) ───────────────────────────────────────
    MCP_JSON="$TARGET_DIR/.mcp.json"

    echo "Configuring Claude Code MCP..."

    if [ -f "$MCP_JSON" ]; then
        python3 -c "
import json

with open('$MCP_JSON', 'r') as f:
    config = json.load(f)

config.setdefault('mcpServers', {})
config['mcpServers']['ssh-hub'] = {
    'command': '$SSH_HUB_BIN',
    'args': []
}

with open('$MCP_JSON', 'w') as f:
    json.dump(config, f, indent=2)
    f.write('\n')
"
        echo "  Updated $MCP_JSON (merged with existing servers)"
    else
        cat > "$MCP_JSON" << EOF
{
  "mcpServers": {
    "ssh-hub": {
      "command": "$SSH_HUB_BIN",
      "args": []
    }
  }
}
EOF
        echo "  Created $MCP_JSON"
    fi

    # ── Codex (.codex/config.toml) ────────────────────────────────────
    CODEX_DIR="$TARGET_DIR/.codex"
    CODEX_TOML="$CODEX_DIR/config.toml"

    echo "Configuring Codex MCP..."

    mkdir -p "$CODEX_DIR"

    CODEX_BLOCK="[mcp_servers.ssh-hub]
command = \"$SSH_HUB_BIN\"
args = []"

    if [ -f "$CODEX_TOML" ]; then
        if grep -q '\[mcp_servers\.ssh-hub\]' "$CODEX_TOML"; then
            python3 -c "
import re

with open('$CODEX_TOML', 'r') as f:
    content = f.read()

pattern = r'\[mcp_servers\.ssh-hub\].*?(?=\n\[|$)'
replacement = '''[mcp_servers.ssh-hub]
command = \"$SSH_HUB_BIN\"
args = []'''

content = re.sub(pattern, replacement, content, flags=re.DOTALL)

with open('$CODEX_TOML', 'w') as f:
    f.write(content)
"
            echo "  Updated $CODEX_TOML (replaced existing ssh-hub entry)"
        else
            printf '\n%s\n' "$CODEX_BLOCK" >> "$CODEX_TOML"
            echo "  Updated $CODEX_TOML (appended ssh-hub entry)"
        fi
    else
        printf '%s\n' "$CODEX_BLOCK" > "$CODEX_TOML"
        echo "  Created $CODEX_TOML"
    fi

    echo ""
    echo "MCP configs set up at: $TARGET_DIR"
    echo ""
    echo "Next steps:"
    echo "  1. Add a remote server:  ssh-hub setup myserver --connection user@host:/path"
    echo "  2. Start Claude Code or Codex from $TARGET_DIR"
    echo "  3. The ssh-hub MCP tools will be available automatically"
}

# ── Dispatch ──────────────────────────────────────────────────────────

case "${1:-}" in
    install)
        cmd_install
        ;;
    add-mcp)
        shift
        cmd_add_mcp "$@"
        ;;
    *)
        usage
        exit 1
        ;;
esac
