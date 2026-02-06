# ssh-hub

MCP server for remote SSH sessions. Rust binary (`ssh-hub`) that exposes remote file ops, shell execution, and directory sync over the MCP protocol.

## Domain concepts

- **ServerRegistry** — TOML config mapping server aliases to connection details
- **ServerEntry** — host, user, port, remote_path, identity file, auth method
- **ConnectionPool** — Thread-safe map of active SSH connections (`Arc<RwLock<HashMap>>`)
- **SshConnection** — SSH session wrapping a russh `Handle`: `exec()`, `read_file()`, `write_file()`, `glob()`
- **AuthMethod** — `Auto | Agent | Key` (no password auth)

## Authentication

- **Auto** (default): SSH agent -> explicit identity -> default keys (`~/.ssh/id_{ed25519,rsa,ecdsa}`)
- RSA keys negotiate sha2-256/512 via `best_supported_rsa_hash()` (legacy SHA-1 rejected by modern servers)
- No password auth — keys only, agent preferred

## Architecture

```
src/
├── main.rs             # CLI entrypoint (add, remove, list, mcp-install, or start MCP server)
├── cli.rs              # Clap definitions, connection string parsing, param builders
├── server.rs           # MCP server (RemoteSessionServer) — routes tools via rmcp macros
├── server_registry.rs  # ServerRegistry, ServerEntry, AuthMethod (serde + TOML)
├── lib.rs              # Public module exports
├── connection/
│   ├── auth.rs         # Authentication logic (agent, key, auto fallback chain)
│   ├── session.rs      # SshConnection, ConnectionParams, SshHandler, ExecResult
│   └── pool.rs         # ConnectionPool (RwLock<HashMap>)
├── tools/              # Each tool: mod.rs + schema.rs + handler.rs
│   ├── connect, disconnect, list_servers
│   ├── remote_bash, remote_read, remote_write, remote_edit, remote_glob
│   └── sync_status, sync_push, sync_pull
└── utils/
    ├── checksum.rs     # MD5 hashing for sync
    └── path.rs         # Path normalization
```

## Build & test

```bash
cargo build
cargo test              # 25 tests (cli parsing, mcp install, server registry, utils)
```

## Business rules

- All remote tools require an active connection (checked via `with_connection()`)
- Connection string: `user@host`, `user@host:/path`, `user@host:port`, `user@host:port:/path`
- Remote path defaults to `~` when omitted
- Commands execute in the server's `remote_path` as cwd
- Server config saved only after successful connection test
- Sync is checksum-based (MD5), git-aware when available
