use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::Parser;
use colored::Colorize;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use ssh_hub::cli::{self, Cli, Command};
use ssh_hub::connection;
use ssh_hub::server::RemoteSessionServer;
use ssh_hub::server_registry::{self, ServerRegistry};

fn init_logging(verbose: bool) {
    let filter = if verbose {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("info")
    };

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    init_logging(cli.verbose);

    match cli.command {
        // Default: start MCP server
        None => {
            tracing::info!("Starting ssh-hub MCP server");

            let config = ServerRegistry::load().unwrap_or_else(|e| {
                tracing::warn!("Failed to load config, starting with empty config: {}", e);
                ServerRegistry::default()
            });

            tracing::debug!("Loaded {} configured servers", config.servers.len());

            let server = RemoteSessionServer::new(config);
            server.run().await?;
        }

        // Add (or reconfigure) a server
        Some(Command::Add {
            name,
            connection,
            port,
            identity,
        }) => {
            run_add(name, connection, port, identity).await?;
        }

        // Remove server from config
        Some(Command::Remove { name }) => {
            run_remove(name)?;
        }

        // List configured servers
        Some(Command::List) => {
            run_list()?;
        }

        // Register MCP in a project directory
        Some(Command::McpInstall {
            directory,
            claude,
            codex,
        }) => {
            run_mcp_install(directory, claude, codex)?;
        }

        // Self-update
        Some(Command::Update { check }) => {
            run_update(check)?;
        }
    }

    Ok(())
}

async fn run_add(
    name: String,
    connection: String,
    port: Option<u16>,
    identity: Option<PathBuf>,
) -> Result<()> {
    let mut config = ServerRegistry::load().unwrap_or_default();

    // If server already exists, show current config
    if let Some(existing) = config.get(&name) {
        println!(
            "{} Server {} already configured:",
            "!".yellow().bold(),
            name.bold(),
        );
        println!("  {} {}", "host:".dimmed(), existing.host);
        println!("  {} {}", "user:".dimmed(), existing.user);
        println!("  {} {}", "port:".dimmed(), existing.port);
        println!("  {} {}", "path:".dimmed(), existing.remote_path);
        println!("  {} {:?}", "auth:".dimmed(), existing.auth);
        println!();

        print!("  Overwrite? {}: ", "[y/N]".dimmed());
        std::io::stdout().flush()?;
        let mut answer = String::new();
        std::io::stdin().read_line(&mut answer)?;
        if !answer.trim().eq_ignore_ascii_case("y") {
            println!("  {}", "Aborted.".dimmed());
            return Ok(());
        }
        println!();
    }

    let conn_info = cli::parse_connection_string(&connection, port)?;

    println!(
        "{} Adding server {}",
        "+".green().bold(),
        name.bold(),
    );
    println!(
        "  {} {}@{}:{}",
        "connect:".dimmed(),
        conn_info.user.cyan(),
        conn_info.host.cyan(),
        conn_info.port.to_string().cyan(),
    );
    println!(
        "  {}    {}",
        "path:".dimmed(),
        conn_info.remote_path.cyan(),
    );

    let auth_method = server_registry::AuthMethod::Auto;

    // If identity file provided, add to ssh-agent so the MCP server can use it at runtime
    if let Some(ref id) = identity {
        println!();
        println!(
            "{} Adding key to ssh-agent: {}",
            ">".blue().bold(),
            id.display().to_string().underline(),
        );

        let result = std::process::Command::new("ssh-add").arg(id).status();
        match result {
            Ok(s) if s.success() => {
                println!("  {} Key added to agent", "ok".green());
            }
            Ok(s) => {
                println!(
                    "  {} ssh-add exited with code {}",
                    "warn".yellow(),
                    s.code().unwrap_or(-1),
                );
            }
            Err(ref e) => {
                println!("  {} failed to run ssh-add: {}", "warn".yellow(), e);
            }
        }

        // Surface the manual hint for any failure
        if !matches!(result, Ok(s) if s.success()) {
            println!(
                "  You may need to run {} manually.",
                format!("ssh-add {}", id.display()).dimmed(),
            );
        }
    }

    // Build the entry (but don't save yet — test connection first)
    let entry = server_registry::ServerEntry {
        host: conn_info.host,
        user: conn_info.user,
        port: conn_info.port,
        remote_path: conn_info.remote_path,
        identity: identity.map(|p| p.to_string_lossy().to_string()),
        auth: auth_method,
    };

    // Test connection before saving
    let params = cli::params_from_config(&name, &entry);

    match connection::SshConnection::connect(params).await {
        Ok(conn) => {
            let _ = conn.exec("echo 'ssh-hub test OK'", Some(10000)).await;

            config.insert(name.clone(), entry);
            config.save()?;
            println!("  {} Server {} is up and running", "ok".green(), name.bold());
        }
        Err(_) => {
            println!("  {} Server {} failed authentication", "failed".red(), name.bold());
            println!();

            print!("  Save server config anyway? {}: ", "[y/N]".dimmed());
            std::io::stdout().flush()?;
            let mut save_choice = String::new();
            std::io::stdin().read_line(&mut save_choice)?;

            if save_choice.trim().eq_ignore_ascii_case("y") {
                config.insert(name.clone(), entry);
                config.save()?;
                println!(
                    "  {} Saved to {}",
                    "ok".green(),
                    ServerRegistry::config_path()?.display().to_string().dimmed(),
                );
            } else {
                println!(
                    "  {} Server not saved. Fix credentials and try again.",
                    "x".red(),
                );
            }
        }
    }

    Ok(())
}

