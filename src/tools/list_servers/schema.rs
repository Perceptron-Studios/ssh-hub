use rmcp::schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};

use crate::metadata::SystemMetadata;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListServersInput {
    #[schemars(
        description = "If true, also show configured servers that are not currently connected (default: true)"
    )]
    pub include_configured: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct ServerInfo {
    pub name: String,
    pub host: String,
    pub user: String,
    pub port: u16,
    pub remote_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<SystemMetadata>,
    pub connectivity: ConnectivityInfo,
}

#[derive(Debug, Serialize)]
pub struct ConnectivityInfo {
    pub status: ServerStatus,
    pub reachable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ServerStatus {
    Connected,
    Configured,
}
