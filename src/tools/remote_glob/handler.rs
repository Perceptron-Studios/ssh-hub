use std::sync::Arc;

use serde_json::json;

use crate::connection::SshConnection;
use super::schema::RemoteGlobInput;

pub async fn handle(conn: Arc<SshConnection>, input: RemoteGlobInput) -> String {
    let base_path = conn.remote_path().to_string();
    let path = input.path.as_deref().unwrap_or(&base_path);

    match conn.glob(&input.pattern, Some(path)).await {
        Ok(files) => {
            let result = json!({ "files": files });
            serde_json::to_string(&result)
                .unwrap_or_else(|e| format!(r#"{{"error": "serialization failed: {}"}}"#, e))
        }
        Err(e) => format!("Error searching files: {}", e),
    }
}
