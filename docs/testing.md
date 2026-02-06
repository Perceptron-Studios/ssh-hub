# Testing

## Running tests

```bash
cargo test
```

## Test structure

All tests are integration tests in `tests/`:

| File | Covers |
|------|--------|
| `cli.rs` | Connection string parsing — all format variants, edge cases, port overrides |
| `mcp_install.rs` | MCP config generation — `.mcp.json` and `.codex/config.toml` create/merge/overwrite, flag behavior |
| `server_registry.rs` | Config serialization — TOML roundtrip, defaults, parsing |
| `utils.rs` | Path normalization, MD5 checksums, line number formatting |

## What's not tested

SSH connection and authentication require a live server and agent — these are tested manually via `ssh-hub add`. The MCP tool handlers (`src/tools/`) are also untested (they delegate to `SshConnection` methods).

## Adding tests

- CLI parsing: add cases to `tests/cli.rs` using `parse_connection_string()`
- Config behavior: add cases to `tests/server_registry.rs`
- MCP install: add cases to `tests/mcp_install.rs` — uses `tempfile` crate for isolated directories
- New tools: if adding a tool that doesn't require SSH, add an integration test in `tests/`
