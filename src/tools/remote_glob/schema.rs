use rmcp::schemars::{self, JsonSchema};
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RemoteGlobInput {
    #[schemars(description = "Name of the connected server to target (e.g., 'staging')")]
    pub server: String,

    #[schemars(description = "The glob pattern to match files against")]
    pub pattern: String,

    #[schemars(description = "The directory to search in. If not specified, uses the connection's base path")]
    pub path: Option<String>,
}
