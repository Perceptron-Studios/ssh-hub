use rmcp::schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SyncPushInput {
    #[schemars(description = "Name of the connected server to target (e.g., 'staging')")]
    pub server: String,

    #[schemars(description = "Local file or directory to push")]
    pub local_path: String,

    #[schemars(description = "Remote destination path (default: mirrors local structure)")]
    pub remote_path: Option<String>,

    #[schemars(description = "Specific files to push (if local_path is a directory)")]
    pub files: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct FailedTransfer {
    pub path: String,
    pub error: String,
}

#[derive(Debug, Serialize)]
pub struct SyncPushOutput {
    pub pushed: Vec<String>,
    pub failed: Vec<FailedTransfer>,
}
