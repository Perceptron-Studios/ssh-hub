use colored::Colorize;

use crate::server_registry::ServerRegistry;

pub fn run() {
    let config = ServerRegistry::load().unwrap_or_default();

    if config.servers.is_empty() {
        println!("{}", "No servers configured.".dimmed());
        println!(
            "Run {} to add one.",
            "ssh-hub add <name> user@host:/path".bold(),
        );
        return;
    }

    for (name, entry) in &config.servers {
        println!(
            "  {} {} {}@{}:{} {}",
            name.bold(),
            "->".dimmed(),
            entry.user.cyan(),
            entry.host.cyan(),
            entry.port.to_string().cyan(),
            format!("(path: {}, auth: {:?})", entry.remote_path, entry.auth).dimmed(),
        );
    }
}
