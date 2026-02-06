use rmcp::schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListServersInput {
    #[schemars(description = "If true, also show configured servers that are not currently connected (default: true)")]
    pub include_configured: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct ConnectedServerInfo {
    pub name: String,
    pub host: String,
    pub user: String,
    pub port: u16,
    pub remote_path: String,
}

#[derive(Debug, Serialize)]
pub struct ConfiguredServerInfo {
    pub name: String,
    pub host: String,
    pub user: String,
    pub port: u16,
    pub remote_path: String,
    pub auth: String,
    pub connected: bool,
}

#[derive(Debug, Serialize)]
pub struct ListServersOutput {
    pub connected: Vec<ConnectedServerInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configured: Option<Vec<ConfiguredServerInfo>>,
}
