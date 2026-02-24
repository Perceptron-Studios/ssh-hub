use std::path::PathBuf;

use anyhow::{anyhow, Result};
use colored::Colorize;

use crate::connection::SshConnection;
use crate::metadata::SystemMetadata;
use crate::server_registry::{ServerEntry, ServerRegistry};
use crate::{metadata, metadata::diff};

use super::params_from_config;
use super::spinner;

#[derive(Default)]
pub struct ConnectionOverrides {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub remote_path: Option<String>,
    pub identity: Option<PathBuf>,
}

impl ConnectionOverrides {
    fn has_any(&self) -> bool {
        self.host.is_some()
            || self.port.is_some()
            || self.remote_path.is_some()
            || self.identity.is_some()
    }
}

pub async fn run(name: Option<String>, all: bool, overrides: ConnectionOverrides) -> Result<()> {
    if !all && name.is_none() {
        return Err(anyhow!(
            "Specify a server name or use --all to refresh all servers"
        ));
    }

    if all && overrides.has_any() {
        return Err(anyhow!(
            "Connection setting overrides (--host, --port, etc.) cannot be used with --all"
        ));
    }

    let mut config = ServerRegistry::load()?;

    if all {
        let names: Vec<String> = config.servers.keys().cloned().collect();
        if names.is_empty() {
            println!("{}", "No servers configured.".dimmed());
            return Ok(());
        }
        for server_name in &names {
            refresh_single(server_name, &mut config, ConnectionOverrides::default()).await;
        }
    } else if let Some(server_name) = name {
        if config.get(&server_name).is_none() {
            return Err(anyhow!("Server '{server_name}' not found in config"));
        }
        refresh_single(&server_name, &mut config, overrides).await;
    }

    config.save()?;
    Ok(())
}

/// Apply connection setting overrides to an entry, printing each change.
fn apply_overrides(entry: &mut ServerEntry, overrides: ConnectionOverrides) {
    if let Some(h) = overrides.host {
        println!("  {} host -> {}", "update".blue(), h.cyan());
        entry.host = h;
    }
    if let Some(p) = overrides.port {
        println!("  {} port -> {}", "update".blue(), p.to_string().cyan());
        entry.port = p;
    }
    if let Some(rp) = overrides.remote_path {
        println!("  {} remote_path -> {}", "update".blue(), rp.cyan());
        entry.remote_path = rp;
    }
    if let Some(id) = overrides.identity {
        let id_str = id.to_string_lossy().to_string();
        println!("  {} identity -> {}", "update".blue(), id_str.cyan());
        entry.identity = Some(id_str);
    }
}

async fn refresh_single(name: &str, config: &mut ServerRegistry, overrides: ConnectionOverrides) {
    println!("{} Refreshing {}...", ">".blue().bold(), name.bold());

    // Apply overrides and extract what we need, then drop the mutable borrow
    let (old_metadata, params) = {
        let Some(entry) = config.servers.get_mut(name) else {
            println!("  {} Server not found", "warn".yellow());
            return;
        };

        apply_overrides(entry, overrides);
        (entry.metadata.clone(), params_from_config(name, entry))
    };

    let sp = spinner::start("Establishing connection...");
    match SshConnection::connect(params).await {
        Ok(conn) => {
            spinner::finish_ok(&sp, "Connection established");
            collect_and_store(name, &conn, old_metadata.as_ref(), config).await;
        }
        Err(e) => {
            spinner::finish_failed(&sp, &format!("Connection failed: {e}"));
        }
    }
}

async fn collect_and_store(
    name: &str,
    conn: &SshConnection,
    old_metadata: Option<&SystemMetadata>,
    config: &mut ServerRegistry,
) {
    let sp = spinner::start("Extracting system metadata...");
    let new_meta = match metadata::collect(conn).await {
        Ok(meta) => meta,
        Err(e) => {
            spinner::finish_warn(&sp, &format!("Metadata extraction failed: {e}"));
            return;
        }
    };

    match old_metadata.and_then(|old| diff(old, &new_meta)) {
        Some(changes) => {
            spinner::finish_ok(&sp, "Metadata updated");
            println!("    {} {}", "!".yellow().bold(), changes);
        }
        None if old_metadata.is_some() => {
            spinner::finish_ok(&sp, "Metadata unchanged");
        }
        None => {
            spinner::finish_ok(&sp, "Metadata extracted");
        }
    }

    if let Some(summary) = new_meta.summary_line() {
        println!("    {}", summary.dimmed());
    }

    if let Some(entry) = config.servers.get_mut(name) {
        entry.metadata = Some(new_meta);
    }
}
