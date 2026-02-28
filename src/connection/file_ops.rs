//! File I/O convenience methods for [`SshConnection`].
//!
//! Wrappers around [`exec`](SshConnection::exec) and
//! [`exec_raw`](SshConnection::exec_raw) for common remote file operations.

use anyhow::{anyhow, Result};

use crate::utils::path::{shell_escape, shell_escape_remote_path};

use super::SshConnection;

/// Default timeout for single-file read/write operations (1 minute).
const FILE_IO_TIMEOUT_MS: u64 = 60_000;

/// Default timeout for glob/find operations (30 seconds).
const GLOB_TIMEOUT_MS: u64 = 30_000;

/// Maximum number of files returned by a glob operation.
const GLOB_MAX_RESULTS: usize = 1000;

impl SshConnection {
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
        let path = base_path.unwrap_or(&self.params().remote_path);
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

        // find piped through head can return non-zero (SIGPIPE) even on success,
        // so only treat it as an error if stderr has content.
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
