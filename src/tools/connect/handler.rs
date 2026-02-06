use tokio::sync::RwLock;

use crate::cli::{params_from_config, params_from_connection_string, parse_connection_string};
use crate::server_registry::{AuthMethod, ServerEntry, ServerRegistry};
use crate::connection::{ConnectionPool, SshConnection};
use super::schema::ConnectInput;

pub async fn handle(
    pool: &ConnectionPool,
    config: &RwLock<ServerRegistry>,
    input: ConnectInput,
) -> String {
    if pool.contains(&input.name).await {
        return format!(
            "Error: '{}' is already connected. Disconnect first or use a different name.",
            input.name
        );
    }

    let params = if let Some(ref conn_str) = input.connection {
        match params_from_connection_string(
            &input.name,
            conn_str,
            input.port,
            input.identity.as_deref(),
        ) {
            Ok(p) => p,
            Err(e) => return format!("Error parsing connection string: {}", e),
        }
    } else {
        let cfg = config.read().await;
        match cfg.get(&input.name) {
            Some(entry) => {
                let mut params = params_from_config(&input.name, entry);
                if let Some(port) = input.port {
                    params.port = port;
                }
                if let Some(ref id) = input.identity {
                    params.identity = Some(std::path::PathBuf::from(id));
                }
                params
            }
            None => {
                return format!(
                    "Error: '{}' not found in config and no connection string provided. \
                     Use 'connection' parameter or run 'ssh-hub setup {}'.",
                    input.name, input.name
                );
            }
        }
    };

    match SshConnection::connect(params).await {
        Ok(conn) => {
            let info = format!(
                "Connected to {}@{}:{} (path: {})",
                conn.params().user,
                conn.params().host,
                conn.params().port,
                conn.params().remote_path,
            );
            pool.insert(input.name.clone(), conn).await;

            if input.save.unwrap_or(false) {
                if let Some(ref conn_str) = input.connection {
                    if let Ok(ci) = parse_connection_string(conn_str, input.port) {
                        let entry = ServerEntry {
                            host: ci.host,
                            user: ci.user,
                            port: ci.port,
                            remote_path: ci.remote_path,
                            identity: input.identity.clone(),
                            auth: AuthMethod::Auto,
                        };
                        let mut cfg = config.write().await;
                        cfg.insert(input.name.clone(), entry);
                        if let Err(e) = cfg.save() {
                            return format!(
                                "{}. Warning: failed to save config: {}",
                                info, e
                            );
                        }
                    }
                }
            }

            info
        }
        Err(e) => format!("Error connecting to '{}': {}", input.name, e),
    }
}
