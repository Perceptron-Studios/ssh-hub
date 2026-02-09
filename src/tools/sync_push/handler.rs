use std::path::Path;
use std::sync::Arc;

use flate2::write::GzEncoder;
use flate2::Compression;

use super::schema::SyncPushInput;
use crate::connection::SshConnection;
use crate::tools::sync_types::SyncOutput;
use crate::utils::gitignore::GitIgnore;
use crate::utils::path::{normalize_remote_path, shell_escape_remote_path, validate_path_within};

/// Timeout for tar-based directory sync operations (2 minutes).
const SYNC_TIMEOUT_MS: u64 = 120_000;

/// Build a tar.gz archive in memory from files under `base_dir`.
/// `files` are relative paths within `base_dir`.
fn build_tar_gz(base_dir: &Path, files: &[String]) -> anyhow::Result<Vec<u8>> {
    let enc = GzEncoder::new(Vec::new(), Compression::default());
    let mut tar = tar::Builder::new(enc);

    for file in files {
        let full_path = validate_path_within(base_dir, file)?;
        tar.append_path_with_name(&full_path, file)
            .map_err(|e| anyhow::anyhow!("Failed to add '{file}' to archive: {e}"))?;
    }

    let enc = tar.into_inner()?;
    let bytes = enc.finish()?;
    Ok(bytes)
}

/// Recursively collect files under `dir`, respecting .gitignore and exclude patterns.
/// Skips symlinks, `.git/`, and gitignored entries.
fn walk_dir(dir: &Path, gitignore: &GitIgnore) -> anyhow::Result<Vec<String>> {
    let mut files = Vec::new();
    walk_dir_inner(dir, dir, gitignore, &mut files)?;
    Ok(files)
}

fn walk_dir_inner(
    base: &Path,
    current: &Path,
    gitignore: &GitIgnore,
    files: &mut Vec<String>,
) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(current)? {
        let entry = entry?;
        let file_type = entry.file_type()?;

        // Skip symlinks — file_type() uses lstat, doesn't follow
        if file_type.is_symlink() {
            continue;
        }

        let path = entry.path();
        let relative = path
            .strip_prefix(base)
            .map_err(|e| anyhow::anyhow!("Path prefix error: {e}"))?
            .to_string_lossy()
            .to_string();

        if file_type.is_dir() {
            // Always skip .git
            if entry.file_name().to_str() == Some(".git") {
                continue;
            }

            // Check gitignore for this directory — skips the entire subtree
            if gitignore.is_ignored(&relative, true) {
                continue;
            }

            walk_dir_inner(base, &path, gitignore, files)?;
        } else if file_type.is_file() {
            if gitignore.is_ignored(&relative, false) {
                continue;
            }

            files.push(relative);
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
        return push_directory(&conn, local, &remote_dest, input.exclude.as_deref()).await;
    }

    SyncOutput::failure(input.local_path, "Path is neither a file nor a directory").to_json()
}

async fn push_single_file(conn: &SshConnection, local: &Path, remote_dest: &str) -> String {
    let path_str = local.display().to_string();

    let content = match tokio::fs::read(local).await {
        Ok(c) => c,
        Err(e) => {
            return SyncOutput::failure(&path_str, format!("Error reading local file: {e}"))
                .to_json();
        }
    };

    match conn.write_file_raw(remote_dest, &content).await {
        Ok(()) => SyncOutput::success(vec![path_str]).to_json(),
        Err(e) => SyncOutput::failure(path_str, e.to_string()).to_json(),
    }
}

async fn push_directory(
    conn: &SshConnection,
    local_dir: &Path,
    remote_dest: &str,
    exclude: Option<&[String]>,
) -> String {
    let dir_str = local_dir.display().to_string();

    // Collect file list — gitignore-aware, symlink-safe
    let dir_owned = local_dir.to_path_buf();
    let exclude_owned = exclude.map(ToOwned::to_owned);
    let files = match tokio::task::spawn_blocking(move || {
        let mut gitignore = GitIgnore::from_file(&dir_owned.join(".gitignore"));
        if let Some(patterns) = &exclude_owned {
            gitignore.extend_patterns(patterns);
        }
        walk_dir(&dir_owned, &gitignore)
    })
    .await
    {
        Ok(Ok(f)) => f,
        Ok(Err(e)) => {
            return SyncOutput::failure(&dir_str, format!("Error walking directory: {e}"))
                .to_json();
        }
        Err(e) => {
            return SyncOutput::failure(&dir_str, format!("Directory walk task panicked: {e}"))
                .to_json();
        }
    };

    if files.is_empty() {
        return SyncOutput::failure(&dir_str, "No files to push").to_json();
    }

    // Build tar.gz in memory (CPU-bound gzip compression)
    let dir_owned = local_dir.to_path_buf();
    let file_list = files.clone(); // kept for the success response
    let tar_bytes = match tokio::task::spawn_blocking(move || build_tar_gz(&dir_owned, &files))
        .await
    {
        Ok(Ok(b)) => b,
        Ok(Err(e)) => {
            return SyncOutput::failure(&dir_str, format!("Error building archive: {e}")).to_json();
        }
        Err(e) => {
            return SyncOutput::failure(&dir_str, format!("Archive build task panicked: {e}"))
                .to_json();
        }
    };

    // Stream to remote via stdin
    let escaped = shell_escape_remote_path(remote_dest);
    let command = format!("mkdir -p {escaped} && tar xzf - -C {escaped}");
    match conn
        .exec_raw(&command, Some(&tar_bytes), Some(SYNC_TIMEOUT_MS))
        .await
    {
        Ok(result) if result.exit_code == 0 => SyncOutput::success(file_list).to_json(),
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
