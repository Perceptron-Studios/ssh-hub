use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use russh::client::{self, Handle};
use russh::keys::PublicKey;
use russh::ChannelMsg;
use tokio::sync::Mutex;

use crate::server_registry::AuthMethod;
use super::auth;

/// Stdin is written to the SSH channel in chunks of this size.
const STDIN_CHUNK_SIZE: usize = 32 * 1024;

/// Default timeout for single-file read/write operations (1 minute).
const FILE_IO_TIMEOUT_MS: u64 = 60_000;

/// Default timeout for glob/find operations (30 seconds).
const GLOB_TIMEOUT_MS: u64 = 30_000;

/// Maximum number of files returned by a glob operation.
const GLOB_MAX_RESULTS: usize = 1000;

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

    // TODO: Implement proper host key verification
    async fn check_server_key(
        &mut self,
        _server_public_key: &PublicKey,
    ) -> Result<bool, Self::Error> {
        tracing::debug!("Accepting server key without verification");
        Ok(true)
    }
}

/// Raw byte output collected from a channel.
struct ChannelOutput {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    exit_code: i32,
}

/// Manages an SSH connection to a remote host.
pub struct SshConnection {
    session: Arc<Mutex<Handle<SshHandler>>>,
    params: ConnectionParams,
}

impl SshConnection {
    /// Establish a new SSH connection.
    pub async fn connect(params: ConnectionParams) -> Result<Self> {
        tracing::debug!(
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

        tracing::debug!("SSH connection established");

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

    /// Open a channel, execute a command, and collect all output with an optional timeout.
    ///
    /// If `stdin_data` is provided, it is written to the channel in
    /// [`STDIN_CHUNK_SIZE`] chunks before reading output.
    async fn run_channel(
        &self,
        command: &str,
        stdin_data: Option<&[u8]>,
        timeout_ms: Option<u64>,
    ) -> Result<ChannelOutput> {
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

        // Write stdin if provided
        if let Some(data) = stdin_data {
            for chunk in data.chunks(STDIN_CHUNK_SIZE) {
                channel
                    .data(chunk)
                    .await
                    .context("Failed to write to stdin")?;
            }
            channel.eof().await.context("Failed to send EOF")?;
        }

        // Collect output
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut exit_code = None;

        let read_loop = async {
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

        if let Some(ms) = timeout_ms {
            let timeout = tokio::time::Duration::from_millis(ms);
            tokio::time::timeout(timeout, read_loop)
                .await
                .context("Command timed out")??;
        } else {
            read_loop.await?;
        }

        Ok(ChannelOutput {
            stdout,
            stderr,
            exit_code: exit_code.unwrap_or(-1),
        })
    }

    /// Execute a command on the remote machine.
    ///
    /// # Errors
    /// Returns an error if the SSH channel cannot be opened, the command
    /// fails to start, or the optional timeout expires.
    pub async fn exec(&self, command: &str, timeout_ms: Option<u64>) -> Result<ExecResult> {
        let output = self.run_channel(command, None, timeout_ms).await?;
        Ok(ExecResult {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.exit_code,
        })
    }

    /// Execute a command with raw byte output and optional stdin piping.
    ///
    /// # Errors
    /// Returns an error if the SSH channel cannot be opened, stdin data
    /// fails to write, or the optional timeout expires.
    pub async fn exec_raw(
        &self,
        command: &str,
        stdin_data: Option<&[u8]>,
        timeout_ms: Option<u64>,
    ) -> Result<ExecRawResult> {
        let output = self.run_channel(command, stdin_data, timeout_ms).await?;
        Ok(ExecRawResult {
            stdout: output.stdout,
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.exit_code,
        })
    }

    /// Read a file from the remote machine.
    ///
    /// # Errors
    /// Returns an error if the remote `cat` command fails or the file does not exist.
    pub async fn read_file(&self, path: &str) -> Result<String> {
        let result = self.exec(&format!("cat '{}'", path), Some(FILE_IO_TIMEOUT_MS)).await?;
        if result.exit_code != 0 {
            return Err(anyhow!("Failed to read file: {}", result.stderr));
        }
        Ok(result.stdout)
    }

    /// Write content to a file on the remote machine.
    ///
    /// # Errors
    /// Returns an error if the remote write command fails.
    pub async fn write_file(&self, path: &str, content: &str) -> Result<()> {
        let command = format!(
            "cat > '{}' << 'CLAUDE_REMOTE_EOF'\n{}\nCLAUDE_REMOTE_EOF",
            path, content
        );
        let result = self.exec(&command, Some(FILE_IO_TIMEOUT_MS)).await?;
        if result.exit_code != 0 {
            return Err(anyhow!("Failed to write file: {}", result.stderr));
        }
        Ok(())
    }

    /// List files matching a glob pattern.
    ///
    /// # Errors
    /// Returns an error if the remote `find` command fails.
    pub async fn glob(&self, pattern: &str, base_path: Option<&str>) -> Result<Vec<String>> {
        let path = base_path.unwrap_or(&self.params.remote_path);
        let result = self
            .exec(
                &format!(
                    "cd '{}' && find . -path '{}' -type f 2>/dev/null | head -{}",
                    path, pattern, GLOB_MAX_RESULTS
                ),
                Some(GLOB_TIMEOUT_MS),
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

/// Result of executing a command with raw byte output.
#[derive(Debug, Clone)]
pub struct ExecRawResult {
    pub stdout: Vec<u8>,
    pub stderr: String,
    pub exit_code: i32,
}
