use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use russh::client::{self, Handle};
use russh::keys::PublicKey;
use russh::ChannelMsg;
use tokio::sync::Mutex;

use crate::server_registry::AuthMethod;
use crate::utils::path::{shell_escape, shell_escape_remote_path};

use super::auth;

/// Stdin is written to the SSH channel in chunks of this size.
const STDIN_CHUNK_SIZE: usize = 32 * 1024;

/// Default timeout for single-file read/write operations (1 minute).
const FILE_IO_TIMEOUT_MS: u64 = 60_000;

/// Default timeout for glob/find operations (30 seconds).
const GLOB_TIMEOUT_MS: u64 = 30_000;

/// Maximum number of files returned by a glob operation.
const GLOB_MAX_RESULTS: usize = 1000;

/// Interval between SSH keepalive probes.
const KEEPALIVE_INTERVAL_SECS: u64 = 30;

/// Number of missed keepalive responses before declaring the connection dead.
const KEEPALIVE_MAX_FAILURES: usize = 3;

/// Timeout for opening a new SSH channel. If `channel_open_session()` doesn't
/// complete within this time, the connection is considered dead.
const CHANNEL_OPEN_TIMEOUT_SECS: u64 = 10;

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

/// SSH client handler for russh — carries host info for key verification.
pub(super) struct SshHandler {
    host: String,
    port: u16,
}

impl SshHandler {
    pub fn new(host: String, port: u16) -> Self {
        Self { host, port }
    }
}

impl client::Handler for SshHandler {
    type Error = anyhow::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &PublicKey,
    ) -> Result<bool, Self::Error> {
        use russh::keys::known_hosts;

        match known_hosts::check_known_hosts(&self.host, self.port, server_public_key) {
            Ok(true) => {
                tracing::debug!("Host key verified for {}:{}", self.host, self.port);
                Ok(true)
            }
            Ok(false) => {
                // TOFU: first time seeing this host — learn the key
                tracing::info!(
                    "New host key for {}:{}, adding to known_hosts",
                    self.host,
                    self.port
                );
                if let Err(e) =
                    known_hosts::learn_known_hosts(&self.host, self.port, server_public_key)
                {
                    tracing::warn!("Failed to save host key to known_hosts: {}", e);
                }
                Ok(true)
            }
            Err(russh::keys::Error::KeyChanged { line }) => Err(anyhow!(
                "HOST KEY VERIFICATION FAILED for {}:{}. \
                     The server's key has changed since it was last recorded \
                     (known_hosts line {}). This could indicate a man-in-the-middle attack. \
                     If the server was legitimately reinstalled, remove line {} from \
                     ~/.ssh/known_hosts and reconnect.",
                self.host,
                self.port,
                line,
                line
            )),
            Err(e) => {
                tracing::warn!(
                    "Could not verify host key for {}:{}: {}. Accepting.",
                    self.host,
                    self.port,
                    e
                );
                Ok(true)
            }
        }
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
    force_closed: Arc<AtomicBool>,
}