fn run_remove(name: String) -> Result<()> {
    let mut config = ServerRegistry::load().unwrap_or_default();

    if config.remove(&name).is_some() {
        config.save()?;
        println!("{} Server {} removed.", "-".red().bold(), name.bold());
    } else {
        println!(
            "{} Server {} not found in config.",
            "!".yellow().bold(),
            name.bold(),
        );
    }

    Ok(())
}

fn run_list() -> Result<()> {
    let config = ServerRegistry::load().unwrap_or_default();

    if config.servers.is_empty() {
        println!("{}", "No servers configured.".dimmed());
        println!(
            "Run {} to add one.",
            "ssh-hub add <name> user@host:/path".bold(),
        );
        return Ok(());
    }

    for (name, entry) in &config.servers {
        println!(
            "  {} {} {}@{}:{} {}",
            name.bold(),
            "->".dimmed(),
            entry.user.cyan(),
            entry.host.cyan(),
            entry.port.to_string().cyan(),
            format!("(path: {}, auth: {:?})", entry.remote_path, entry.auth).dimmed(),
        );
    }

    Ok(())
}

fn run_mcp_install(directory: PathBuf, claude: bool, codex: bool) -> Result<()> {
    // When neither flag is provided, configure both
    let (do_claude, do_codex) = if !claude && !codex {
        (true, true)
    } else {
        (claude, codex)
    };

    let target = std::fs::canonicalize(&directory)
        .map_err(|e| anyhow::anyhow!("Cannot resolve directory '{}': {}", directory.display(), e))?;

    if !target.is_dir() {
        return Err(anyhow::anyhow!("'{}' is not a directory", target.display()));
    }

    if do_claude {
        install_claude_config(&target)?;
    }
    if do_codex {
        install_codex_config(&target)?;
    }

    Ok(())
}

fn install_claude_config(target: &Path) -> Result<()> {
    let path = target.join(".mcp.json");

    let mut root: serde_json::Value = if path.exists() {
        let content = std::fs::read_to_string(&path)?;
        serde_json::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse {}: {}", path.display(), e))?
    } else {
        serde_json::json!({})
    };

    let servers = root
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!(".mcp.json root is not a JSON object"))?
        .entry("mcpServers")
        .or_insert_with(|| serde_json::json!({}));

    let servers_map = servers
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("\"mcpServers\" is not a JSON object"))?;

    servers_map.insert(
        "ssh-hub".to_string(),
        serde_json::json!({
            "command": "ssh-hub",
            "args": []
        }),
    );

    let output = serde_json::to_string_pretty(&root)? + "\n";
    std::fs::write(&path, output)?;

    println!(
        "  {} Claude Code: {}",
        "ok".green(),
        path.display().to_string().dimmed(),
    );
    Ok(())
}

fn install_codex_config(target: &Path) -> Result<()> {
    let codex_dir = target.join(".codex");
    let path = codex_dir.join("config.toml");

    let mut doc: toml::Table = if path.exists() {
        let content = std::fs::read_to_string(&path)?;
        content.parse::<toml::Table>()
            .map_err(|e| anyhow::anyhow!("Failed to parse {}: {}", path.display(), e))?
    } else {
        toml::Table::new()
    };

    let mcp_servers = doc
        .entry("mcp_servers")
        .or_insert_with(|| toml::Value::Table(toml::Table::new()));

    let servers_table = mcp_servers
        .as_table_mut()
        .ok_or_else(|| anyhow::anyhow!("\"mcp_servers\" is not a TOML table"))?;

    let mut entry = toml::Table::new();
    entry.insert("command".to_string(), toml::Value::String("ssh-hub".to_string()));
    entry.insert("args".to_string(), toml::Value::Array(vec![]));

    servers_table.insert("ssh-hub".to_string(), toml::Value::Table(entry));

    std::fs::create_dir_all(&codex_dir)?;
    let output = toml::to_string_pretty(&doc)?;
    std::fs::write(&path, output)?;

    println!(
        "  {} Codex: {}",
        "ok".green(),
        path.display().to_string().dimmed(),
    );
    Ok(())
}

fn run_update(check_only: bool) -> Result<()> {
    let current = env!("CARGO_PKG_VERSION");

    println!(
        "{} Current version: {}",
        ">".blue().bold(),
        format!("v{}", current).bold(),
    );

    let updater = self_update::backends::github::Update::configure()
        .repo_owner("Perceptron-Studios")
        .repo_name("ssh-hub")
        .bin_name("ssh-hub")
        .current_version(current)
        .show_download_progress(true)
        .no_confirm(true)
        .build()?;

    let latest = match updater.get_latest_release() {
        Ok(release) => release,
        Err(_) => {
            println!(
                "  {} No releases found. You're on the latest build.",
                "ok".green(),
            );
            return Ok(());
        }
    };
    let latest_version = latest.version.trim_start_matches('v');

    if latest_version == current {
        println!(
            "  {} Already on latest version",
            "ok".green(),
        );
        return Ok(());
    }

    println!(
        "  {} New version available: {}",
        "!".yellow().bold(),
        format!("v{}", latest_version).bold(),
    );

    if check_only {
        println!(
            "  Run {} to install",
            "ssh-hub update".bold(),
        );
        return Ok(());
    }

    let status = updater.update()?;
    println!(
        "  {} Updated to {}",
        "ok".green(),
        format!("v{}", status.version()).bold(),
    );

    Ok(())
}
