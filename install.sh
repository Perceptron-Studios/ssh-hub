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

# Resolve absolute path to the binary (MCP servers need it since they don't inherit shell PATH)
if command -v ssh-hub &>/dev/null; then
    SSH_HUB_BIN="$(command -v ssh-hub)"
else
    SSH_HUB_BIN="$HOME/.cargo/bin/ssh-hub"
fi

if [ ! -x "$SSH_HUB_BIN" ]; then
    echo ""
    echo "Error: ssh-hub binary not found after install."
    echo ""
    echo "Add this to your shell profile (~/.zshrc or ~/.bashrc):"
    echo "  export PATH=\"\$HOME/.cargo/bin:\$PATH\""
    echo ""
    echo "Then restart your shell and re-run this script."
    exit 1
fi

echo "Installed: $($SSH_HUB_BIN --version) at $SSH_HUB_BIN"

# ── MCP config target ─────────────────────────────────────────────────

echo ""
read -rp "Project directory for MCP configs [$(pwd)]: " TARGET_DIR
TARGET_DIR="${TARGET_DIR:-$(pwd)}"

# Expand tilde
TARGET_DIR="${TARGET_DIR/#\~/$HOME}"

if [ ! -d "$TARGET_DIR" ]; then
    echo "Error: directory '$TARGET_DIR' does not exist."
    exit 1
fi

# ── Claude Code (.mcp.json) ───────────────────────────────────────────

MCP_JSON="$TARGET_DIR/.mcp.json"

echo ""
echo "Configuring Claude Code MCP..."

if [ -f "$MCP_JSON" ]; then
    # Merge into existing file using python3
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

# ── Codex (.codex/config.toml) ────────────────────────────────────────

CODEX_DIR="$TARGET_DIR/.codex"
CODEX_TOML="$CODEX_DIR/config.toml"

echo ""
echo "Configuring Codex MCP..."

mkdir -p "$CODEX_DIR"

CODEX_BLOCK="[mcp_servers.ssh-hub]
command = \"$SSH_HUB_BIN\"
args = []"

if [ -f "$CODEX_TOML" ]; then
    if grep -q '\[mcp_servers\.ssh-hub\]' "$CODEX_TOML"; then
        # Replace existing block: from [mcp_servers.ssh-hub] to next section or EOF
        python3 -c "
import re

with open('$CODEX_TOML', 'r') as f:
    content = f.read()

# Replace the ssh-hub section (up to next [section] or end of file)
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
        # Append to existing file
        printf '\n%s\n' "$CODEX_BLOCK" >> "$CODEX_TOML"
        echo "  Updated $CODEX_TOML (appended ssh-hub entry)"
    fi
else
    printf '%s\n' "$CODEX_BLOCK" > "$CODEX_TOML"
    echo "  Created $CODEX_TOML"
fi

# ── Summary ───────────────────────────────────────────────────────────

echo ""
echo "=== Done ==="
echo ""
echo "ssh-hub is installed and MCP configs are set up at: $TARGET_DIR"
echo ""
echo "Next steps:"
echo "  1. Add a remote server:  ssh-hub setup myserver --connection user@host:/path"
echo "  2. Start Claude Code or Codex from $TARGET_DIR"
echo "  3. The ssh-hub MCP tools will be available automatically"