impl SshConnection {
    /// Establish a new SSH connection.
    ///
    /// # Errors
    ///
    /// Returns an error if the TCP connection, SSH handshake, or
    /// authentication fails.
    pub async fn connect(params: ConnectionParams) -> Result<Self> {
        tracing::debug!(
            "Connecting to {}@{}:{} (path: {})",
            params.user,
            params.host,
            params.port,
            params.remote_path,
        );

        let config = Arc::new(client::Config {
            keepalive_interval: Some(Duration::from_secs(KEEPALIVE_INTERVAL_SECS)),
            keepalive_max: KEEPALIVE_MAX_FAILURES,
            ..client::Config::default()
        });
        let handler = SshHandler::new(params.host.clone(), params.port);

        let mut session = client::connect(config, (params.host.as_str(), params.port), handler)
            .await
            .context("Failed to connect to SSH server")?;

        auth::authenticate(&mut session, &params).await?;

        tracing::debug!("SSH connection established");

        Ok(Self {
            session: Arc::new(Mutex::new(session)),
            params,
            force_closed: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Get the base remote path for this connection.
    #[must_use]
    pub fn remote_path(&self) -> &str {
        &self.params.remote_path
    }

    /// Get the connection parameters.
    #[must_use]
    pub fn params(&self) -> &ConnectionParams {
        &self.params
    }

    /// Check whether the underlying SSH session has been closed.
    ///
    /// Returns `true` if the session was explicitly marked dead (e.g. channel
    /// open timeout) or if russh reports the session as closed.
    pub async fn is_closed(&self) -> bool {
        if self.force_closed.load(Ordering::Relaxed) {
            return true;
        }
        let session = self.session.lock().await;
        session.is_closed()
    }

    /// Mark this connection as dead. Subsequent `is_closed()` calls return
    /// `true` without acquiring the session mutex.
    pub fn mark_closed(&self) {
        self.force_closed.store(true, Ordering::Relaxed);
    }

    /// Open a channel, execute a command, and collect all output with an optional timeout.
    ///
    /// If `stdin_data` is provided, it is written to the channel in
    /// [`STDIN_CHUNK_SIZE`] chunks before reading output.
    ///
    /// The session mutex is held only for `channel_open_session` — all
    /// subsequent I/O uses the independent `Channel`, allowing concurrent
    /// commands over the same SSH connection.
    async fn run_channel(
        &self,
        command: &str,
        stdin_data: Option<&[u8]>,
        timeout_ms: Option<u64>,
    ) -> Result<ChannelOutput> {
        // Lock ONLY for channel creation, then drop.
        // Timeout prevents hanging on dead connections (e.g. after OS suspend).
        let mut channel = if let Ok(result) =
            tokio::time::timeout(Duration::from_secs(CHANNEL_OPEN_TIMEOUT_SECS), async {
                let session = self.session.lock().await;
                session
                    .channel_open_session()
                    .await
                    .context("Failed to open channel")
            })
            .await
        {
            result?
        } else {
            tracing::warn!(
                "Channel open timed out after {CHANNEL_OPEN_TIMEOUT_SECS}s, \
                 connection likely dead"
            );
            self.mark_closed();
            return Err(anyhow!(
                "Timed out opening SSH channel ({CHANNEL_OPEN_TIMEOUT_SECS}s). \
                 The connection may be dead — retry to auto-reconnect."
            ));
        };

        let full_command = format!(
            "cd {} && {}",
            shell_escape_remote_path(&self.params.remote_path),
            command,
        );

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
                        exit_code = Some(exit_status.cast_signed());
                    }
                    None => break,
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
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
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
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            exit_code: output.exit_code,
        })
    }

    /// Read a file as raw bytes from the remote machine.
    ///
    /// # Errors
    /// Returns an error if the remote `cat` command fails or the file does not exist.
    pub async fn read_file_raw(&self, path: &str) -> Result<Vec<u8>> {
        let command = format!("cat {}", shell_escape_remote_path(path));
        let result = self
            .exec_raw(&command, None, Some(FILE_IO_TIMEOUT_MS))
            .await?;
        if result.exit_code != 0 {
            return Err(anyhow!("Failed to read file: {}", result.stderr));
        }
        Ok(result.stdout)
    }

    /// Read a file as UTF-8 text from the remote machine.
    ///
    /// Invalid UTF-8 sequences are replaced with the Unicode replacement character.
    ///
    /// # Errors
    /// Returns an error if the remote `cat` command fails or the file does not exist.
    pub async fn read_file(&self, path: &str) -> Result<String> {
        let bytes = self.read_file_raw(path).await?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// Write raw bytes to a file on the remote machine.
    ///
    /// Uses stdin piping instead of heredoc to avoid delimiter collisions.
    ///
    /// # Errors
    /// Returns an error if the remote write command fails.
    pub async fn write_file_raw(&self, path: &str, content: &[u8]) -> Result<()> {
        let escaped_path = shell_escape_remote_path(path);
        let command = format!("cat > {escaped_path}");
        let result = self
            .exec_raw(&command, Some(content), Some(FILE_IO_TIMEOUT_MS))
            .await?;
        if result.exit_code != 0 {
            return Err(anyhow!("Failed to write file: {}", result.stderr));
        }
        Ok(())
    }

    /// Write UTF-8 text to a file on the remote machine.
    ///
    /// # Errors
    /// Returns an error if the remote write command fails.
    pub async fn write_file(&self, path: &str, content: &str) -> Result<()> {
        self.write_file_raw(path, content.as_bytes()).await
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
                    "cd {} && find . -path {} -type f 2>/dev/null | head -{}",
                    shell_escape_remote_path(path),
                    shell_escape(pattern),
                    GLOB_MAX_RESULTS
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
