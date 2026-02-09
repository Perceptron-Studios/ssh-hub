use std::path::Path;

use anyhow::Result;
use colored::Colorize;

pub fn run(directory: &Path, claude: bool, codex: bool) -> Result<()> {
    // When neither flag is provided, configure both
    let (do_claude, do_codex) = if !claude && !codex {
        (true, true)
    } else {
        (claude, codex)
    };

    let target = std::fs::canonicalize(directory).map_err(|e| {
        anyhow::anyhow!("Cannot resolve directory '{}': {}", directory.display(), e)
    })?;

    if !target.is_dir() {
        return Err(anyhow::anyhow!("'{}' is not a directory", target.display()));
    }

    if do_claude {
        install_claude_config(&target)?;
    }
    if do_codex {
        install_codex_config(&target)?;
    }

    Ok(())
}

fn install_claude_config(target: &Path) -> Result<()> {
    let path = target.join(".mcp.json");

    let mut root: serde_json::Value = if path.exists() {
        let content = std::fs::read_to_string(&path)?;
        serde_json::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse {}: {}", path.display(), e))?
    } else {
        serde_json::json!({})
    };

    let servers = root
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!(".mcp.json root is not a JSON object"))?
        .entry("mcpServers")
        .or_insert_with(|| serde_json::json!({}));

    let servers_map = servers
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("\"mcpServers\" is not a JSON object"))?;

    servers_map.insert(
        "ssh-hub".to_string(),
        serde_json::json!({
            "command": "ssh-hub",
            "args": []
        }),
    );

    let output = serde_json::to_string_pretty(&root)? + "\n";
    std::fs::write(&path, output)?;

    println!(
        "  {} Claude Code: {}",
        "ok".green(),
        path.display().to_string().dimmed(),
    );
    Ok(())
}

fn install_codex_config(target: &Path) -> Result<()> {
    let codex_dir = target.join(".codex");
    let path = codex_dir.join("config.toml");

    let mut doc: toml::Table = if path.exists() {
        let content = std::fs::read_to_string(&path)?;
        content
            .parse::<toml::Table>()
            .map_err(|e| anyhow::anyhow!("Failed to parse {}: {}", path.display(), e))?
    } else {
        toml::Table::new()
    };

    let mcp_servers = doc
        .entry("mcp_servers")
        .or_insert_with(|| toml::Value::Table(toml::Table::new()));

    let servers_table = mcp_servers
        .as_table_mut()
        .ok_or_else(|| anyhow::anyhow!("\"mcp_servers\" is not a TOML table"))?;

    let mut entry = toml::Table::new();
    entry.insert(
        "command".to_string(),
        toml::Value::String("ssh-hub".to_string()),
    );
    entry.insert("args".to_string(), toml::Value::Array(vec![]));

    servers_table.insert("ssh-hub".to_string(), toml::Value::Table(entry));

    std::fs::create_dir_all(&codex_dir)?;
    let output = toml::to_string_pretty(&doc)?;
    std::fs::write(&path, output)?;

    println!(
        "  {} Codex: {}",
        "ok".green(),
        path.display().to_string().dimmed(),
    );
    Ok(())
}
