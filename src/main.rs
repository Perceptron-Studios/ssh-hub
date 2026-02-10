use anyhow::Result;
use clap::Parser;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use ssh_hub::cli::Cli;
use ssh_hub::server::RemoteSessionServer;
use ssh_hub::server_registry::ServerRegistry;

fn init_logging(verbose: bool) {
    let filter = if verbose {
        LevelFilter::DEBUG
    } else {
        LevelFilter::INFO
    };

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    init_logging(cli.verbose);

    if let Some(command) = cli.command {
        ssh_hub::cli::run(command).await
    } else {
        tracing::info!("Starting ssh-hub MCP server");

        let config = ServerRegistry::load().unwrap_or_else(|e| {
            tracing::warn!("Failed to load config, starting with empty config: {e}");
            ServerRegistry::default()
        });

        tracing::debug!("Loaded {} configured servers", config.servers.len());

        let server = RemoteSessionServer::new(config);
        server.run().await
    }
}
