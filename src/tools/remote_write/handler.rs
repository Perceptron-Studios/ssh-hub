use std::sync::Arc;

use super::schema::RemoteWriteInput;
use crate::connection::SshConnection;
use crate::utils::path::normalize_remote_path;

pub async fn handle(conn: Arc<SshConnection>, input: RemoteWriteInput) -> String {
    let base_path = conn.remote_path().to_string();
    let path = normalize_remote_path(&input.file_path, &base_path);

    match conn.write_file(&path, &input.content).await {
        Ok(()) => format!("Successfully wrote to {path}"),
        Err(e) => format!("Error writing file: {e}"),
    }
}
