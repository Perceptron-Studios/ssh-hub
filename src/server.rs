use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::SystemTime;

use anyhow::Result;
use futures::future::join_all;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo};
use rmcp::{tool, tool_handler, tool_router, ServerHandler, ServiceExt};
use tokio::io::{stdin, stdout};
use tokio::sync::RwLock;

use crate::cli::params_from_config;
use crate::connection::{ConnectionParams, ConnectionPool, SshConnection};
use crate::server_registry::ServerRegistry;
use crate::tools;

/// MCP server for remote SSH sessions — manages multiple simultaneous connections.
#[derive(Clone)]
pub struct RemoteSessionServer {
    pool: Arc<ConnectionPool>,
    config: Arc<RwLock<ServerRegistry>>,
    config_mtime: Arc<RwLock<Option<SystemTime>>>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl RemoteSessionServer {
    #[must_use]
    pub fn new(config: ServerRegistry) -> Self {
        let initial_mtime = ServerRegistry::config_path()
            .ok()
            .and_then(|p| std::fs::metadata(p).ok())
            .and_then(|m| m.modified().ok());

        Self {
            pool: Arc::new(ConnectionPool::new()),
            config: Arc::new(RwLock::new(config)),
            config_mtime: Arc::new(RwLock::new(initial_mtime)),
            tool_router: Self::tool_router(),
        }
    }

    // ── Management Tools ──────────────────────────────────────────────

    #[tool(
        description = "List pre-configured and currently connected servers. Use this to discover available servers before connecting. Includes reachability probe (TCP to SSH port) by default."
    )]
    async fn list_servers(&self, Parameters(input): Parameters<tools::ListServersInput>) -> String {
        self.maybe_reload_config().await;
        tools::list_servers::handler::handle(&self.pool, &self.config, input).await
    }

    // ── Remote Tools ──────────────────────────────────────────────────

    #[tool(
        description = "Execute a shell command on a remote server. Commands run from the connection's base path. Use this for git operations, build tools, process management, and any other shell task on the remote machine. Set run_in_background=true for long-running commands — returns immediately with a PID and log file path instead of waiting for completion."
    )]
    async fn remote_bash(&self, Parameters(input): Parameters<tools::RemoteBashInput>) -> String {
        let server = input.server.clone();
        self.with_connection(&server, |conn| async {
            tools::remote_bash::handler::handle(conn, input).await
        })
        .await
    }

    #[tool(
        description = "Read a file from a remote server. Returns contents with line numbers. For pulling multiple files or directories to the local machine, use sync_pull instead."
    )]
    async fn remote_read(&self, Parameters(input): Parameters<tools::RemoteReadInput>) -> String {
        let server = input.server.clone();
        self.with_connection(&server, |conn| async {
            tools::remote_read::handler::handle(conn, input).await
        })
        .await
    }

    #[tool(
        description = "Write content to a file on a remote server. Overwrites the file if it exists. For pushing multiple files or directories from local, use sync_push instead."
    )]
    async fn remote_write(&self, Parameters(input): Parameters<tools::RemoteWriteInput>) -> String {
        let server = input.server.clone();
        self.with_connection(&server, |conn| async {
            tools::remote_write::handler::handle(conn, input).await
        })
        .await
    }

    #[tool(
        description = "Edit a file on a remote server using exact string replacement. The old_string must match uniquely in the file. Use replace_all to change every occurrence."
    )]
    async fn remote_edit(&self, Parameters(input): Parameters<tools::RemoteEditInput>) -> String {
        let server = input.server.clone();
        self.with_connection(&server, |conn| async {
            tools::remote_edit::handler::handle(conn, input).await
        })
        .await
    }

    #[tool(
        description = "Search for files matching a glob pattern on a remote server. Returns matching file paths relative to the search directory."
    )]
    async fn remote_glob(&self, Parameters(input): Parameters<tools::RemoteGlobInput>) -> String {
        let server = input.server.clone();
        self.with_connection(&server, |conn| async {
            tools::remote_glob::handler::handle(conn, input).await
        })
        .await
    }

    // ── Sync Tools ────────────────────────────────────────────────────

    #[tool(
        description = "Push local file(s) to a connected remote server. Supports single files and entire directories. Directory walks respect .gitignore rules and skip symlinks. Use the 'exclude' parameter for additional exclusion patterns (gitignore syntax)."
    )]
    async fn sync_push(&self, Parameters(input): Parameters<tools::SyncPushInput>) -> String {
        let server = input.server.clone();
        self.with_connection(&server, |conn| async {
            tools::sync_push::handler::handle(conn, input).await
        })
        .await
    }

    #[tool(
        description = "Pull remote file(s) from a connected server to the local machine. Supports single files and entire directories. Use the 'files' parameter to pull a subset of a directory."
    )]
    async fn sync_pull(&self, Parameters(input): Parameters<tools::SyncPullInput>) -> String {
        let server = input.server.clone();
        self.with_connection(&server, |conn| async {
            tools::sync_pull::handler::handle(conn, input).await
        })
        .await
    }

    // ── Internals ─────────────────────────────────────────────────────

    /// Execute a closure with a named connection, auto-connecting from config if needed.
    ///
    /// After execution, checks if the connection died during the operation and
    /// removes it from the pool so the next call triggers auto-reconnect.
    ///
    /// A per-server lock serializes connection establishment so concurrent tool
    /// calls don't race to create duplicate SSH connections.
    async fn with_connection(&self, server: &str, f: impl AsyncConnectionFn) -> String {
        self.maybe_reload_config().await;
        let conn = match self.resolve_connection(server).await {
            Ok(conn) => conn,
            Err(msg) => return msg,
        };
        let conn_ref = Arc::clone(&conn);
        let result = f.call(conn).await;
        self.cleanup_if_dead(server, &conn_ref).await;
        result
    }

    /// Resolve a connection for the given server: return from pool, or
    /// auto-connect from config under a per-server lock.
    async fn resolve_connection(&self, server: &str) -> Result<Arc<SshConnection>, String> {
        // Fast path: already in the pool — no lock needed.
        if let Some(conn) = self.pool.get(server).await {
            return Ok(conn);
        }

        // Server not in pool — check config for auto-connect or produce an error.
        let params = {
            let cfg = self.config.read().await;
            if let Some(entry) = cfg.get(server) {
                params_from_config(server, entry)
            } else {
                let names: Vec<&str> = cfg.servers.keys().map(String::as_str).collect();
                return Err(if names.is_empty() {
                    format!(
                        "Error: server '{server}' not found. No servers are configured. \
                         Add servers via 'ssh-hub add <name> <connection>'."
                    )
                } else {
                    format!(
                        "Error: server '{}' not found. Configured servers: {}.",
                        server,
                        names.join(", ")
                    )
                });
            }
        };

        // Acquire per-server lock to prevent concurrent connect races.
        let lock = self.pool.connect_lock(server).await;
        let _guard = lock.lock().await;

        // Re-check pool — another task may have connected while we waited.
        if let Some(conn) = self.pool.get(server).await {
            return Ok(conn);
        }

        // Auto-connect from config
        self.try_auto_connect(server, params).await.map_err(|e| {
            format!("Error: server '{server}' is configured but auto-connect failed: {e}")
        })
    }

    /// Remove a connection from the pool if it died during an operation.
    async fn cleanup_if_dead(&self, server: &str, conn: &SshConnection) {
        if conn.is_closed().await {
            tracing::debug!("Connection '{server}' died during operation, removing from pool");
            drop(self.pool.remove(server).await);
        }
    }

    /// Connect to a server using pre-resolved params and add it to the pool.
    async fn try_auto_connect(
        &self,
        server: &str,
        params: ConnectionParams,
    ) -> Result<Arc<SshConnection>> {
        tracing::info!("Auto-connecting to configured server '{}'", server);
        let conn = SshConnection::connect(params).await?;
        Ok(self.pool.insert(server.to_string(), conn).await)
    }

    /// Check if the config file has been modified since last load, and reload
    /// if so. Only evicts connections for servers whose connection-relevant
    /// fields changed or that were removed — unchanged servers keep their
    /// pooled connections.
    async fn maybe_reload_config(&self) {
        let Ok(path) = ServerRegistry::config_path() else {
            return;
        };

        let Ok(current_mtime) = std::fs::metadata(&path).and_then(|m| m.modified()) else {
            return;
        };

        // Fast path: mtime unchanged (read lock only).
        {
            let stored = self.config_mtime.read().await;
            if *stored == Some(current_mtime) {
                return;
            }
        }

        // Mtime changed — reload config from disk.
        let new_config = match ServerRegistry::load() {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("Config file changed but reload failed: {e}");
                return;
            }
        };

        // Diff old vs new to find servers that need reconnection.
        let servers_to_evict: Vec<String> = {
            let old_cfg = self.config.read().await;
            old_cfg.changed_servers(&new_config)
        };

        tracing::info!(
            "Config reloaded ({} servers, {} connections evicted)",
            new_config.servers.len(),
            servers_to_evict.len(),
        );

        // Swap in new config.
        {
            let mut cfg = self.config.write().await;
            *cfg = new_config;
        }

        // Update stored mtime.
        {
            let mut stored = self.config_mtime.write().await;
            *stored = Some(current_mtime);
        }

        // Evict and cleanly disconnect changed/removed servers concurrently.
        if !servers_to_evict.is_empty() {
            let mut futs = Vec::new();
            for name in &servers_to_evict {
                if let Some(conn) = self.pool.remove(name).await {
                    tracing::debug!("Evicting connection '{name}' (config changed)");
                    futs.push(async move { conn.disconnect().await });
                }
            }
            join_all(futs).await;
        }
    }

    /// Run the MCP server on stdio.
    ///
    /// # Errors
    ///
    /// Returns an error if the stdio transport or MCP service fails.
    pub async fn run(self) -> Result<()> {
        let transport = (stdin(), stdout());
        tracing::info!("Starting MCP server on stdio");
        let service = self.serve(transport).await?;
        service.waiting().await?;
        Ok(())
    }
}

