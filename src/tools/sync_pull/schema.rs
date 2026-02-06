use rmcp::schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};

use crate::tools::sync_push::FailedTransfer;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SyncPullInput {
    #[schemars(description = "Name of the connected server to target (e.g., 'staging')")]
    pub server: String,

    #[schemars(description = "Remote file or directory to pull")]
    pub remote_path: String,

    #[schemars(description = "Local destination path")]
    pub local_path: Option<String>,

    #[schemars(description = "Specific files to pull (if remote_path is a directory)")]
    pub files: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct SyncPullOutput {
    pub pulled: Vec<String>,
    pub failed: Vec<FailedTransfer>,
}
