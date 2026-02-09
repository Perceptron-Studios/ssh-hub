use std::sync::Arc;

use super::schema::RemoteReadInput;
use crate::connection::SshConnection;
use crate::utils::path::{
    format_with_line_numbers, normalize_remote_path, shell_escape_remote_path,
};

pub async fn handle(conn: Arc<SshConnection>, input: RemoteReadInput) -> String {
    let base_path = conn.remote_path().to_string();
    let path = normalize_remote_path(&input.file_path, &base_path);

    let offset = input.offset.unwrap_or(0);
    let has_slicing = offset > 0 || input.limit.is_some();

    if has_slicing {
        // Server-side slicing with sed — transfers only the requested lines
        let start = offset + 1; // sed is 1-indexed
        let end = match input.limit {
            Some(limit) => format!("{}", offset + limit),
            None => "$".to_string(),
        };
        let command = format!(
            "sed -n '{start},{end}p' {}",
            shell_escape_remote_path(&path),
        );
        let line_offset = usize::try_from(offset).unwrap_or(usize::MAX);
        match conn.exec(&command, Some(60_000)).await {
            Ok(result) if result.exit_code == 0 => {
                format_with_line_numbers(&result.stdout, line_offset)
            }
            Ok(result) => format!("Error reading file: {}", result.stderr),
            Err(e) => format!("Error reading file: {e}"),
        }
    } else {
        // Full file read — pass directly to formatter
        match conn.read_file(&path).await {
            Ok(content) => format_with_line_numbers(&content, 0),
            Err(e) => format!("Error reading file: {e}"),
        }
    }
}
