use std::time::{Duration, Instant};

use anyhow::Result;
use colored::Colorize;
use futures::future::join_all;
use tokio::net::TcpStream;
use tokio::time::timeout;

use crate::metadata::SystemMetadata;
use crate::server_registry::{ServerEntry, ServerRegistry};

use super::spinner;

const PROBE_TIMEOUT: Duration = Duration::from_secs(3);

/// TCP-level reachability probe to the SSH port.
///
/// Returns `(reachable, latency_ms)` — latency is `Some` only on success.
async fn probe_reachability(host: &str, port: u16) -> (bool, Option<u32>) {
    let addr = format!("{host}:{port}");
    let start = Instant::now();
    match timeout(PROBE_TIMEOUT, TcpStream::connect(&addr)).await {
        Ok(Ok(_)) => {
            let ms = u32::try_from(start.elapsed().as_millis()).unwrap_or(u32::MAX);
            (true, Some(ms))
        }
        _ => (false, None),
    }
}

pub async fn run(no_probe: bool) -> Result<()> {
    let config = ServerRegistry::load()?;

    if config.servers.is_empty() {
        println!("{}", "No servers configured.".dimmed());
        println!(
            "Run {} to add one.",
            "ssh-hub add <name> user@host:/path".bold(),
        );
        return Ok(());
    }

    if no_probe {
        for (name, entry) in &config.servers {
            print_server(name, entry);
        }
        return Ok(());
    }

    // Probe all servers concurrently behind a single spinner.
    let sp = spinner::start_root("Probing reachability...");
    let servers: Vec<_> = config.servers.iter().collect();
    let probes = servers
        .iter()
        .map(|(_, entry)| probe_reachability(&entry.host, entry.port));
    let results = join_all(probes).await;
    spinner::clear(&sp);

    for ((name, entry), (reachable, latency_ms)) in servers.iter().zip(results) {
        print_server(name, entry);
        if reachable {
            let ms = latency_ms.map_or(String::new(), |ms| format!(" ({ms}ms)"));
            println!("  {} reachable{ms}", "ok".green());
        } else {
            println!("  {} unreachable", "warn".yellow());
        }
    }
    Ok(())
}

fn format_server_info(name: &str, entry: &ServerEntry) -> String {
    format!(
        "{} {} {}@{}:{} {}",
        name.bold(),
        "->".dimmed(),
        entry.user.cyan(),
        entry.host.cyan(),
        entry.port.to_string().cyan(),
        format!("(path: {}, auth: {})", entry.remote_path, entry.auth).dimmed(),
    )
}

fn format_metadata(entry: &ServerEntry) -> Option<String> {
    entry
        .metadata
        .as_ref()
        .and_then(SystemMetadata::summary_line)
        .map(|summary| format!("  {}", summary.dimmed()))
}

fn print_server(name: &str, entry: &ServerEntry) {
    println!("{}", format_server_info(name, entry));
    if let Some(meta) = format_metadata(entry) {
        println!("{meta}");
    }
}
