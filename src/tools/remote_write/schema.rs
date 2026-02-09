use rmcp::schemars::{self, JsonSchema};
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RemoteWriteInput {
    #[schemars(description = "Name of the connected server to target (e.g., 'staging')")]
    pub server: String,

    #[schemars(
        description = "The absolute path to the file to write (must be absolute, not relative)"
    )]
    pub file_path: String,

    #[schemars(description = "The content to write to the file")]
    pub content: String,
}
