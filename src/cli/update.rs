use anyhow::{Context, Result};
use colored::Colorize;

const REPO_URL: &str = "https://github.com/Perceptron-Studios/ssh-hub.git";
const REPO_API: &str = "https://api.github.com/repos/Perceptron-Studios/ssh-hub/tags?per_page=1";

pub fn run(check_only: bool) -> Result<()> {
    let current = env!("CARGO_PKG_VERSION");

    println!(
        "{} Current version: {}",
        ">".blue().bold(),
        format!("v{current}").bold(),
    );

    // Fetch latest tag from GitHub API via curl
    let output = std::process::Command::new("curl")
        .args(["-sL", REPO_API])
        .output()
        .context("Failed to run curl — is it installed?")?;

    if !output.status.success() {
        return Err(anyhow::anyhow!("Failed to fetch tags from GitHub"));
    }

    let body = String::from_utf8_lossy(&output.stdout);
    let tags: serde_json::Value =
        serde_json::from_str(&body).context("Failed to parse GitHub API response")?;

    let latest_tag = tags
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|tag| tag["name"].as_str())
        .ok_or_else(|| anyhow::anyhow!("No tags found in repository"))?;

    let latest_version = latest_tag.trim_start_matches('v');

    if latest_version == current {
        println!("  {} Already on latest version", "ok".green());
        return Ok(());
    }

    println!(
        "  {} New version available: {}",
        "!".yellow().bold(),
        format!("v{latest_version}").bold(),
    );

    if check_only {
        println!("  Run {} to install", "ssh-hub update".bold());
        return Ok(());
    }

    // Check for cargo
    if std::process::Command::new("cargo")
        .arg("--version")
        .output()
        .is_err()
    {
        return Err(anyhow::anyhow!(
            "Rust toolchain not found. Install it via https://rustup.rs/ then retry."
        ));
    }

    println!("{} Installing {}...", ">".blue().bold(), latest_tag.bold());

    let status = std::process::Command::new("cargo")
        .args(["install", "--git", REPO_URL, "--tag", latest_tag])
        .status()
        .context("Failed to run cargo install")?;

    if status.success() {
        println!(
            "  {} Updated to {}",
            "ok".green(),
            format!("v{latest_version}").bold()
        );
    } else {
        return Err(anyhow::anyhow!(
            "cargo install failed with exit code {}",
            status.code().unwrap_or(-1)
        ));
    }

    Ok(())
}
