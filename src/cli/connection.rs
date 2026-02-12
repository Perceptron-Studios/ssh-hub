use std::path::PathBuf;

use anyhow::{anyhow, Result};

use crate::connection::ConnectionParams;
use crate::server_registry::ServerEntry;

const DEFAULT_PORT: u16 = 22;
const DEFAULT_REMOTE_PATH: &str = "~";

/// Parsed SSH connection details
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub user: String,
    pub host: String,
    pub port: u16,
    pub remote_path: String,
}

/// Parse connection string format:
///   user@host              — no path, default port
///   user@host:/path        — with path, default port
///   user@host:port         — no path, custom port
///   user@host:port:/path   — with path, custom port
///
/// # Errors
///
/// Returns an error if the connection string is malformed (missing `@`,
/// empty user/host, invalid port number, or invalid path).
pub fn parse_connection_string(conn: &str, port_override: Option<u16>) -> Result<ConnectionInfo> {
    // Split user@host from the rest (everything after the first ':')
    let (user_host, rest) = match conn.split_once(':') {
        Some(parts) => parts,
        None => (conn, ""), // no colon: just user@host
    };

    let (user, host) = user_host
        .split_once('@')
        .ok_or_else(|| anyhow!("Invalid connection string: missing '@' in user@host"))?;

    if user.is_empty() {
        return Err(anyhow!("Invalid connection string: empty username"));
    }
    if host.is_empty() {
        return Err(anyhow!("Invalid connection string: empty hostname"));
    }

    let (port, remote_path) = if rest.is_empty() {
        // user@host
        (DEFAULT_PORT, DEFAULT_REMOTE_PATH.to_string())
    } else if rest.starts_with('/') {
        // user@host:/path
        (DEFAULT_PORT, rest.to_string())
    } else if let Some((port_str, path)) = rest.split_once(':') {
        // user@host:port:/path or user@host:port:
        let port: u16 = port_str
            .parse()
            .map_err(|_| anyhow!("Invalid port number: {port_str}"))?;

        if path.is_empty() {
            (port, DEFAULT_REMOTE_PATH.to_string())
        } else if path.starts_with('/') {
            (port, path.to_string())
        } else {
            return Err(anyhow!(
                "Invalid connection string: path must start with '/'"
            ));
        }
    } else {
        // user@host:port (no second colon, rest is just a number)
        let port: u16 = rest.parse().map_err(|_| {
            anyhow!("Invalid connection string: '{rest}' is not a port number or path")
        })?;
        (port, DEFAULT_REMOTE_PATH.to_string())
    };

    Ok(ConnectionInfo {
        user: user.to_string(),
        host: host.to_string(),
        port: port_override.unwrap_or(port),
        remote_path,
    })
}

/// Build `ConnectionParams` from a `ServerEntry` (config file)
#[must_use]
pub fn params_from_config(name: &str, entry: &ServerEntry) -> ConnectionParams {
    ConnectionParams {
        host: entry.host.clone(),
        user: entry.user.clone(),
        port: entry.port,
        remote_path: entry.remote_path.clone(),
        identity: entry
            .identity
            .as_ref()
            .map(|p| PathBuf::from(shellexpand_tilde(p))),
        auth_method: entry.auth.clone(),
        server_name: Some(name.to_string()),
    }
}

/// Simple tilde expansion for paths
fn shellexpand_tilde(path: &str) -> String {
    if let Some(suffix) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return format!("{}/{suffix}", home.display());
        }
    }
    path.to_string()
}
