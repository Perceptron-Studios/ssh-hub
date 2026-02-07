use std::sync::Arc;

use crate::connection::SshConnection;
use crate::utils::path::normalize_remote_path;
use super::schema::RemoteEditInput;

pub async fn handle(conn: Arc<SshConnection>, input: RemoteEditInput) -> String {
    let base_path = conn.remote_path().to_string();
    let path = normalize_remote_path(&input.file_path, &base_path);

    let content = match conn.read_file(&path).await {
        Ok(c) => c,
        Err(e) => return format!("Error reading file: {}", e),
    };

    let replace_all = input.replace_all.unwrap_or(false);
    let new_content = if replace_all {
        content.replace(&input.old_string, &input.new_string)
    } else {
        content.replacen(&input.old_string, &input.new_string, 1)
    };

    // If unchanged, the old_string was never found
    if new_content == content {
        return format!("String '{}' not found in file", input.old_string);
    }

    match conn.write_file(&path, &new_content).await {
        Ok(()) => format!("Successfully edited {}", path),
        Err(e) => format!("Error writing file: {}", e),
    }
}
