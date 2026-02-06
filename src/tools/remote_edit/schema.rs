use rmcp::schemars::{self, JsonSchema};
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RemoteEditInput {
    #[schemars(description = "Name of the connected server to target (e.g., 'staging')")]
    pub server: String,

    #[schemars(description = "The absolute path to the file to modify")]
    pub file_path: String,

    #[schemars(description = "The text to replace")]
    pub old_string: String,

    #[schemars(description = "The text to replace it with (must be different from old_string)")]
    pub new_string: String,

    #[schemars(description = "Replace all occurrences of old_string (default false)")]
    pub replace_all: Option<bool>,
}
