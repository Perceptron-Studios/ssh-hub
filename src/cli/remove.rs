use anyhow::Result;
use colored::Colorize;

use crate::server_registry::ServerRegistry;

pub fn run(name: &str) -> Result<()> {
    let mut config = ServerRegistry::load().unwrap_or_default();

    if config.remove(name).is_some() {
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
