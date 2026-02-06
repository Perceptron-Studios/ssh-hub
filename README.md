# ssh-hub

MCP server that gives AI coding agents full access to remote machines over SSH. Connect to multiple servers simultaneously and use remote file operations, shell commands, and directory sync — all through the MCP tool interface.

## Prerequisites

- **[Rust toolchain](https://rustup.rs/)** (1.70+)
- **SSH agent** with loaded keys (`ssh-add`) or SSH key files (`~/.ssh/id_ed25519`, etc.)

## Install

```bash
cargo install --git https://github.com/Perceptron-Studios/ssh-hub.git
```

### Update

```bash
ssh-hub update
```

## Quick start

```bash
# 1. Add a remote server
ssh-hub add myserver user@host:/path/to/project

# 2. Register as MCP in your project
ssh-hub mcp-install /path/to/project
```

### Connection string formats

```
user@host                  # defaults to port 22, path ~
user@host:/path            # explicit path
user@host:2222             # custom port, path ~
user@host:2222:/path       # custom port and path
```

### Identity files

For passphrase-protected keys, pass `-i` during `add` — ssh-hub will load the key into your SSH agent:

```bash
ssh-hub add myserver user@host -i ~/.ssh/my_key
# Prompts for passphrase via ssh-add, then tests the connection
```

## CLI commands

| Command | Description |
|---------|-------------|
| `ssh-hub` | Start MCP server on stdio (used by MCP clients) |
| `ssh-hub add <name> <connection>` | Add a server (tests connection, then saves) |
| `ssh-hub remove <name>` | Remove a server from config |
| `ssh-hub list` | List configured servers |
| `ssh-hub mcp-install [directory]` | Register ssh-hub as MCP server in a project |
| `ssh-hub update` | Update to the latest release |

**Options:** `-v` verbose logging, `-i <path>` identity file, `-p <port>` port override.

**`mcp-install` flags:** `--claude` (`.mcp.json` only), `--codex` (`.codex/config.toml` only). Defaults to both.

## Authentication

All authentication is SSH-key based. No passwords are stored or transmitted.

The default method is **Auto**, which tries in order:

1. **SSH agent** — keys loaded via `ssh-add`, signing delegated to `ssh-agent`
2. **Identity file** — key specified with `-i` during `add`
3. **Default keys** — `~/.ssh/id_ed25519`, `id_rsa`, `id_ecdsa`

RSA keys are automatically negotiated with SHA-256/SHA-512 signatures (modern servers reject legacy SHA-1).

## MCP tools

Once connected, the following tools are available to any MCP client:

### Connection management

- **`connect`** — Connect to a configured server (or ad-hoc via connection string)
- **`disconnect`** — Disconnect from a server
- **`list_servers`** — Show configured and connected servers

### Remote operations

All remote tools take a `server` parameter to target a specific connection.

- **`remote_bash`** — Execute shell commands (with optional timeout and background mode)
- **`remote_read`** — Read file contents (with offset/limit for large files)
- **`remote_write`** — Write content to a file
- **`remote_edit`** — Edit a file using string replacement
- **`remote_glob`** — Find files matching a glob pattern

### Sync

- **`sync_status`** — Compare local and remote directories (git-aware when available)
- **`sync_push`** — Push local files to remote
- **`sync_pull`** — Pull remote files to local

## Configuration

Server configs are stored in `~/.config/ssh-hub/servers.toml` (macOS: `~/Library/Application Support/ssh-hub/servers.toml`):

```toml
[servers.staging]
host = "staging.example.com"
user = "deploy"
port = 2222
remote_path = "/var/www/app"
identity = "~/.ssh/id_staging"
auth = "auto"

[servers.prod]
host = "prod.example.com"
user = "deploy"
remote_path = "~"
auth = "auto"
```

## License

MIT
