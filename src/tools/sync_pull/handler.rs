use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;

use flate2::read::GzDecoder;

use crate::connection::SshConnection;
use crate::tools::sync_types::SyncOutput;
use crate::utils::path::normalize_remote_path;
use super::schema::SyncPullInput;

/// Timeout for the remote `test -d` probe (10 seconds).
const PROBE_TIMEOUT_MS: u64 = 10_000;

/// Timeout for tar-based directory sync operations (2 minutes).
const SYNC_TIMEOUT_MS: u64 = 120_000;

pub async fn handle(conn: Arc<SshConnection>, input: SyncPullInput) -> String {
    let base_path = conn.remote_path().to_string();
    let remote_path = normalize_remote_path(&input.remote_path, &base_path);

    // Determine if remote path is file or directory
    let is_dir = match conn
        .exec(
            &format!("test -d '{}' && echo dir || echo file", remote_path),
            Some(PROBE_TIMEOUT_MS),
        )
        .await
    {
        Ok(result) => result.stdout.trim() == "dir",
        Err(_) => false,
    };

    if is_dir || input.files.is_some() {
        let local_dest = input.local_path.unwrap_or_else(|| ".".to_string());
        return pull_directory(&conn, &remote_path, &local_dest, input.files.as_deref()).await;
    }

    // Single file
    let local_dest = input.local_path.unwrap_or_else(|| {
        Path::new(&input.remote_path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "downloaded_file".to_string())
    });
    pull_single_file(&conn, &remote_path, &local_dest).await
}

async fn pull_single_file(conn: &SshConnection, remote_path: &str, local_dest: &str) -> String {
    let content = match conn.read_file(remote_path).await {
        Ok(c) => c,
        Err(e) => {
            return SyncOutput::failure(remote_path, format!("Error reading remote file: {}", e))
                .to_json();
        }
    };

    // Ensure parent directory exists locally
    if let Some(parent) = Path::new(local_dest).parent() {
        if !parent.as_os_str().is_empty() {
            if let Err(e) = tokio::fs::create_dir_all(parent).await {
                return SyncOutput::failure(
                    local_dest,
                    format!("Error creating local directory: {}", e),
                )
                .to_json();
            }
        }
    }

    match tokio::fs::write(local_dest, &content).await {
        Ok(()) => SyncOutput::success(vec![local_dest.to_string()]).to_json(),
        Err(e) => SyncOutput::failure(local_dest, e.to_string()).to_json(),
    }
}

async fn pull_directory(
    conn: &SshConnection,
    remote_path: &str,
    local_dest: &str,
    files_filter: Option<&[String]>,
) -> String {
    // Build tar command
    let files_arg = match files_filter {
        Some(files) => files
            .iter()
            .map(|f| format!("'{}'", f))
            .collect::<Vec<_>>()
            .join(" "),
        None => ".".to_string(),
    };
    let command = format!("tar czf - -C '{}' {}", remote_path, files_arg);

    // Get raw tar bytes from remote
    let raw_result = match conn.exec_raw(&command, None, Some(SYNC_TIMEOUT_MS)).await {
        Ok(r) => r,
        Err(e) => {
            return SyncOutput::failure(remote_path, format!("Error running remote tar: {}", e))
                .to_json();
        }
    };

    if raw_result.exit_code != 0 {
        return SyncOutput::failure(
            remote_path,
            format!(
                "Remote tar failed (exit {}): {}",
                raw_result.exit_code, raw_result.stderr
            ),
        )
        .to_json();
    }

    // Create local destination
    let dest = Path::new(local_dest);
    if let Err(e) = tokio::fs::create_dir_all(dest).await {
        return SyncOutput::failure(
            local_dest,
            format!("Error creating local directory: {}", e),
        )
        .to_json();
    }

    // Extract tar.gz locally
    let decoder = GzDecoder::new(Cursor::new(&raw_result.stdout));
    let mut archive = tar::Archive::new(decoder);

    let entries = match archive.entries() {
        Ok(entries) => entries,
        Err(e) => {
            return SyncOutput::failure(
                local_dest,
                format!("Error extracting archive: {}", e),
            )
            .to_json();
        }
    };

    let pulled: Vec<String> = entries
        .filter_map(|entry| entry.ok())
        .filter_map(|mut entry| {
            let path = entry.path().ok()?.to_string_lossy().to_string();
            entry.unpack_in(dest).ok()?;
            Some(path)
        })
        .collect();

    SyncOutput::success(pulled).to_json()
}
