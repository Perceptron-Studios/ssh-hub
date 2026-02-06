use std::sync::Arc;

use crate::connection::SshConnection;
use super::schema::{RemoteBashInput, RemoteBashOutput};

pub async fn handle(conn: Arc<SshConnection>, input: RemoteBashInput) -> String {
    let timeout = input.timeout.unwrap_or(120000).min(600000);

    match conn.exec(&input.command, Some(timeout)).await {
        Ok(result) => {
            let output = RemoteBashOutput {
                stdout: result.stdout,
                stderr: result.stderr,
                exit_code: result.exit_code,
            };
            serde_json::to_string_pretty(&output).unwrap_or_default()
        }
        Err(e) => format!("Error: {}", e),
    }
}
