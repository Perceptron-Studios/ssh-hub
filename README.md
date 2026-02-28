# ssh-hub

MCP server that gives AI coding agents full access to remote machines over SSH. Connect to multiple servers simultaneously and use remote file operations, shell commands, and directory sync — all through the MCP tool interface.

## Prerequisites

- **[Rust toolchain](https://rustup.rs/)** (1.70+)
- **SSH agent** with loaded keys (`ssh-add`) or SSH key files (`~/.ssh/id_ed25519`, etc.)
- **SSH key authorized on the remote server** — see [docs/server-setup.md](docs/server-setup.md)

## Install

```bash
cargo install --git https://github.com/Perceptron-Studios/ssh-hub.git
```

### Upgrade

```bash
ssh-hub upgrade
```

## Quick start

```bash
# 1. Add a remote server
ssh-hub add <SERVER_ALIAS> user@host

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

NOTE: If a phrase is required to access the private key, this command will ask for passphrase via ssh-add and store it securely.

```bash
ssh-hub add <SERVER_ALIAS> user@host -i ~/.ssh/my_key
```

## CLI commands

| Command                           | Description                                     |
| --------------------------------- | ----------------------------------------------- |
| `ssh-hub`                         | Start MCP server on stdio (used by MCP clients) |
| `ssh-hub list`                    | List configured servers                         |
| `ssh-hub add <name> <connection>` | Add a server (tests connection, then saves)     |
| `ssh-hub remove <name>`           | Remove a server from config                     |
| `ssh-hub update <name>`           | Update server metadata and connection settings  |
| `ssh-hub mcp-install [directory]` | Register ssh-hub as MCP server in a project     |
| `ssh-hub upgrade`                 | Upgrade to the latest release                   |

**Options:** `-v` verbose logging, `-i <path>` identity file, `-p <port>` port override.

**`mcp-install` flags:** `--claude` (`.mcp.json` only), `--codex` (`.codex/config.toml` only). Defaults to both.

## Authentication

All authentication is SSH-key based. No passwords are stored or transmitted. Keys are tried in order:

1. **Identity file** — key specified with `-i` during `add` (highest signal)
2. **SSH agent** — keys loaded via `ssh-add`, signing delegated to `ssh-agent` (capped at 10 keys)
3. **Default keys** — `~/.ssh/id_ed25519`, `id_rsa`, `id_ecdsa`

RSA keys are automatically negotiated with SHA-256/SHA-512 signatures (modern servers reject legacy SHA-1).

## MCP tools

All tools auto-connect to configured servers on first use — no manual connection step needed. Each tool takes a `server` parameter referencing a configured server name.

### Discovery

- **`list_servers`** — Show configured servers with live reachability probes (TCP ping with latency)

### Remote operations

- **`remote_bash`** — Execute shell commands (with optional timeout and background mode)
- **`remote_read`** — Read file contents (with offset/limit for large files)
- **`remote_write`** — Write content to a file
- **`remote_edit`** — Edit a file using string replacement
- **`remote_glob`** — Find files matching a glob pattern

### Sync

- **`sync_push`** — Push local files or directories to remote (tar streaming for directories)
- **`sync_pull`** — Pull remote files or directories to local (tar streaming for directories)

## Configuration

Server configs are stored in `~/.config/ssh-hub/servers.toml` (macOS: `~/Library/Application Support/ssh-hub/servers.toml`):

```toml
[servers.staging]
host = "staging.example.com"
user = "deploy"
port = 2222
remote_path = "/var/www/app"
identity = "~/.ssh/id_staging"

[servers.prod]
host = "prod.example.com"
user = "deploy"
remote_path = "~"
```

## License

MIT
