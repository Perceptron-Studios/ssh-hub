use rmcp::schemars::{self, JsonSchema};
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ConnectInput {
    #[schemars(description = "Alias for this connection (e.g., 'staging'). If connecting a pre-configured server, use its name.")]
    pub name: String,

    #[schemars(description = "SSH connection string (user@host:/path or user@host:port:/path). Omit if connecting a pre-configured server by name.")]
    pub connection: Option<String>,

    #[schemars(description = "SSH port (default: 22)")]
    pub port: Option<u16>,

    #[schemars(description = "Path to SSH private key file")]
    pub identity: Option<String>,

    #[schemars(description = "If true, save this server to the config file for future use (default: false)")]
    pub save: Option<bool>,
}
