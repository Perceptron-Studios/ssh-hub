# Testing

## Unit tests

```bash
cargo test
```

All unit tests live in `tests/`:

| File | Covers |
|------|--------|
| `cli.rs` | Connection string parsing — all format variants, edge cases, port overrides |
| `mcp_install.rs` | MCP config generation — `.mcp.json` and `.codex/config.toml` create/merge/overwrite, flag behavior |
| `server_registry.rs` | Config serialization — TOML roundtrip, defaults, parsing |
| `utils.rs` | Path normalization, shell escaping, line number formatting, path traversal validation |

## MCP integration testing

Unit tests cover parsing and config logic, but the MCP tools operate over live SSH connections and **cannot be unit tested**. Any change to tool handlers, `session.rs`, `pool.rs`, or `auth.rs` must be validated through the MCP interface.

### Process

1. **Build and install locally:**
   ```bash
   cargo install --path .
   ```

2. **Restart the MCP server** in your MCP client (e.g., restart Claude Code's ssh-hub server so it picks up the new binary).

3. **Connect to a real server** via the `connect` tool and exercise the affected tools against it. Test both the happy path and edge cases relevant to your change (e.g., binary files for sync changes, large output for bash handler changes, offset/limit for read changes).

This is the primary development loop for this repo — `cargo test` validates offline logic, MCP integration testing validates everything that touches SSH.

## Adding tests

- **CLI parsing:** add cases to `tests/cli.rs`
- **Config behavior:** add cases to `tests/server_registry.rs`
- **MCP install:** add cases to `tests/mcp_install.rs` — uses `tempfile` for isolated dirs
- **Path utilities:** add cases to `tests/utils.rs`
- **MCP tools:** no mocking — test via the integration process above
