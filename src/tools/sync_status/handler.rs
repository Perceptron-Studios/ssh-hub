use std::sync::Arc;

use crate::connection::SshConnection;
use super::schema::{SyncStatusInput, SyncStatusOutput, SyncSummary};

pub async fn handle(_conn: Arc<SshConnection>, _input: SyncStatusInput) -> String {
    // TODO: implement actual sync status comparison
    let output = SyncStatusOutput {
        method: "checksum".to_string(),
        files: vec![],
        summary: SyncSummary {
            local_only: 0,
            remote_only: 0,
            modified: 0,
            in_sync: 0,
        },
        git_info: None,
    };
    serde_json::to_string_pretty(&output).unwrap_or_default()
}
