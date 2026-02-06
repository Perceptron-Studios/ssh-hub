# ssh-hub

MCP server for remote SSH sessions via Claude Code. Rust binary (`ssh-hub`) that exposes remote file ops, shell execution, and directory sync over the MCP protocol.

## Domain concepts

- **ServerRegistry** — TOML-based config (`~/.config/ssh-hub/servers.toml`) mapping server aliases to connection details
- **ServerEntry** — A configured server: host, user, port, remote_path, identity file, auth method
- **ConnectionPool** — Thread-safe map of active SSH connections (`Arc<RwLock<HashMap>>`)
- **SshConnection** — Single SSH session wrapping a russh `Handle`, provides `exec()`, `read_file()`, `write_file()`, `glob()`
- **AuthMethod** — `Auto | Agent | Key` (no password auth)

## Authentication

- **Auto** (default): tries SSH agent -> explicit identity -> default keys (`~/.ssh/id_{ed25519,rsa,ecdsa}`)
- **Agent**: connects to `ssh-agent` via `SSH_AUTH_SOCK`, uses `authenticate_publickey_with` + russh's `Signer` trait
- **Key**: loads private key from disk via `russh_keys::load_secret_key`
- No password/keychain auth — removed in favor of SSH agent

## Architecture

```
src/
├── main.rs          # CLI entrypoint (setup, add, remove, list, or start MCP server)
├── cli.rs           # Clap definitions, connection string parsing, param builders
├── server.rs        # MCP server (RemoteSessionServer) — routes tools via rmcp macros
├── server_registry.rs  # ServerRegistry, ServerEntry, AuthMethod (serde + TOML)
├── lib.rs           # Public module exports
├── connection/
│   ├── auth.rs      # Authentication logic (agent, key, auto fallback chain)
│   ├── session.rs   # SshConnection, ConnectionParams, SshHandler, ExecResult
│   └── pool.rs      # ConnectionPool (RwLock<HashMap>)
├── tools/           # Each tool: mod.rs + schema.rs + handler.rs
│   ├── connect, disconnect, list_servers
│   ├── remote_bash, remote_read, remote_write, remote_edit, remote_glob
│   └── sync_status, sync_push, sync_pull
└── utils/
    ├── checksum.rs  # MD5 hashing for sync
    └── path.rs      # Path normalization
```

## Key dependencies

- `rmcp` — MCP SDK (server + stdio transport)
- `russh` / `russh-keys` — SSH protocol, key loading, agent client
- `tokio` — async runtime
- `clap` — CLI parsing
- `serde` / `toml` — config serialization

## Build & test

```bash
cargo build
cargo test          # 13 tests (cli parsing, server registry, utils)
```

## Business rules

- All remote tools require an active connection (checked via `with_connection()`)
- Connection string format: `user@host:/path` or `user@host:port:/path`
- Commands execute in the server's `remote_path` as cwd
- Sync is checksum-based (MD5), git-aware when available
