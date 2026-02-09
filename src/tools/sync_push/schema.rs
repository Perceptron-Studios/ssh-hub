use rmcp::schemars::{self, JsonSchema};
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SyncPushInput {
    #[schemars(description = "Name of the connected server to target (e.g., 'staging')")]
    pub server: String,

    #[schemars(description = "Absolute path to a local file or directory to push")]
    pub local_path: String,

    #[schemars(
        description = "Remote destination path. If omitted, mirrors the local path relative to the connection's base path"
    )]
    pub remote_path: Option<String>,

    #[schemars(
        description = "Extra exclusion patterns (gitignore syntax). Applied on top of .gitignore rules. Example: [\"*.log\", \"tmp/\", \"dist\"]"
    )]
    pub exclude: Option<Vec<String>>,
}
