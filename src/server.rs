use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use anyhow::Result;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo};
use rmcp::{tool, tool_handler, tool_router, ServerHandler, ServiceExt};
use tokio::io::{stdin, stdout};
use tokio::sync::RwLock;

use crate::server_registry::ServerRegistry;
use crate::connection::{ConnectionPool, SshConnection};
use crate::tools;

/// MCP server for remote SSH sessions — manages multiple simultaneous connections.
#[derive(Clone)]
pub struct RemoteSessionServer {
    pool: Arc<ConnectionPool>,
    config: Arc<RwLock<ServerRegistry>>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl RemoteSessionServer {
    pub fn new(config: ServerRegistry) -> Self {
        Self {
            pool: Arc::new(ConnectionPool::new()),
            config: Arc::new(RwLock::new(config)),
            tool_router: Self::tool_router(),
        }
    }

    // ── Management Tools ──────────────────────────────────────────────

    #[tool(description = "List pre-configured and currently connected servers. Use this to discover available servers before connecting.")]
    async fn list_servers(&self, Parameters(input): Parameters<tools::ListServersInput>) -> String {
        tools::list_servers::handler::handle(&self.pool, &self.config, input).await
    }

    #[tool(description = "Connect to a remote server via SSH. Required before using any remote_* or sync_* tool. Use 'name' alone for pre-configured servers, or provide a 'connection' string for ad-hoc connections (e.g., user@host:/path).")]
    async fn connect(&self, Parameters(input): Parameters<tools::ConnectInput>) -> String {
        tools::connect::handler::handle(&self.pool, &self.config, input).await
    }

    #[tool(description = "Disconnect from a connected remote server. Closes the SSH session.")]
    async fn disconnect(&self, Parameters(input): Parameters<tools::DisconnectInput>) -> String {
        tools::disconnect::handler::handle(&self.pool, input).await
    }

    // ── Remote Tools ──────────────────────────────────────────────────

    #[tool(description = "Execute a shell command on a remote server. Commands run from the connection's base path. Use this for git operations, build tools, process management, and any other shell task on the remote machine.")]
    async fn remote_bash(&self, Parameters(input): Parameters<tools::RemoteBashInput>) -> String {
        let server = input.server.clone();
        self.with_connection(&server, |conn| async { tools::remote_bash::handler::handle(conn, input).await }).await
    }

    #[tool(description = "Read a file from a remote server. Returns contents with line numbers. For pulling multiple files or directories to the local machine, use sync_pull instead.")]
    async fn remote_read(&self, Parameters(input): Parameters<tools::RemoteReadInput>) -> String {
        let server = input.server.clone();
        self.with_connection(&server, |conn| async { tools::remote_read::handler::handle(conn, input).await }).await
    }

    #[tool(description = "Write content to a file on a remote server. Overwrites the file if it exists. For pushing multiple files or directories from local, use sync_push instead.")]
    async fn remote_write(&self, Parameters(input): Parameters<tools::RemoteWriteInput>) -> String {
        let server = input.server.clone();
        self.with_connection(&server, |conn| async { tools::remote_write::handler::handle(conn, input).await }).await
    }

    #[tool(description = "Edit a file on a remote server using exact string replacement. The old_string must match uniquely in the file. Use replace_all to change every occurrence.")]
    async fn remote_edit(&self, Parameters(input): Parameters<tools::RemoteEditInput>) -> String {
        let server = input.server.clone();
        self.with_connection(&server, |conn| async { tools::remote_edit::handler::handle(conn, input).await }).await
    }

    #[tool(description = "Search for files matching a glob pattern on a remote server. Returns matching file paths relative to the search directory.")]
    async fn remote_glob(&self, Parameters(input): Parameters<tools::RemoteGlobInput>) -> String {
        let server = input.server.clone();
        self.with_connection(&server, |conn| async { tools::remote_glob::handler::handle(conn, input).await }).await
    }

    // ── Sync Tools ────────────────────────────────────────────────────

    #[tool(description = "Push local file(s) to a connected remote server. Supports single files and entire directories. Directory transfers use tar streaming for efficiency (one round-trip regardless of file count). Use the 'files' parameter to push a subset of a directory.")]
    async fn sync_push(&self, Parameters(input): Parameters<tools::SyncPushInput>) -> String {
        let server = input.server.clone();
        self.with_connection(&server, |conn| async { tools::sync_push::handler::handle(conn, input).await }).await
    }

    #[tool(description = "Pull remote file(s) from a connected server to the local machine. Supports single files and entire directories. Directory transfers use tar streaming for efficiency (one round-trip regardless of file count). Use the 'files' parameter to pull a subset of a directory.")]
    async fn sync_pull(&self, Parameters(input): Parameters<tools::SyncPullInput>) -> String {
        let server = input.server.clone();
        self.with_connection(&server, |conn| async { tools::sync_pull::handler::handle(conn, input).await }).await
    }

    // ── Internals ─────────────────────────────────────────────────────

    /// Execute a closure with a named connection, returning a clear error if not connected.
    async fn with_connection(&self, server: &str, f: impl AsyncConnectionFn) -> String {
        match self.pool.get(server).await {
            Some(conn) => f.call(conn).await,
            None => {
                let connected = self.pool.list().await;
                if connected.is_empty() {
                    format!(
                        "Error: server '{}' is not connected. No servers are currently connected. Use the connect tool first.",
                        server
                    )
                } else {
                    format!(
                        "Error: server '{}' is not connected. Connected servers: {}. Use the connect tool first.",
                        server,
                        connected.join(", ")
                    )
                }
            }
        }
    }

    /// Run the MCP server on stdio.
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
                 Workflow: connect -> use remote_*/sync_* tools -> disconnect.\n\
                 All remote_* and sync_* tools require a 'server' parameter — the alias of a connected server."
                    .to_string(),
            ),
        }
    }
}
