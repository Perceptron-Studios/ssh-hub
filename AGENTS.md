# ssh-hub

MCP server for remote SSH sessions. Rust binary that exposes remote file ops, shell execution, and directory sync over the MCP protocol.

## Core concepts

- **ServerRegistry** — TOML config mapping server aliases to `ServerEntry` (host, user, port, remote_path, identity, auth)
- **ConnectionPool** — `Arc<RwLock<HashMap>>` of active `SshConnection`s
- **SshConnection** — wraps a russh `Handle`, provides `exec()`, `exec_raw()`, `read_file()`, `read_file_raw()`, `write_file()`, `write_file_raw()`, `glob()`, `is_closed()`
- **AuthMethod** — `Auto | Agent | Key` — no password auth, agent preferred

## Architecture

- `src/main.rs` — CLI handlers (add, remove, list, mcp-install, update). Colored output via `colored` crate.
- `src/cli.rs` — Clap definitions, connection string parsing, `ConnectionParams` builders
- `src/server.rs` — MCP server (`RemoteSessionServer`) using `rmcp` macros (`#[tool_router]`, `#[tool_handler]`)
- `src/connection/` — `auth.rs` (agent/key fallback chain, RSA hash negotiation), `session.rs` (SSH session), `pool.rs` (connection pool)
- `src/tools/` — one module per MCP tool, each with `mod.rs` + `schema.rs` + `handler.rs`. Shared sync types in `sync_types.rs`
- `src/utils/` — `path.rs` (normalization, line number formatting)

## Key patterns

- Connection strings: `user@host`, `user@host:/path`, `user@host:port`, `user@host:port:/path` — path defaults to `~`
- All remote tools go through `with_connection()` which checks the pool (stale connections auto-removed via `is_closed()`)
- Commands execute as `cd {remote_path} && {command}` — `~` is shell-expanded
- Session mutex held only for `channel_open_session()` — allows concurrent SSH commands
- Auth order: explicit identity → SSH agent (capped at 10 keys) → default key paths. RSA hash negotiated once per auth attempt.
- `remote_bash` saves large output (>128 KB) to local disk, returns head/tail summary with file path
- `sync_push`/`sync_pull` use tar.gz streaming for directory transfers, raw bytes for single files (binary-safe)
- Blocking I/O (`walk_dir`, `build_tar_gz`, tar extraction, `load_secret_key`) wrapped in `spawn_blocking`
- `remote_read` with offset/limit uses server-side `sed` — transfers only requested lines
- CLI output uses `colored` crate — `ok`/`warn`/`failed` status prefixes
- Self-update via `ssh-hub update` — checks GitHub tags, runs `cargo install --git` if newer version exists

## Releasing

1. Bump `version` in `Cargo.toml`
2. Push to `main`
3. GitHub Action (`.github/workflows/tag.yml`) auto-creates a `vX.Y.Z` tag if it doesn't exist
4. Users pick up the new version via `ssh-hub update`

## Testing

- `cargo test` — unit tests for CLI parsing, config, path utils
- **MCP integration testing** — the primary validation for tool changes. Install locally (`cargo install --path .`), restart the MCP server, connect to a real server, and exercise the affected tools. See [docs/testing.md](docs/testing.md) for details.

## References

- [docs/testing.md](docs/testing.md) — test structure, integration testing process
- [changelog/](changelog/) — per-version changelog with motivations and design decisions ([format guide](docs/changelog.md))
