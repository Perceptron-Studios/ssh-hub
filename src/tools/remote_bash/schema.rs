use rmcp::schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RemoteBashInput {
    #[schemars(description = "Name of the connected server to target (e.g., 'staging')")]
    pub server: String,

    #[schemars(description = "The command to execute")]
    pub command: String,

    #[schemars(
        description = "Timeout in milliseconds. Defaults to 120000 (2 min), max 600000 (10 min). Ignored when run_in_background is true."
    )]
    pub timeout: Option<u64>,

    #[schemars(description = "Clear, concise description of what this command does")]
    pub description: Option<String>,

    #[schemars(
        description = "Set to true to run this command in the background. Returns a PID and log file path immediately. The 'timeout' parameter is ignored for background commands."
    )]
    pub run_in_background: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct RemoteBashOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// Output returned when a command is launched in background mode.
///
/// The `pid` can be used with `kill <pid>` or `ps -p <pid>` to manage the
/// process. Command output (stdout + stderr) is written to `log_file`.
#[derive(Debug, Serialize)]
pub struct RemoteBashBackgroundOutput {
    /// Process ID of the backgrounded command.
    pub pid: String,
    /// Path to the log file capturing stdout and stderr on the remote server.
    pub log_file: String,
    /// Human-readable status message.
    pub message: String,
}
