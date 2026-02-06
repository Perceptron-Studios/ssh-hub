use std::sync::Arc;

use crate::connection::SshConnection;
use crate::tools::sync_push::FailedTransfer;
use crate::utils::path::normalize_remote_path;
use super::schema::{SyncPullInput, SyncPullOutput};

pub async fn handle(conn: Arc<SshConnection>, input: SyncPullInput) -> String {
    let base_path = conn.remote_path().to_string();
    let remote_path = normalize_remote_path(&input.remote_path, &base_path);

    let content = match conn.read_file(&remote_path).await {
        Ok(c) => c,
        Err(e) => {
            let output = SyncPullOutput {
                pulled: vec![],
                failed: vec![FailedTransfer {
                    path: input.remote_path.clone(),
                    error: format!("Error reading remote file: {}", e),
                }],
            };
            return serde_json::to_string_pretty(&output).unwrap_or_default();
        }
    };

    let local_path = input.local_path.unwrap_or_else(|| {
        std::path::Path::new(&input.remote_path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "downloaded_file".to_string())
    });

    match tokio::fs::write(&local_path, &content).await {
        Ok(()) => {
            let output = SyncPullOutput {
                pulled: vec![local_path],
                failed: vec![],
            };
            serde_json::to_string_pretty(&output).unwrap_or_default()
        }
        Err(e) => {
            let output = SyncPullOutput {
                pulled: vec![],
                failed: vec![FailedTransfer {
                    path: local_path,
                    error: e.to_string(),
                }],
            };
            serde_json::to_string_pretty(&output).unwrap_or_default()
        }
    }
}
