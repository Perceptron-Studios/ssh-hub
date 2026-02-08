use std::time::Instant;

use futures::future::join_all;
use tokio::net::TcpStream;
use tokio::sync::RwLock;
use tokio::time::{timeout, Duration};

use crate::connection::ConnectionPool;
use crate::server_registry::ServerRegistry;
use super::schema::*;

const PROBE_TIMEOUT: Duration = Duration::from_secs(3);

/// TCP-level reachability probe to the SSH port.
async fn probe_reachability(host: &str, port: u16) -> ReachabilityInfo {
    let addr = format!("{}:{}", host, port);
    let start = Instant::now();
    match timeout(PROBE_TIMEOUT, TcpStream::connect(&addr)).await {
        Ok(Ok(_)) => ReachabilityInfo {
            reachable: true,
            latency_ms: Some(start.elapsed().as_millis() as u64),
        },
        _ => ReachabilityInfo {
            reachable: false,
            latency_ms: None,
        },
    }
}

/// Probe all servers concurrently and patch reachability into both lists.
async fn apply_reachability(
    connected: &mut [ConnectedServerInfo],
    configured: Option<&mut [ConfiguredServerInfo]>,
) {
    let mut targets: Vec<(&str, u16)> = connected
        .iter()
        .map(|s| (s.host.as_str(), s.port))
        .collect();

    let connected_count = targets.len();

    if let Some(ref cfg) = configured {
        targets.extend(cfg.iter().map(|s| (s.host.as_str(), s.port)));
    }

    let probes = targets.iter().map(|(host, port)| probe_reachability(host, *port));
    let results = join_all(probes).await;
    let (conn_results, cfg_results) = results.split_at(connected_count);

    for (server, info) in connected.iter_mut().zip(conn_results) {
        server.reachability = info.clone();
    }

    if let Some(cfg) = configured {
        for (server, info) in cfg.iter_mut().zip(cfg_results) {
            server.reachability = info.clone();
        }
    }
}

pub async fn handle(
    pool: &ConnectionPool,
    config: &RwLock<ServerRegistry>,
    input: ListServersInput,
) -> String {
    let include_configured = input.include_configured.unwrap_or(true);

    // Build connected server list from pool (single lock)
    let details = pool.list_with_details().await;
    let connected_names: Vec<String> = details.iter().map(|(name, _)| name.clone()).collect();

    let mut connected: Vec<ConnectedServerInfo> = details
        .into_iter()
        .map(|(name, params)| ConnectedServerInfo {
            name,
            host: params.host,
            user: params.user,
            port: params.port,
            remote_path: params.remote_path,
            reachability: ReachabilityInfo::default(),
        })
        .collect();

    // Build configured server list from registry (lock dropped before probing)
    let mut configured: Option<Vec<ConfiguredServerInfo>> = if include_configured {
        let cfg = config.read().await;
        Some(
            cfg.servers
                .iter()
                .map(|(name, entry)| ConfiguredServerInfo {
                    name: name.clone(),
                    host: entry.host.clone(),
                    user: entry.user.clone(),
                    port: entry.port,
                    remote_path: entry.remote_path.clone(),
                    auth: format!("{:?}", entry.auth).to_lowercase(),
                    connected: connected_names.contains(name),
                    reachability: ReachabilityInfo::default(),
                })
                .collect(),
        )
    } else {
        None
    };

    // Probe all servers concurrently
    apply_reachability(&mut connected, configured.as_deref_mut()).await;

    let output = ListServersOutput {
        connected,
        configured,
    };
    serde_json::to_string_pretty(&output)
        .unwrap_or_else(|e| format!(r#"{{"error": "serialization failed: {}"}}"#, e))
}
