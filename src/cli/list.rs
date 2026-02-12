use anyhow::Result;
use colored::Colorize;

use crate::metadata::SystemMetadata;
use crate::server_registry::ServerRegistry;

pub fn run() -> Result<()> {
    let config = ServerRegistry::load()?;

    if config.servers.is_empty() {
        println!("{}", "No servers configured.".dimmed());
        println!(
            "Run {} to add one.",
            "ssh-hub add <name> user@host:/path".bold(),
        );
        return Ok(());
    }

    for (name, entry) in &config.servers {
        println!(
            "  {} {} {}@{}:{} {}",
            name.bold(),
            "->".dimmed(),
            entry.user.cyan(),
            entry.host.cyan(),
            entry.port.to_string().cyan(),
            format!("(path: {}, auth: {})", entry.remote_path, entry.auth).dimmed(),
        );
        if let Some(summary) = entry
            .metadata
            .as_ref()
            .and_then(SystemMetadata::summary_line)
        {
            println!("    {}", summary.dimmed());
        }
    }
    Ok(())
}
