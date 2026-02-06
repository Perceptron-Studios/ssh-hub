use tokio::sync::RwLock;

use crate::server_registry::ServerRegistry;
use crate::connection::ConnectionPool;
use super::schema::*;

pub async fn handle(
    pool: &ConnectionPool,
    config: &RwLock<ServerRegistry>,
    input: ListServersInput,
) -> String {
    let connected_names = pool.list().await;
    let mut connected = Vec::new();

    for name in &connected_names {
        if let Some(conn) = pool.get(name).await {
            let params = conn.params();
            connected.push(ConnectedServerInfo {
                name: name.clone(),
                host: params.host.clone(),
                user: params.user.clone(),
                port: params.port,
                remote_path: params.remote_path.clone(),
            });
        }
    }

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
    serde_json::to_string_pretty(&output).unwrap_or_default()
}
