use rmcp::schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RemoteBashInput {
    #[schemars(description = "Name of the connected server to target (e.g., 'staging')")]
    pub server: String,

    #[schemars(description = "The command to execute")]
    pub command: String,

    #[schemars(description = "Timeout in milliseconds. Defaults to 120000 (2 min), max 600000 (10 min)")]
    pub timeout: Option<u64>,

    #[schemars(description = "Clear, concise description of what this command does")]
    pub description: Option<String>,

    #[schemars(description = "Set to true to run this command in the background")]
    pub run_in_background: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct RemoteBashOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}
