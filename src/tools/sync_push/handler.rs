use std::path::Path;
use std::sync::Arc;

use flate2::write::GzEncoder;
use flate2::Compression;

use crate::connection::SshConnection;
use crate::tools::sync_types::SyncOutput;
use crate::utils::path::normalize_remote_path;
use super::schema::SyncPushInput;

/// Timeout for tar-based directory sync operations (2 minutes).
const SYNC_TIMEOUT_MS: u64 = 120_000;

/// Build a tar.gz archive in memory from files under `base_dir`.
/// `files` are relative paths within `base_dir`.
fn build_tar_gz(base_dir: &Path, files: &[String]) -> anyhow::Result<Vec<u8>> {
    let enc = GzEncoder::new(Vec::new(), Compression::default());
    let mut tar = tar::Builder::new(enc);

    for file in files {
        let full_path = base_dir.join(file);
        tar.append_path_with_name(&full_path, file)
            .map_err(|e| anyhow::anyhow!("Failed to add '{}' to archive: {}", file, e))?;
    }

    let enc = tar.into_inner()?;
    let bytes = enc.finish()?;
    Ok(bytes)
}

/// Recursively collect all file paths under `dir`, relative to `dir`.
fn walk_dir(dir: &Path) -> anyhow::Result<Vec<String>> {
    let mut files = Vec::new();
    walk_dir_inner(dir, dir, &mut files)?;
    Ok(files)
}

fn walk_dir_inner(base: &Path, current: &Path, files: &mut Vec<String>) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            walk_dir_inner(base, &path, files)?;
        } else {
            let relative = path
                .strip_prefix(base)
                .map_err(|e| anyhow::anyhow!("Path prefix error: {}", e))?;
            files.push(relative.to_string_lossy().to_string());
        }
    }
    Ok(())
}

pub async fn handle(conn: Arc<SshConnection>, input: SyncPushInput) -> String {
    let base_path = conn.remote_path().to_string();
    let local = Path::new(&input.local_path);

    let remote_dest = input
        .remote_path
        .unwrap_or_else(|| normalize_remote_path(&input.local_path, &base_path));

    if local.is_file() {
        return push_single_file(&conn, local, &remote_dest).await;
    }

    if local.is_dir() {
        return push_directory(&conn, local, &remote_dest, input.files.as_deref()).await;
    }

    SyncOutput::failure(input.local_path, "Path is neither a file nor a directory").to_json()
}

async fn push_single_file(conn: &SshConnection, local: &Path, remote_dest: &str) -> String {
    let path_str = local.display().to_string();

    let content = match tokio::fs::read_to_string(local).await {
        Ok(c) => c,
        Err(e) => {
            return SyncOutput::failure(&path_str, format!("Error reading local file: {}", e))
                .to_json();
        }
    };

    match conn.write_file(remote_dest, &content).await {
        Ok(()) => SyncOutput::success(vec![path_str]).to_json(),
        Err(e) => SyncOutput::failure(path_str, e.to_string()).to_json(),
    }
}

async fn push_directory(
    conn: &SshConnection,
    local_dir: &Path,
    remote_dest: &str,
    files_filter: Option<&[String]>,
) -> String {
    let dir_str = local_dir.display().to_string();

    // Collect file list
    let files = match files_filter {
        Some(f) => f.to_vec(),
        None => match walk_dir(local_dir) {
            Ok(f) => f,
            Err(e) => {
                return SyncOutput::failure(&dir_str, format!("Error walking directory: {}", e))
                    .to_json();
            }
        },
    };

    if files.is_empty() {
        return SyncOutput::failure(&dir_str, "No files to push").to_json();
    }

    // Build tar.gz in memory
    let tar_bytes = match build_tar_gz(local_dir, &files) {
        Ok(b) => b,
        Err(e) => {
            return SyncOutput::failure(&dir_str, format!("Error building archive: {}", e))
                .to_json();
        }
    };

    // Stream to remote via stdin
    let command = format!("mkdir -p '{}' && tar xzf - -C '{}'", remote_dest, remote_dest);
    match conn.exec_raw(&command, Some(&tar_bytes), Some(SYNC_TIMEOUT_MS)).await {
        Ok(result) if result.exit_code == 0 => SyncOutput::success(files).to_json(),
        Ok(result) => SyncOutput::failure(
            &dir_str,
            format!(
                "Remote tar extraction failed (exit {}): {}",
                result.exit_code, result.stderr
            ),
        )
        .to_json(),
        Err(e) => SyncOutput::failure(dir_str, e.to_string()).to_json(),
    }
}
