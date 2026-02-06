use anyhow::Result;
use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use ssh_hub::cli::{self, Cli, Command};
use ssh_hub::server_registry::{self as server_registry, ServerRegistry};
use ssh_hub::connection;
use ssh_hub::server::RemoteSessionServer;

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

    match cli.command {
        // Default: start MCP server
        None => {
            init_logging(cli.verbose);
            tracing::info!("Starting ssh-hub MCP server");

            let config = ServerRegistry::load().unwrap_or_else(|e| {
                tracing::warn!("Failed to load config, starting with empty config: {}", e);
                ServerRegistry::default()
            });

            tracing::debug!("Loaded {} configured servers", config.servers.len());

            let server = RemoteSessionServer::new(config);
            server.run().await?;
        }

        // Interactive setup
        Some(Command::Setup {
            name,
            connection,
            port,
            identity,
        }) => {
            run_setup(name, connection, port, identity).await?;
        }

        // Add server to config
        Some(Command::Add {
            name,
            connection,
            identity,
        }) => {
            run_add(name, connection, identity)?;
        }

        // Remove server from config
        Some(Command::Remove { name }) => {
            run_remove(name)?;
        }

        // List configured servers
        Some(Command::List) => {
            run_list()?;
        }
    }

    Ok(())
}

async fn run_setup(
    name: String,
    connection: Option<String>,
    port: Option<u16>,
    identity: Option<std::path::PathBuf>,
) -> Result<()> {
    let mut config = ServerRegistry::load().unwrap_or_default();

    // If server already exists, show current config
    if let Some(existing) = config.get(&name) {
        println!("Server '{}' already configured:", name);
        println!("  host: {}", existing.host);
        println!("  user: {}", existing.user);
        println!("  port: {}", existing.port);
        println!("  path: {}", existing.remote_path);
        println!("  auth: {:?}", existing.auth);
        println!();
    }

    // Parse connection string if provided
    let conn_info = if let Some(ref conn_str) = connection {
        Some(cli::parse_connection_string(conn_str, port)?)
    } else if let Some(existing) = config.get(&name) {
        Some(cli::ConnectionInfo {
            user: existing.user.clone(),
            host: existing.host.clone(),
            port: port.unwrap_or(existing.port),
            remote_path: existing.remote_path.clone(),
        })
    } else {
        println!("No connection string provided and server '{}' not in config.", name);
        println!("Usage: ssh-hub setup {} --connection user@host:/path", name);
        return Ok(());
    };

    let conn_info = conn_info.unwrap();

    println!("Configuring server '{}':", name);
    println!("  {}@{}:{} -> {}", conn_info.user, conn_info.host, conn_info.port, conn_info.remote_path);

    // Ask for auth method
    println!();
    println!("Authentication method:");
    println!("  1) SSH key (default)");
    println!("  2) SSH agent");
    print!("Choose [1]: ");
    use std::io::Write;
    std::io::stdout().flush()?;

    let mut choice = String::new();
    std::io::stdin().read_line(&mut choice)?;
    let choice = choice.trim();

    let auth_method = match choice {
        "2" => server_registry::AuthMethod::Agent,
        _ => {
            if let Some(ref id) = identity {
                println!("Using identity file: {:?}", id);
            }
            server_registry::AuthMethod::Key
        }
    };

    // Save config
    let entry = server_registry::ServerEntry {
        host: conn_info.host,
        user: conn_info.user,
        port: conn_info.port,
        remote_path: conn_info.remote_path,
        identity: identity.map(|p| p.to_string_lossy().to_string()),
        auth: auth_method,
    };

    config.insert(name.clone(), entry);
    config.save()?;
    println!("Config saved to {:?}", ServerRegistry::config_path()?);

    // Test connection
    println!();
    print!("Test connection? [Y/n]: ");
    std::io::stdout().flush()?;

    let mut test_choice = String::new();
    std::io::stdin().read_line(&mut test_choice)?;

    if !test_choice.trim().eq_ignore_ascii_case("n") {
        println!("Testing connection...");
        let entry = config.get(&name).unwrap();
        let params = cli::params_from_config(&name, entry);

        match connection::SshConnection::connect(params).await {
            Ok(conn) => {
                println!("Connection successful!");
                // Run a quick test
                match conn.exec("echo 'ssh-hub test OK'", Some(10000)).await {
                    Ok(result) => println!("Remote exec test: {}", result.stdout.trim()),
                    Err(e) => println!("Warning: connected but exec failed: {}", e),
                }
            }
            Err(e) => {
                println!("Connection failed: {}", e);
                println!("Check your credentials and try again.");
            }
        }
    }

    println!();
    println!("Setup complete. Add to Claude Code with:");
    println!("  claude mcp add remote -- ssh-hub");

    Ok(())
}

fn run_add(
    name: String,
    connection: String,
    identity: Option<std::path::PathBuf>,
) -> Result<()> {
    let mut config = ServerRegistry::load().unwrap_or_default();

    if config.get(&name).is_some() {
        println!("Server '{}' already exists. Use 'setup' to reconfigure or 'remove' first.", name);
        return Ok(());
    }

    let conn_info = cli::parse_connection_string(&connection, None)?;

    let entry = server_registry::ServerEntry {
        host: conn_info.host,
        user: conn_info.user,
        port: conn_info.port,
        remote_path: conn_info.remote_path,
        identity: identity.map(|p| p.to_string_lossy().to_string()),
        auth: server_registry::AuthMethod::Auto,
    };

    config.insert(name.clone(), entry);
    config.save()?;
    println!("Server '{}' added to config.", name);
    println!("Config saved to {:?}", ServerRegistry::config_path()?);

    Ok(())
}

fn run_remove(name: String) -> Result<()> {
    let mut config = ServerRegistry::load().unwrap_or_default();

    if config.remove(&name).is_some() {
        config.save()?;
        println!("Server '{}' removed from config.", name);
    } else {
        println!("Server '{}' not found in config.", name);
    }

    Ok(())
}

fn run_list() -> Result<()> {
    let config = ServerRegistry::load().unwrap_or_default();

    if config.servers.is_empty() {
        println!("No servers configured.");
        println!("Use 'ssh-hub add <name> user@host:/path' to add one.");
        return Ok(());
    }

    for (name, entry) in &config.servers {
        println!(
            "  {} -> {}@{}:{} (path: {}, auth: {:?})",
            name, entry.user, entry.host, entry.port, entry.remote_path, entry.auth
        );
    }

    Ok(())
}
