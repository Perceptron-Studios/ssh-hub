use std::sync::Arc;

use crate::connection::SshConnection;
use crate::utils::path::{format_with_line_numbers, normalize_remote_path};
use super::schema::RemoteReadInput;

pub async fn handle(conn: Arc<SshConnection>, input: RemoteReadInput) -> String {
    let base_path = conn.remote_path().to_string();
    let path = normalize_remote_path(&input.file_path, &base_path);

    match conn.read_file(&path).await {
        Ok(content) => {
            let offset = input.offset.unwrap_or(0) as usize;
            let limit = input.limit.map(|l| l as usize).unwrap_or(usize::MAX);

            let sliced: Vec<&str> = content.lines().skip(offset).take(limit).collect();

            format_with_line_numbers(&sliced.join("\n"), offset)
        }
        Err(e) => format!("Error reading file: {}", e),
    }
}
