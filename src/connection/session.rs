use anyhow::{anyhow, Context, Result};
use russh::client::{self, Handle};
use russh::keys::PublicKey;
use russh::ChannelMsg;
use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::server_registry::AuthMethod;
use super::auth;

/// Parameters needed to establish an SSH connection.
/// Decoupled from CLI args — can be built from config, MCP tool input, or CLI.
#[derive(Debug, Clone)]
pub struct ConnectionParams {
    pub host: String,
    pub user: String,
    pub port: u16,
    pub remote_path: String,
    pub identity: Option<PathBuf>,
    pub auth_method: AuthMethod,
    /// Server alias — used for keychain lookups.
    pub server_name: Option<String>,
}

/// SSH client handler for russh.
pub(super) struct SshHandler;

impl client::Handler for SshHandler {
    type Error = anyhow::Error;

    fn check_server_key(
        &mut self,
        _server_public_key: &PublicKey,
    ) -> impl Future<Output = Result<bool, Self::Error>> + Send {
        // TODO: Implement proper host key verification
        async {
            tracing::warn!("Accepting server key without verification");
            Ok(true)
        }
    }
}

/// Manages an SSH connection to a remote host.
pub struct SshConnection {
    session: Arc<Mutex<Handle<SshHandler>>>,
    params: ConnectionParams,
}

impl SshConnection {
    /// Establish a new SSH connection.
    pub async fn connect(params: ConnectionParams) -> Result<Self> {
        tracing::info!(
            "Connecting to {}@{}:{} (path: {})",
            params.user,
            params.host,
            params.port,
            params.remote_path,
        );

        let config = Arc::new(client::Config::default());
        let handler = SshHandler;

        let mut session =
            client::connect(config, (params.host.as_str(), params.port), handler)
                .await
                .context("Failed to connect to SSH server")?;

        auth::authenticate(&mut session, &params).await?;

        tracing::info!("SSH connection established");

        Ok(Self {
            session: Arc::new(Mutex::new(session)),
            params,
        })
    }

    /// Get the base remote path for this connection.
    pub fn remote_path(&self) -> &str {
        &self.params.remote_path
    }

    /// Get the connection parameters.
    pub fn params(&self) -> &ConnectionParams {
        &self.params
    }

    /// Execute a command on the remote machine.
    pub async fn exec(&self, command: &str, timeout_ms: Option<u64>) -> Result<ExecResult> {
        let session = self.session.lock().await;

        let mut channel = session
            .channel_open_session()
            .await
            .context("Failed to open channel")?;

        let full_command = format!("cd {} && {}", self.params.remote_path, command);

        channel
            .exec(true, full_command)
            .await
            .context("Failed to execute command")?;

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut exit_code = None;

        let timeout = timeout_ms.map(tokio::time::Duration::from_millis);

        let result = async {
            loop {
                match channel.wait().await {
                    Some(ChannelMsg::Data { data }) => {
                        stdout.extend_from_slice(&data);
                    }
                    Some(ChannelMsg::ExtendedData { data, ext }) => {
                        if ext == 1 {
                            stderr.extend_from_slice(&data);
                        }
                    }
                    Some(ChannelMsg::ExitStatus { exit_status }) => {
                        exit_code = Some(exit_status as i32);
                    }
                    Some(ChannelMsg::Eof) | None => break,
                    _ => {}
                }
            }
            Ok::<_, anyhow::Error>(())
        };

        if let Some(timeout) = timeout {
            tokio::time::timeout(timeout, result)
                .await
                .context("Command timed out")??;
        } else {
            result.await?;
        }

        Ok(ExecResult {
            stdout: String::from_utf8_lossy(&stdout).to_string(),
            stderr: String::from_utf8_lossy(&stderr).to_string(),
            exit_code: exit_code.unwrap_or(-1),
        })
    }

    /// Read a file from the remote machine.
    pub async fn read_file(&self, path: &str) -> Result<String> {
        let result = self.exec(&format!("cat '{}'", path), Some(60000)).await?;
        if result.exit_code != 0 {
            return Err(anyhow!("Failed to read file: {}", result.stderr));
        }
        Ok(result.stdout)
    }

    /// Write content to a file on the remote machine.
    pub async fn write_file(&self, path: &str, content: &str) -> Result<()> {
        let command = format!(
            "cat > '{}' << 'CLAUDE_REMOTE_EOF'\n{}\nCLAUDE_REMOTE_EOF",
            path, content
        );
        let result = self.exec(&command, Some(60000)).await?;
        if result.exit_code != 0 {
            return Err(anyhow!("Failed to write file: {}", result.stderr));
        }
        Ok(())
    }

    /// List files matching a glob pattern.
    pub async fn glob(&self, pattern: &str, base_path: Option<&str>) -> Result<Vec<String>> {
        let path = base_path.unwrap_or(&self.params.remote_path);
        let result = self
            .exec(
                &format!(
                    "cd '{}' && find . -path '{}' -type f 2>/dev/null | head -1000",
                    path, pattern
                ),
                Some(30000),
            )
            .await?;

        if result.exit_code != 0 && !result.stderr.is_empty() {
            return Err(anyhow!("Glob failed: {}", result.stderr));
        }

        Ok(result
            .stdout
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| l.trim_start_matches("./").to_string())
            .collect())
    }
}

/// Result of executing a command.
#[derive(Debug, Clone)]
pub struct ExecResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}
