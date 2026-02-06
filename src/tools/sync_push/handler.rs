use std::sync::Arc;

use crate::connection::SshConnection;
use crate::utils::path::normalize_remote_path;
use super::schema::{FailedTransfer, SyncPushInput, SyncPushOutput};

pub async fn handle(conn: Arc<SshConnection>, input: SyncPushInput) -> String {
    let base_path = conn.remote_path().to_string();

    let local_content = match tokio::fs::read_to_string(&input.local_path).await {
        Ok(c) => c,
        Err(e) => {
            let output = SyncPushOutput {
                pushed: vec![],
                failed: vec![FailedTransfer {
                    path: input.local_path.clone(),
                    error: format!("Error reading local file: {}", e),
                }],
            };
            return serde_json::to_string_pretty(&output).unwrap_or_default();
        }
    };

    let remote_path = input
        .remote_path
        .unwrap_or_else(|| normalize_remote_path(&input.local_path, &base_path));

    match conn.write_file(&remote_path, &local_content).await {
        Ok(()) => {
            let output = SyncPushOutput {
                pushed: vec![input.local_path],
                failed: vec![],
            };
            serde_json::to_string_pretty(&output).unwrap_or_default()
        }
        Err(e) => {
            let output = SyncPushOutput {
                pushed: vec![],
                failed: vec![FailedTransfer {
                    path: input.local_path,
                    error: e.to_string(),
                }],
            };
            serde_json::to_string_pretty(&output).unwrap_or_default()
        }
    }
}
