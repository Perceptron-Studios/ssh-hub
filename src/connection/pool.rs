use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

use super::session::ConnectionParams;
use super::SshConnection;

/// Thread-safe pool of named SSH connections.
/// Uses `RwLock` for concurrent reads (tool execution) and exclusive writes (connect/disconnect).
///
/// Per-server connect locks prevent concurrent tool calls from racing to create
/// duplicate connections to the same server. The lock is held only during the
/// "check pool → connect → insert" window — once a connection is pooled, all
/// callers proceed without blocking.
pub struct ConnectionPool {
    connections: RwLock<HashMap<String, Arc<SshConnection>>>,
    /// Per-server locks that serialize connection establishment.
    connect_locks: RwLock<HashMap<String, Arc<Mutex<()>>>>,
}

impl Default for ConnectionPool {
    fn default() -> Self {
        Self::new()
    }
}

impl ConnectionPool {
    #[must_use]
    pub fn new() -> Self {
        Self {
            connections: RwLock::new(HashMap::new()),
            connect_locks: RwLock::new(HashMap::new()),
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
                // Only evict if this is still the same connection object —
                // another task may have already replaced it with a fresh one.
                if let Some(current) = guard.get(name) {
                    if Arc::ptr_eq(current, c) {
                        guard.remove(name);
                    }
                }
                return None;
            }
        }

        conn
    }

    /// Insert a new connection into the pool, returning the `Arc` handle to it.
    pub async fn insert(&self, name: String, conn: SshConnection) -> Arc<SshConnection> {
        let arc = Arc::new(conn);
        let mut guard = self.connections.write().await;
        guard.insert(name, Arc::clone(&arc));
        arc
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

    /// Get the per-server connect lock. Callers hold this while establishing
    /// a new connection to prevent duplicate connections from concurrent calls.
    pub async fn connect_lock(&self, name: &str) -> Arc<Mutex<()>> {
        // Fast path: lock already exists
        {
            let guard = self.connect_locks.read().await;
            if let Some(lock) = guard.get(name) {
                return Arc::clone(lock);
            }
        }
        // Slow path: create the lock
        let mut guard = self.connect_locks.write().await;
        Arc::clone(guard.entry(name.to_string()).or_default())
    }
}
