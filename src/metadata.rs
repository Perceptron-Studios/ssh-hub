use std::time::SystemTime;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::connection::SshConnection;

const METADATA_TIMEOUT_MS: u64 = 15_000;

/// Cached system information collected from a remote server via `ssh-hub refresh`.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct SystemMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distro: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_manager: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collected_at: Option<u64>,
}

impl SystemMetadata {
    /// One-line summary of distro, architecture, and package manager.
    ///
    /// Example: `"Ubuntu 22.04 | x86_64 | apt"`.
    /// Returns `None` if none of these three fields are set.
    /// The `os` and `shell` fields are omitted because `distro` subsumes `os`
    /// (e.g., "Ubuntu 22.04" implies Linux) and `shell` adds noise to a summary.
    #[must_use]
    pub fn summary_line(&self) -> Option<String> {
        let parts: Vec<&str> = [
            self.distro.as_deref(),
            self.arch.as_deref(),
            self.package_manager.as_deref(),
        ]
        .into_iter()
        .flatten()
        .collect();
        if parts.is_empty() {
            None
        } else {
            Some(parts.join(" | "))
        }
    }

    /// Clone with `collected_at` cleared — for MCP output where the timestamp
    /// is internal bookkeeping and not useful to the LLM.
    #[must_use]
    pub fn without_timestamp(&self) -> Self {
        Self {
            collected_at: None,
            ..self.clone()
        }
    }
}

/// POSIX shell snippet that prints `KEY=VALUE` lines for system metadata.
/// Works on both Linux (via `/etc/os-release`) and macOS (via `sw_vers`).
const METADATA_COMMAND: &str = concat!(
    r#"echo "ARCH=$(uname -m)"; "#,
    r#"echo "OS=$(uname -s | tr '[:upper:]' '[:lower:]')"; "#,
    r#"if [ -f /etc/os-release ]; then "#,
    r#". /etc/os-release; "#,
    r#"echo "DISTRO=${PRETTY_NAME:-${NAME} ${VERSION}}"; "#,
    r#"elif command -v sw_vers >/dev/null 2>&1; then "#,
    r#"echo "DISTRO=macOS $(sw_vers -productVersion)"; "#,
    r#"else "#,
    r#"echo "DISTRO=unknown"; "#,
    r#"fi; "#,
    r#"echo "SHELL=$SHELL"; "#,
    r#"for pm in apt dnf yum pacman apk brew; do "#,
    r#"command -v "$pm" >/dev/null 2>&1 && echo "PKG_MANAGER=$pm" && break; "#,
    r#"done"#,
);

/// Collect system metadata from a connected server.
///
/// # Errors
///
/// Returns an error if the SSH command fails or times out.
pub async fn collect(conn: &SshConnection) -> Result<SystemMetadata> {
    let result = conn
        .exec(METADATA_COMMAND, Some(METADATA_TIMEOUT_MS))
        .await?;
    parse_output(&result.stdout)
}

/// Parse `KEY=VALUE` output into a `SystemMetadata` struct.
///
/// Missing or unknown keys are silently ignored; empty values are treated as
/// absent. Returns `Result` to allow future validation (e.g. rejecting
/// malformed output) without a breaking API change.
///
/// # Errors
///
/// Currently infallible. Reserved for future validation of malformed output.
pub fn parse_output(stdout: &str) -> Result<SystemMetadata> {
    let mut meta = SystemMetadata::default();

    for line in stdout.lines() {
        if let Some((key, value)) = line.split_once('=') {
            let value = value.trim();
            if value.is_empty() {
                continue;
            }
            match key.trim() {
                "ARCH" => meta.arch = Some(value.to_string()),
                "OS" => meta.os = Some(value.to_string()),
                "DISTRO" => meta.distro = Some(value.to_string()),
                "SHELL" => meta.shell = Some(value.to_string()),
                "PKG_MANAGER" => meta.package_manager = Some(value.to_string()),
                _ => {}
            }
        }
    }

    meta.collected_at = Some(
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );

    Ok(meta)
}

/// Compare two metadata instances, ignoring `collected_at`.
/// Returns a human-readable diff string if anything changed, or `None`.
#[must_use]
pub fn diff(old: &SystemMetadata, new: &SystemMetadata) -> Option<String> {
    let fields: &[(&str, Option<&str>, Option<&str>)] = &[
        ("os", old.os.as_deref(), new.os.as_deref()),
        ("distro", old.distro.as_deref(), new.distro.as_deref()),
        ("arch", old.arch.as_deref(), new.arch.as_deref()),
        ("shell", old.shell.as_deref(), new.shell.as_deref()),
        (
            "package_manager",
            old.package_manager.as_deref(),
            new.package_manager.as_deref(),
        ),
    ];

    let changes: Vec<String> = fields
        .iter()
        .filter(|(_, o, n)| o != n)
        .map(|(name, old, new)| {
            format!(
                "{name}: {} -> {}",
                old.unwrap_or("(none)"),
                new.unwrap_or("(none)"),
            )
        })
        .collect();

    if changes.is_empty() {
        None
    } else {
        Some(changes.join(", "))
    }
}
