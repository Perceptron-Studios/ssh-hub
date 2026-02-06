use rmcp::schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SyncStatusInput {
    #[schemars(description = "Name of the connected server to target (e.g., 'staging')")]
    pub server: String,

    #[schemars(description = "Local directory path to compare")]
    pub local_path: String,

    #[schemars(description = "Remote path to compare (default: connection base path)")]
    pub remote_path: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FileStatus {
    pub path: String,
    pub status: SyncState,
    pub local_modified: Option<String>,
    pub remote_modified: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncState {
    LocalOnly,
    RemoteOnly,
    Modified,
    InSync,
}

#[derive(Debug, Serialize)]
pub struct SyncSummary {
    pub local_only: usize,
    pub remote_only: usize,
    pub modified: usize,
    pub in_sync: usize,
}

#[derive(Debug, Serialize)]
pub struct GitInfo {
    pub local_branch: String,
    pub remote_branch: String,
    pub local_commit: String,
    pub remote_commit: String,
    pub behind_by: Option<usize>,
    pub ahead_by: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct SyncStatusOutput {
    pub method: String,
    pub files: Vec<FileStatus>,
    pub summary: SyncSummary,
    pub git_info: Option<GitInfo>,
}