/// Trait to allow passing async closures to `with_connection`.
trait AsyncConnectionFn: Send + 'static {
    fn call(self, conn: Arc<SshConnection>) -> Pin<Box<dyn Future<Output = String> + Send>>;
}

impl<F, Fut> AsyncConnectionFn for F
where
    F: FnOnce(Arc<SshConnection>) -> Fut + Send + 'static,
    Fut: Future<Output = String> + Send + 'static,
{
    fn call(self, conn: Arc<SshConnection>) -> Pin<Box<dyn Future<Output = String> + Send>> {
        Box::pin(self(conn))
    }
}

#[tool_handler]
impl ServerHandler for RemoteSessionServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::LATEST,
            capabilities: ServerCapabilities {
                tools: Some(rmcp::model::ToolsCapability { list_changed: None }),
                ..Default::default()
            },
            server_info: Implementation::from_build_env(),
            instructions: Some(
                "MCP server for remote SSH sessions. Supports multiple simultaneous connections.\n\
                 IMPORTANT: These tools operate on REMOTE servers over SSH — not the local machine. \
                 You already have local tools for local operations. Before using any remote tool, \
                 decide whether the target belongs to the local environment or a remote server.\n\
                 Workflow: list_servers to discover available servers -> use remote_*/sync_* tools (auto-connects configured servers).\n\
                 All remote_* and sync_* tools require a 'server' parameter — the name of a configured server.\n\
                 Troubleshooting: the `ssh-hub` CLI is available locally for server management \
                 (add, remove, update). Run `ssh-hub --help` for details."
                    .to_string(),
            ),
        }
    }
}
