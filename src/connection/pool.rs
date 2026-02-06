use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

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

    /// Get a connection by name. Returns None if not connected.
    pub async fn get(&self, name: &str) -> Option<Arc<SshConnection>> {
        let guard = self.connections.read().await;
        guard.get(name).cloned()
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

    /// Check if a server name has an active connection.
    pub async fn contains(&self, name: &str) -> bool {
        let guard = self.connections.read().await;
        guard.contains_key(name)
    }
}
