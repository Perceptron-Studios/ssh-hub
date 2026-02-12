use std::collections::HashSet;
use std::time::Instant;

use futures::future::join_all;
use tokio::net::TcpStream;
use tokio::sync::RwLock;
use tokio::time::{timeout, Duration};

use super::schema::{ConnectivityInfo, ListServersInput, ServerInfo, ServerStatus};
use crate::connection::ConnectionPool;
use crate::server_registry::ServerRegistry;

const PROBE_TIMEOUT: Duration = Duration::from_secs(3);

/// TCP-level reachability probe to the SSH port.
async fn probe_reachability(host: &str, port: u16) -> (bool, Option<u64>) {
    let addr = format!("{host}:{port}");
    let start = Instant::now();
    match timeout(PROBE_TIMEOUT, TcpStream::connect(&addr)).await {
        Ok(Ok(_)) => (
            true,
            #[allow(clippy::cast_possible_truncation)]
            Some(start.elapsed().as_millis() as u64),
        ),
        _ => (false, None),
    }
}

/// Probe all servers concurrently and patch reachability into the list.
async fn apply_reachability(servers: &mut [ServerInfo]) {
    let probes = servers.iter().map(|s| probe_reachability(&s.host, s.port));
    let results = join_all(probes).await;

    for (server, (reachable, latency_ms)) in servers.iter_mut().zip(results) {
        server.connectivity.reachable = reachable;
        server.connectivity.latency_ms = latency_ms;
    }
}

/// Enrich connected servers with stored metadata and append configured-only servers.
async fn enrich_with_config(
    servers: &mut Vec<ServerInfo>,
    connected_names: &HashSet<String>,
    config: &RwLock<ServerRegistry>,
    include_configured: bool,
) {
    let cfg = config.read().await;

    for server in servers.iter_mut() {
        if let Some(entry) = cfg.get(&server.name) {
            server.metadata = entry
                .metadata
                .as_ref()
                .map(crate::metadata::SystemMetadata::without_timestamp);
        }
    }

    if include_configured {
        for (name, entry) in &cfg.servers {
            if connected_names.contains(name) {
                continue;
            }
            servers.push(ServerInfo {
                name: name.clone(),
                host: entry.host.clone(),
                user: entry.user.clone(),
                port: entry.port,
                remote_path: entry.remote_path.clone(),
                metadata: entry
                    .metadata
                    .as_ref()
                    .map(crate::metadata::SystemMetadata::without_timestamp),
                connectivity: ConnectivityInfo {
                    status: ServerStatus::Configured,
                    reachable: false,
                    latency_ms: None,
                },
            });
        }
    }
}

pub async fn handle(
    pool: &ConnectionPool,
    config: &RwLock<ServerRegistry>,
    input: ListServersInput,
) -> String {
    let include_configured = input.include_configured.unwrap_or(true);

    // Build connected server list from pool
    let details = pool.list_with_details().await;
    let connected_names: HashSet<String> = details.iter().map(|(name, _)| name.clone()).collect();

    let mut servers: Vec<ServerInfo> = details
        .into_iter()
        .map(|(name, params)| ServerInfo {
            name,
            host: params.host,
            user: params.user,
            port: params.port,
            remote_path: params.remote_path,
            metadata: None,
            connectivity: ConnectivityInfo {
                status: ServerStatus::Connected,
                reachable: false,
                latency_ms: None,
            },
        })
        .collect();

    enrich_with_config(&mut servers, &connected_names, config, include_configured).await;
    apply_reachability(&mut servers).await;

    serde_json::to_string_pretty(&servers)
        .unwrap_or_else(|e| format!(r#"{{"error": "serialization failed: {e}"}}"#))
}
