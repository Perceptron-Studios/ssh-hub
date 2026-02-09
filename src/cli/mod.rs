mod add;
mod list;
mod mcp_install;
mod remove;
mod update;

use std::path::PathBuf;

use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};

use crate::connection::ConnectionParams;
use crate::server_registry::ServerEntry;

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
    /// Add a server to the config (or reconfigure an existing one)
    Add {
        /// Server name (alias for config)
        name: String,

        /// SSH connection string: user@host:/path or user@host:port:/path
        connection: String,

        /// SSH port override
        #[arg(short = 'p', long)]
        port: Option<u16>,

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

    /// Register ssh-hub as an MCP server in a project directory
    #[command(name = "mcp-install")]
    McpInstall {
        /// Target project directory (default: current working directory)
        #[arg(default_value = ".")]
        directory: PathBuf,

        /// Configure for Claude Code (.mcp.json)
        #[arg(long)]
        claude: bool,

        /// Configure for Codex (.codex/config.toml)
        #[arg(long)]
        codex: bool,
    },

    /// Update ssh-hub to the latest release
    Update {
        /// Check for updates without installing
        #[arg(long)]
        check: bool,
    },
}

/// Dispatch a CLI command to its handler.
///
/// # Errors
///
/// Returns an error if the command's underlying operation fails (I/O, network,
/// config parse, etc.).
pub async fn run(command: Command) -> Result<()> {
    match command {
        Command::Add {
            name,
            connection,
            port,
            identity,
        } => add::run(name, connection, port, identity).await,

        Command::Remove { name } => remove::run(&name),

        Command::List => {
            list::run();
            Ok(())
        }

        Command::McpInstall {
            directory,
            claude,
            codex,
        } => mcp_install::run(&directory, claude, codex),

        Command::Update { check } => update::run(check),
    }
}

// ── Connection string parsing ────────────────────────────────────────

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
    if path.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            return format!("{}{}", home.display(), &path[1..]);
        }
    }
    path.to_string()
}
