use std::sync::Arc;

use crate::connection::SshConnection;
use super::schema::{RemoteBashInput, RemoteBashOutput};

/// Default timeout for bash commands (2 minutes).
const DEFAULT_TIMEOUT_MS: u64 = 120_000;

/// Maximum allowed timeout for bash commands (10 minutes).
const MAX_TIMEOUT_MS: u64 = 600_000;

pub async fn handle(conn: Arc<SshConnection>, input: RemoteBashInput) -> String {
    let timeout = input.timeout.unwrap_or(DEFAULT_TIMEOUT_MS).min(MAX_TIMEOUT_MS);

    match conn.exec(&input.command, Some(timeout)).await {
        Ok(result) => {
            let output = RemoteBashOutput {
                stdout: result.stdout,
                stderr: result.stderr,
                exit_code: result.exit_code,
            };
            serde_json::to_string_pretty(&output)
                .unwrap_or_else(|e| format!(r#"{{"error": "serialization failed: {}"}}"#, e))
        }
        Err(e) => format!("Error: {}", e),
    }
}
