# ssh-hub

MCP server for remote SSH sessions. Rust binary that exposes remote file ops, shell execution, and directory sync over the MCP protocol.

## Core concepts

- **ServerRegistry** — TOML config mapping server aliases to `ServerEntry` (host, user, port, remote_path, identity, auth)
- **ConnectionPool** — `Arc<RwLock<HashMap>>` of active `SshConnection`s
- **SshConnection** — wraps a russh `Handle`, provides `exec()`, `read_file()`, `write_file()`, `glob()`
- **AuthMethod** — `Auto | Agent | Key` — no password auth, agent preferred

## Architecture

- `src/main.rs` — CLI handlers (add, remove, list, mcp-install). Colored output via `colored` crate.
- `src/cli.rs` — Clap definitions, connection string parsing, `ConnectionParams` builders
- `src/server.rs` — MCP server (`RemoteSessionServer`) using `rmcp` macros (`#[tool_router]`, `#[tool_handler]`)
- `src/connection/` — `auth.rs` (agent/key fallback chain, RSA hash negotiation), `session.rs` (SSH session), `pool.rs` (connection pool)
- `src/tools/` — one module per MCP tool, each with `mod.rs` + `schema.rs` + `handler.rs`
- `src/utils/` — `checksum.rs` (MD5), `path.rs` (normalization)

## Key patterns

- Connection strings: `user@host`, `user@host:/path`, `user@host:port`, `user@host:port:/path` — path defaults to `~`
- All remote tools go through `with_connection()` which checks the pool
- Commands execute as `cd {remote_path} && {command}` — `~` is shell-expanded
- RSA keys negotiate sha2-256/512 via `best_supported_rsa_hash()` (servers reject SHA-1)
- Server config is saved only after a successful connection test
- CLI output uses `colored` crate — `ok`/`warn`/`failed` status prefixes

## References

- [docs/testing.md](docs/testing.md) — test structure, what's covered, how to add tests
