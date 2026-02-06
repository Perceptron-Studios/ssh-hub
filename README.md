# ssh-hub

MCP server that gives Claude Code full access to remote machines over SSH. Connect to multiple servers simultaneously and use remote file operations, shell commands, and directory sync — all through the MCP tool interface.

## Prerequisites

- **Rust toolchain** (1.70+) — install via [rustup](https://rustup.rs/)
- **SSH agent** or **SSH key** (`~/.ssh/id_ed25519`, `id_rsa`, or `id_ecdsa`)
- A running `ssh-agent` with loaded keys if using agent auth (`ssh-add`)

## Install

```bash
cargo install --path .
```

## Quick start

```bash
# Add a server
ssh-hub add staging deploy@staging.example.com:/var/www/app

# Or use interactive setup (picks auth method, tests connection)
ssh-hub setup staging --connection deploy@staging.example.com:/var/www/app

# Register as MCP server in Claude Code
claude mcp add remote -- ssh-hub
```

## CLI commands

| Command | Description |
|---------|-------------|
| `ssh-hub` | Start MCP server on stdio (used by Claude Code) |
| `ssh-hub setup <name> --connection user@host:/path` | Interactive setup with auth selection and connection test |
| `ssh-hub add <name> user@host:/path` | Add server to config (non-interactive) |
| `ssh-hub remove <name>` | Remove server from config |
| `ssh-hub list` | List configured servers |

Options: `-v` for verbose logging, `-i <path>` for identity file, `-p <port>` for custom port.

## Authentication

Authentication is SSH-key based. No passwords are stored or transmitted.

| Method | Config value | Description |
|--------|-------------|-------------|
| **Auto** (default) | `auto` | Tries SSH agent, then explicit identity file, then default keys (`~/.ssh/id_ed25519`, `id_rsa`, `id_ecdsa`) |
| **Key** | `key` | Uses the identity file specified with `-i` |
| **Agent** | `agent` | Delegates signing to `ssh-agent` via `SSH_AUTH_SOCK` |

## MCP tools

Once registered, Claude Code gets these tools:

### Connection management

- **`list_servers`** — Show configured and connected servers
- **`connect`** — Connect to a configured server by name, or ad-hoc via connection string
- **`disconnect`** — Disconnect from a server

### Remote operations

All remote tools take a `server` parameter to target a specific connection.

- **`remote_bash`** — Execute shell commands (with optional timeout and background mode)
- **`remote_read`** — Read file contents
- **`remote_write`** — Write content to a file
- **`remote_edit`** — Edit a file using string replacement
- **`remote_glob`** — Find files matching a glob pattern

### Sync

- **`sync_status`** — Compare local and remote directories (git-aware)
- **`sync_push`** — Push local files to remote
- **`sync_pull`** — Pull remote files to local

## Configuration

Server configs are stored in `~/.config/ssh-hub/servers.toml`:

```toml
[servers.staging]
host = "staging.example.com"
user = "deploy"
port = 2222
remote_path = "/var/www/app"
identity = "~/.ssh/id_staging"
auth = "key"

[servers.prod]
host = "prod.example.com"
user = "deploy"
remote_path = "/var/www/app"
auth = "agent"
```

## License

MIT
