use tokio::sync::RwLock;

use crate::server_registry::ServerRegistry;
use crate::connection::ConnectionPool;
use super::schema::*;

pub async fn handle(
    pool: &ConnectionPool,
    config: &RwLock<ServerRegistry>,
    input: ListServersInput,
) -> String {
    // Single lock acquisition for all connected server details
    let details = pool.list_with_details().await;
    let connected_names: Vec<String> = details.iter().map(|(name, _)| name.clone()).collect();

    let connected: Vec<ConnectedServerInfo> = details
        .into_iter()
        .map(|(name, params)| ConnectedServerInfo {
            name,
            host: params.host,
            user: params.user,
            port: params.port,
            remote_path: params.remote_path,
        })
        .collect();

    let include_configured = input.include_configured.unwrap_or(true);
    let configured = if include_configured {
        let cfg = config.read().await;
        let list: Vec<ConfiguredServerInfo> = cfg
            .servers
            .iter()
            .map(|(name, entry)| ConfiguredServerInfo {
                name: name.clone(),
                host: entry.host.clone(),
                user: entry.user.clone(),
                port: entry.port,
                remote_path: entry.remote_path.clone(),
                auth: format!("{:?}", entry.auth).to_lowercase(),
                connected: connected_names.contains(name),
            })
            .collect();
        Some(list)
    } else {
        None
    };

    let output = ListServersOutput {
        connected,
        configured,
    };
    serde_json::to_string(&output)
        .unwrap_or_else(|e| format!(r#"{{"error": "serialization failed: {}"}}"#, e))
}
