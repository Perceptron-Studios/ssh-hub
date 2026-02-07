use rmcp::schemars::{self, JsonSchema};
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SyncPushInput {
    #[schemars(description = "Name of the connected server to target (e.g., 'staging')")]
    pub server: String,

    #[schemars(description = "Absolute path to a local file or directory to push")]
    pub local_path: String,

    #[schemars(description = "Remote destination path. If omitted, mirrors the local path relative to the connection's base path")]
    pub remote_path: Option<String>,

    #[schemars(description = "Specific files to push, as relative paths within local_path. Only used when local_path is a directory. If omitted, pushes all files")]
    pub files: Option<Vec<String>>,
}
