mod add;
mod connection;
mod list;
mod mcp_install;
mod refresh;
mod remove;
mod spinner;
mod update;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

pub use connection::{params_from_config, parse_connection_string, ConnectionInfo};

/// MCP server for remote SSH sessions
#[derive(Parser, Debug)]
#[command(name = "ssh-hub")]
#[command(version, about)]
#[command(long_about = "\
MCP server that exposes remote file operations and shell execution over SSH.

Configured servers are stored in ~/.config/ssh-hub/servers.toml. MCP tools \
auto-connect on first use — no manual connection step needed.")]
#[command(after_long_help = "\
SSH KEY SETUP:
    Your SSH public key must be authorized on the remote server:
      ssh-copy-id -i ~/.ssh/id_ed25519.pub user@host

    For cloud VMs (GCP, AWS), bind the key via the provider's metadata service.
    See: https://github.com/Perceptron-Studios/ssh-hub/blob/main/docs/server-setup.md

    The private key must be loaded in ssh-agent or available at ~/.ssh/id_*.
    Auth order: explicit -i key → SSH agent (max 10 keys) → default key paths.

AGENTS:
    If an MCP connection fails or a server is unreachable, use these commands to
    diagnose and fix the issue from your local shell:

    ssh-hub refresh <server>
      Re-collect system metadata. Also use this to update connection settings
      when a server's IP changes (common in cloud/ephemeral environments):
        ssh-hub refresh staging --host <new-ip>
        ssh-hub refresh staging --port <new-port>

    ssh-hub add <name> user@host:/path
      Register a new server. Required before MCP tools can reach it.
      Run `ssh-hub add --help` for connection string formats.

    Other commands (list, remove, update, mcp-install) are self-explanatory
    from the descriptions above.")]
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
    #[command(long_about = "\
Add a server to the config (or reconfigure an existing one).

Tests SSH connectivity and collects system metadata (OS, arch, package manager) \
on success. Use -i for passphrase-protected keys — runs ssh-add to load the key \
into the agent (prompts once for the passphrase).")]
    #[command(after_long_help = "\
CONNECTION FORMATS:
    user@host              Port 22, path ~
    user@host:/path        Port 22, explicit path
    user@host:2222         Custom port, path ~
    user@host:2222:/path   Custom port and path

EXAMPLES:
    ssh-hub add prod deploy@10.0.0.5:/var/www
    ssh-hub add dev me@devbox
    ssh-hub add gpu root@gpu-server:2222 -i ~/.ssh/gpu_key")]
    Add {
        /// Server name (alias used in MCP tools and CLI commands)
        name: String,

        /// SSH connection string (see CONNECTION FORMATS below)
        connection: String,

        /// Override SSH port from the connection string
        #[arg(short = 'p', long)]
        port: Option<u16>,

        /// Path to SSH private key (loaded into ssh-agent via ssh-add)
        #[arg(short = 'i', long)]
        identity: Option<PathBuf>,
    },

    /// Remove a server from config. Active MCP sessions are not affected
    Remove {
        /// Server name to remove
        name: String,
    },

    /// List configured servers with connection details and system metadata
    List,

    /// Register ssh-hub as an MCP server in a project directory
    #[command(name = "mcp-install")]
    #[command(long_about = "\
Register ssh-hub as an MCP server in a project directory.

Writes the config file so Claude Code (.mcp.json) and/or Codex (.codex/config.toml) \
discover ssh-hub as an MCP server. Without --claude or --codex, configures both. \
MCP tools auto-connect to configured servers on first use.")]
    #[command(after_long_help = "\
EXAMPLES:
    ssh-hub mcp-install                           Both clients, current dir
    ssh-hub mcp-install /path/to/project --claude Claude Code only")]
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

    /// Refresh server metadata and optionally update connection settings
    #[command(long_about = "\
Refresh server metadata and optionally update connection settings.

Connects to the server and collects system metadata (OS, distro, arch, shell, \
package manager). Diffs against previously stored values and reports changes.

Connection setting overrides (--host, --port, etc.) are saved to config before \
connecting — useful for ephemeral networks where server IPs change between sessions.")]
    #[command(after_long_help = "\
EXAMPLES:
    ssh-hub refresh staging                       Refresh metadata
    ssh-hub refresh staging --host 10.0.0.99      Update IP and refresh
    ssh-hub refresh --all                         Refresh all servers")]
    Refresh {
        /// Server name to refresh
        name: Option<String>,

        /// Refresh all configured servers
        #[arg(long)]
        all: bool,

        /// Update the stored SSH host before connecting
        #[arg(long)]
        host: Option<String>,

        /// Update the stored SSH port before connecting
        #[arg(short = 'p', long)]
        port: Option<u16>,

        /// Update the stored remote base path before connecting
        #[arg(long)]
        remote_path: Option<String>,

        /// Update the stored SSH private key path before connecting
        #[arg(short = 'i', long)]
        identity: Option<PathBuf>,
    },

    /// Check for a newer release and install it via cargo install
    #[command(long_about = "\
Check GitHub for a newer release and install it via cargo install --git. \
Use --check to preview the available version without installing.")]
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

        Command::List => list::run(),

        Command::McpInstall {
            directory,
            claude,
            codex,
        } => mcp_install::run(&directory, claude, codex),

        Command::Refresh {
            name,
            all,
            host,
            port,
            remote_path,
            identity,
        } => {
            let overrides = refresh::ConnectionOverrides {
                host,
                port,
                remote_path,
                identity,
            };
            refresh::run(name, all, overrides).await
        }

        Command::Update { check } => update::run(check),
    }
}
