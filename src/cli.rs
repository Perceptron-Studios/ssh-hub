use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use crate::server_registry::{AuthMethod, ServerEntry};
use crate::connection::ConnectionParams;

/// MCP server for remote SSH sessions
#[derive(Parser, Debug)]
#[command(name = "ssh-hub")]
#[command(version, about)]
pub struct Cli {
    /// Enable verbose logging
    #[arg(short = 'v', long = "verbose", global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Interactive setup for a server's credentials
    Setup {
        /// Server name (alias for config)
        name: String,

        /// SSH connection string: user@host:/path or user@host:port:/path
        #[arg(long)]
        connection: Option<String>,

        /// SSH port
        #[arg(short = 'p', long)]
        port: Option<u16>,

        /// Path to SSH private key
        #[arg(short = 'i', long)]
        identity: Option<PathBuf>,
    },

    /// Add a server to the config (without interactive credential setup)
    Add {
        /// Server name (alias for config)
        name: String,

        /// SSH connection string: user@host:/path or user@host:port:/path
        connection: String,

        /// Path to SSH private key
        #[arg(short = 'i', long)]
        identity: Option<PathBuf>,
    },

    /// Remove a server from config
    Remove {
        /// Server name to remove
        name: String,
    },

    /// List all configured servers
    List,
}

/// Parsed SSH connection details
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub user: String,
    pub host: String,
    pub port: u16,
    pub remote_path: String,
}

/// Parse connection string format: user@host:/path or user@host:port:/path
pub fn parse_connection_string(conn: &str, port_override: Option<u16>) -> Result<ConnectionInfo> {
    let (user_host, rest) = conn
        .split_once(':')
        .ok_or_else(|| anyhow!("Invalid connection string: missing ':' after host"))?;

    let (user, host) = user_host
        .split_once('@')
        .ok_or_else(|| anyhow!("Invalid connection string: missing '@' in user@host"))?;

    if user.is_empty() {
        return Err(anyhow!("Invalid connection string: empty username"));
    }
    if host.is_empty() {
        return Err(anyhow!("Invalid connection string: empty hostname"));
    }

    let (port, remote_path) = if rest.starts_with('/') {
        (22, rest.to_string())
    } else {
        let (port_str, path) = rest
            .split_once(':')
            .ok_or_else(|| anyhow!("Invalid connection string: expected port:path or /path"))?;

        let port: u16 = port_str
            .parse()
            .map_err(|_| anyhow!("Invalid port number: {}", port_str))?;

        if !path.starts_with('/') {
            return Err(anyhow!(
                "Invalid connection string: path must start with '/'"
            ));
        }

        (port, path.to_string())
    };

    Ok(ConnectionInfo {
        user: user.to_string(),
        host: host.to_string(),
        port: port_override.unwrap_or(port),
        remote_path,
    })
}

/// Build ConnectionParams from a ServerEntry (config file)
pub fn params_from_config(name: &str, entry: &ServerEntry) -> ConnectionParams {
    ConnectionParams {
        host: entry.host.clone(),
        user: entry.user.clone(),
        port: entry.port,
        remote_path: entry.remote_path.clone(),
        identity: entry.identity.as_ref().map(|p| {
            PathBuf::from(shellexpand_tilde(p))
        }),
        auth_method: entry.auth.clone(),
        server_name: Some(name.to_string()),
    }
}

/// Build ConnectionParams from a connection string (ad-hoc)
pub fn params_from_connection_string(
    name: &str,
    connection: &str,
    port_override: Option<u16>,
    identity: Option<&str>,
) -> Result<ConnectionParams> {
    let info = parse_connection_string(connection, port_override)?;
    Ok(ConnectionParams {
        host: info.host,
        user: info.user,
        port: info.port,
        remote_path: info.remote_path,
        identity: identity.map(|p| PathBuf::from(shellexpand_tilde(p))),
        auth_method: AuthMethod::Auto,
        server_name: Some(name.to_string()),
    })
}

/// Simple tilde expansion for paths
fn shellexpand_tilde(path: &str) -> String {
    if path.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            return format!("{}{}", home.display(), &path[1..]);
        }
    }
    path.to_string()
}