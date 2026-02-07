use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::session::ConnectionParams;
use super::SshConnection;

/// Thread-safe pool of named SSH connections.
/// Uses RwLock for concurrent reads (tool execution) and exclusive writes (connect/disconnect).
pub struct ConnectionPool {
    connections: RwLock<HashMap<String, Arc<SshConnection>>>,
}

impl ConnectionPool {
    pub fn new() -> Self {
        Self {
            connections: RwLock::new(HashMap::new()),
        }
    }

    /// Get a connection by name. Returns None if not connected or if the
    /// underlying SSH session has been closed (stale connections are removed).
    pub async fn get(&self, name: &str) -> Option<Arc<SshConnection>> {
        let conn = {
            let guard = self.connections.read().await;
            guard.get(name).cloned()
        };

        if let Some(ref c) = conn {
            if c.is_closed().await {
                tracing::debug!("Connection '{}' is closed, removing from pool", name);
                let mut guard = self.connections.write().await;
                guard.remove(name);
                return None;
            }
        }

        conn
    }

    /// Insert a new connection. Returns the previous connection if one existed with this name.
    pub async fn insert(&self, name: String, conn: SshConnection) -> Option<Arc<SshConnection>> {
        let mut guard = self.connections.write().await;
        guard.insert(name, Arc::new(conn))
    }

    /// Remove and return a connection by name.
    pub async fn remove(&self, name: &str) -> Option<Arc<SshConnection>> {
        let mut guard = self.connections.write().await;
        guard.remove(name)
    }

    /// List all connected server names.
    pub async fn list(&self) -> Vec<String> {
        let guard = self.connections.read().await;
        guard.keys().cloned().collect()
    }

    /// List all connected servers with their connection parameters in a single lock.
    pub async fn list_with_details(&self) -> Vec<(String, ConnectionParams)> {
        let guard = self.connections.read().await;
        guard
            .iter()
            .map(|(name, conn)| (name.clone(), conn.params().clone()))
            .collect()
    }

    /// Check if a server name has an active connection.
    pub async fn contains(&self, name: &str) -> bool {
        let guard = self.connections.read().await;
        guard.contains_key(name)
    }
}
