use rmcp::schemars::{self, JsonSchema};
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RemoteReadInput {
    #[schemars(description = "Name of the connected server to target (e.g., 'staging')")]
    pub server: String,

    #[schemars(description = "The absolute path to the file to read")]
    pub file_path: String,

    #[schemars(
        description = "The line number to start reading from. Only provide if the file is too large to read at once"
    )]
    pub offset: Option<u64>,

    #[schemars(
        description = "The number of lines to read. Only provide if the file is too large to read at once"
    )]
    pub limit: Option<u64>,
}
