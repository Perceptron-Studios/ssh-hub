use std::fs;
use std::process::Command;

fn ssh_hub_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ssh-hub"))
}

// ── Claude Code (.mcp.json) ─────────────────────────────────────────

#[test]
fn creates_mcp_json_from_scratch() {
    let dir = tempfile::tempdir().unwrap();

    let output = ssh_hub_bin()
        .args(["mcp-install", dir.path().to_str().unwrap(), "--claude"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let content = fs::read_to_string(dir.path().join(".mcp.json")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

    assert_eq!(parsed["mcpServers"]["ssh-hub"]["command"], "ssh-hub");
    assert_eq!(
        parsed["mcpServers"]["ssh-hub"]["args"],
        serde_json::json!([])
    );
}

#[test]
fn merges_into_existing_mcp_json() {
    let dir = tempfile::tempdir().unwrap();
    let mcp_json = dir.path().join(".mcp.json");

    fs::write(
        &mcp_json,
        r#"{
  "mcpServers": {
    "other-server": {
      "command": "other",
      "args": ["--flag"]
    }
  }
}
"#,
    )
    .unwrap();

    let output = ssh_hub_bin()
        .args(["mcp-install", dir.path().to_str().unwrap(), "--claude"])
        .output()
        .unwrap();

    assert!(output.status.success());

    let content = fs::read_to_string(&mcp_json).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

    // ssh-hub added
    assert_eq!(parsed["mcpServers"]["ssh-hub"]["command"], "ssh-hub");
    // other-server preserved
    assert_eq!(parsed["mcpServers"]["other-server"]["command"], "other");
}

#[test]
fn overwrites_existing_ssh_hub_in_mcp_json() {
    let dir = tempfile::tempdir().unwrap();
    let mcp_json = dir.path().join(".mcp.json");

    fs::write(
        &mcp_json,
        r#"{
  "mcpServers": {
    "ssh-hub": {
      "command": "/old/path/ssh-hub",
      "args": ["--old"]
    }
  }
}
"#,
    )
    .unwrap();

    let output = ssh_hub_bin()
        .args(["mcp-install", dir.path().to_str().unwrap(), "--claude"])
        .output()
        .unwrap();

    assert!(output.status.success());

    let content = fs::read_to_string(&mcp_json).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

    assert_eq!(parsed["mcpServers"]["ssh-hub"]["command"], "ssh-hub");
    assert_eq!(
        parsed["mcpServers"]["ssh-hub"]["args"],
        serde_json::json!([])
    );
}

// ── Codex (.codex/config.toml) ──────────────────────────────────────

#[test]
fn creates_codex_config_from_scratch() {
    let dir = tempfile::tempdir().unwrap();

    let output = ssh_hub_bin()
        .args(["mcp-install", dir.path().to_str().unwrap(), "--codex"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let content = fs::read_to_string(dir.path().join(".codex/config.toml")).unwrap();
    let parsed: toml::Table = content.parse().unwrap();

    let ssh_hub = parsed["mcp_servers"]["ssh-hub"].as_table().unwrap();
    assert_eq!(ssh_hub["command"].as_str().unwrap(), "ssh-hub");
    assert!(ssh_hub["args"].as_array().unwrap().is_empty());
}

#[test]
fn merges_into_existing_codex_config() {
    let dir = tempfile::tempdir().unwrap();
    let codex_dir = dir.path().join(".codex");
    fs::create_dir_all(&codex_dir).unwrap();

    let config_path = codex_dir.join("config.toml");
    fs::write(
        &config_path,
        r#"model = "o3"

[mcp_servers.other-server]
command = "other"
args = []
"#,
    )
    .unwrap();

    let output = ssh_hub_bin()
        .args(["mcp-install", dir.path().to_str().unwrap(), "--codex"])
        .output()
        .unwrap();

    assert!(output.status.success());

    let content = fs::read_to_string(&config_path).unwrap();
    let parsed: toml::Table = content.parse().unwrap();

    // ssh-hub added
    assert_eq!(
        parsed["mcp_servers"]["ssh-hub"]["command"]
            .as_str()
            .unwrap(),
        "ssh-hub"
    );
    // other-server preserved
    assert_eq!(
        parsed["mcp_servers"]["other-server"]["command"]
            .as_str()
            .unwrap(),
        "other"
    );
    // top-level key preserved
    assert_eq!(parsed["model"].as_str().unwrap(), "o3");
}

// ── Flag behavior ───────────────────────────────────────────────────

#[test]
fn no_flags_configures_both() {
    let dir = tempfile::tempdir().unwrap();

    let output = ssh_hub_bin()
        .args(["mcp-install", dir.path().to_str().unwrap()])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(dir.path().join(".mcp.json").exists());
    assert!(dir.path().join(".codex/config.toml").exists());
}

#[test]
fn claude_only_skips_codex() {
    let dir = tempfile::tempdir().unwrap();

    let output = ssh_hub_bin()
        .args(["mcp-install", dir.path().to_str().unwrap(), "--claude"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(dir.path().join(".mcp.json").exists());
    assert!(!dir.path().join(".codex").exists());
}

#[test]
fn codex_only_skips_claude() {
    let dir = tempfile::tempdir().unwrap();

    let output = ssh_hub_bin()
        .args(["mcp-install", dir.path().to_str().unwrap(), "--codex"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(!dir.path().join(".mcp.json").exists());
    assert!(dir.path().join(".codex/config.toml").exists());
}

// ── Error cases ─────────────────────────────────────────────────────

#[test]
fn errors_on_nonexistent_directory() {
    let output = ssh_hub_bin()
        .args(["mcp-install", "/nonexistent/path/that/does/not/exist"])
        .output()
        .unwrap();

    assert!(!output.status.success());
}
