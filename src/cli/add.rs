use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::Result;
use colored::Colorize;

use crate::connection;
use crate::metadata;
use crate::server_registry::{self, ServerRegistry};

use super::params_from_config;
use super::parse_connection_string;

/// Timeout for the connectivity test after adding a server (10 seconds).
const CONNECTION_TEST_TIMEOUT_MS: u64 = 10_000;

pub async fn run(
    name: String,
    connection: String,
    port: Option<u16>,
    identity: Option<PathBuf>,
) -> Result<()> {
    let mut config = ServerRegistry::load()?;

    if let Some(existing) = config.get(&name) {
        if !prompt_overwrite(&name, existing)? {
            return Ok(());
        }
    }

    let conn_info = parse_connection_string(&connection, port)?;

    println!("{} Adding server {}", "+".green().bold(), name.bold(),);
    println!(
        "  {} {}@{}:{}",
        "connect:".dimmed(),
        conn_info.user.cyan(),
        conn_info.host.cyan(),
        conn_info.port.to_string().cyan(),
    );
    println!("  {}    {}", "path:".dimmed(), conn_info.remote_path.cyan(),);

    if let Some(ref id) = identity {
        add_key_to_agent(id);
    }

    let entry = server_registry::ServerEntry {
        host: conn_info.host,
        user: conn_info.user,
        port: conn_info.port,
        remote_path: conn_info.remote_path,
        identity: identity.map(|p| p.to_string_lossy().to_string()),
        auth: server_registry::AuthMethod::Auto,
        metadata: None,
    };

    test_and_save(&name, entry, &mut config).await
}

/// Show current config and ask user whether to overwrite.
/// Returns `true` if user confirms, `false` if aborted.
fn prompt_overwrite(name: &str, existing: &server_registry::ServerEntry) -> Result<bool> {
    println!(
        "{} Server {} already configured:",
        "!".yellow().bold(),
        name.bold(),
    );
    println!("  {} {}", "host:".dimmed(), existing.host);
    println!("  {} {}", "user:".dimmed(), existing.user);
    println!("  {} {}", "port:".dimmed(), existing.port);
    println!("  {} {}", "path:".dimmed(), existing.remote_path);
    println!("  {} {}", "auth:".dimmed(), existing.auth);
    println!();

    print!("  Overwrite? {}: ", "[y/N]".dimmed());
    std::io::stdout().flush()?;
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    if !answer.trim().eq_ignore_ascii_case("y") {
        println!("  {}", "Aborted.".dimmed());
        return Ok(false);
    }
    println!();
    Ok(true)
}

/// Add an identity key to ssh-agent, printing status.
fn add_key_to_agent(id: &Path) {
    println!();
    println!(
        "{} Adding key to ssh-agent: {}",
        ">".blue().bold(),
        id.display().to_string().underline(),
    );

    let result = std::process::Command::new("ssh-add").arg(id).status();
    let key_added = matches!(&result, Ok(s) if s.success());

    match result {
        Ok(_) if key_added => {
            println!("  {} Key added to agent", "ok".green());
        }
        Ok(s) => {
            let code = s
                .code()
                .map_or_else(|| "signal".to_string(), |c| c.to_string());
            println!("  {} ssh-add exited with code {code}", "warn".yellow());
        }
        Err(ref e) => {
            println!("  {} failed to run ssh-add: {e}", "warn".yellow());
        }
    }

    if !key_added {
        println!(
            "  You may need to run {} manually.",
            format!("ssh-add {}", id.display()).dimmed(),
        );
    }
}

/// Test the SSH connection and save the entry to config.
async fn test_and_save(
    name: &str,
    mut entry: server_registry::ServerEntry,
    config: &mut ServerRegistry,
) -> Result<()> {
    let params = params_from_config(name, &entry);

    let Ok(conn) = connection::SshConnection::connect(params).await else {
        return prompt_save_on_failure(name, entry, config);
    };

    if let Err(e) = conn
        .exec("echo 'ssh-hub test OK'", Some(CONNECTION_TEST_TIMEOUT_MS))
        .await
    {
        tracing::debug!("Connection test command failed: {e}");
    }

    // Collect system metadata while we have an open connection
    match metadata::collect(&conn).await {
        Ok(meta) => {
            if let Some(summary) = meta.summary_line() {
                println!("  {} {}", "system:".dimmed(), summary);
            }
            entry.metadata = Some(meta);
        }
        Err(e) => {
            tracing::debug!("Metadata collection failed during add: {e}");
        }
    }

    config.insert(name.to_string(), entry);
    config.save()?;
    println!(
        "  {} Server {} is up and running",
        "ok".green(),
        name.bold()
    );
    Ok(())
}

/// Prompt the user to save config even though the connection failed.
fn prompt_save_on_failure(
    name: &str,
    entry: server_registry::ServerEntry,
    config: &mut ServerRegistry,
) -> Result<()> {
    println!(
        "  {} Server {} failed authentication",
        "failed".red(),
        name.bold()
    );
    println!();

    print!("  Save server config anyway? {}: ", "[y/N]".dimmed());
    std::io::stdout().flush()?;
    let mut save_choice = String::new();
    std::io::stdin().read_line(&mut save_choice)?;

    if save_choice.trim().eq_ignore_ascii_case("y") {
        config.insert(name.to_string(), entry);
        config.save()?;
        println!(
            "  {} Saved to {}",
            "ok".green(),
            ServerRegistry::config_path()?
                .display()
                .to_string()
                .dimmed(),
        );
    } else {
        println!(
            "  {} Server not saved. Fix credentials and try again.",
            "x".red(),
        );
    }

    Ok(())
}
