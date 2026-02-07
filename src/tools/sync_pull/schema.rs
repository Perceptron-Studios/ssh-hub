use rmcp::schemars::{self, JsonSchema};
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SyncPullInput {
    #[schemars(description = "Name of the connected server to target (e.g., 'staging')")]
    pub server: String,

    #[schemars(description = "Absolute or relative path to a remote file or directory. Relative paths resolve from the connection's base path")]
    pub remote_path: String,

    #[schemars(description = "Local destination path. For files: defaults to the filename in the current directory. For directories: defaults to the current directory")]
    pub local_path: Option<String>,

    #[schemars(description = "Specific files to pull, as relative paths within remote_path. Only used when remote_path is a directory. If omitted, pulls all files")]
    pub files: Option<Vec<String>>,
}
