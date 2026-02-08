use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::connection::SshConnection;
use super::schema::{RemoteBashInput, RemoteBashOutput};

/// Default timeout for bash commands (2 minutes).
const DEFAULT_TIMEOUT_MS: u64 = 120_000;

/// Maximum allowed timeout for bash commands (10 minutes).
const MAX_TIMEOUT_MS: u64 = 600_000;

/// Stdout larger than this is saved to disk instead of returned inline.
/// 128 KB keeps the LLM context window manageable while still showing
/// a meaningful amount of output for most commands.
const MAX_INLINE_OUTPUT: usize = 128 * 1024;

/// Number of lines from the start included in the disk-save summary.
const SUMMARY_HEAD_LINES: usize = 150;

/// Number of lines from the end included in the disk-save summary.
const SUMMARY_TAIL_LINES: usize = 50;

pub async fn handle(conn: Arc<SshConnection>, input: RemoteBashInput) -> String {
    let timeout = input.timeout.unwrap_or(DEFAULT_TIMEOUT_MS).min(MAX_TIMEOUT_MS);

    match conn.exec(&input.command, Some(timeout)).await {
        Ok(result) => {
            let stdout = if result.stdout.len() > MAX_INLINE_OUTPUT {
                match save_output_to_disk(&result.stdout).await {
                    Ok(path) => build_output_summary(&result.stdout, &path),
                    Err(e) => {
                        tracing::warn!("Failed to save large output to disk: {}", e);
                        truncate_inline(&result.stdout)
                    }
                }
            } else {
                result.stdout
            };

            let output = RemoteBashOutput {
                stdout,
                stderr: result.stderr,
                exit_code: result.exit_code,
            };
            serde_json::to_string_pretty(&output)
                .unwrap_or_else(|e| format!(r#"{{"error": "serialization failed: {}"}}"#, e))
        }
        Err(e) => format!("Error: {}", e),
    }
}

/// Save stdout to a temp file and return the path.
async fn save_output_to_disk(stdout: &str) -> Result<PathBuf, std::io::Error> {
    let dir = std::env::temp_dir().join("ssh-hub");
    tokio::fs::create_dir_all(&dir).await?;

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let path = dir.join(format!(
        "output-{}.{:03}.log",
        ts.as_secs(),
        ts.subsec_millis(),
    ));

    tokio::fs::write(&path, stdout).await?;
    Ok(path)
}

/// Build a head/tail summary with a pointer to the full output on disk.
fn build_output_summary(stdout: &str, file_path: &Path) -> String {
    let lines: Vec<&str> = stdout.lines().collect();
    let total_lines = lines.len();
    let total_bytes = stdout.len();

    let size_str = if total_bytes >= 1_000_000 {
        format!("{:.1} MB", total_bytes as f64 / 1_000_000.0)
    } else {
        format!("{:.0} KB", total_bytes as f64 / 1_000.0)
    };

    let mut out = String::with_capacity(32 * 1024);

    let _ = writeln!(
        out,
        "[Output too large for context ({}, {} lines)]",
        size_str, total_lines,
    );
    let _ = writeln!(out, "Full output saved to: {}", file_path.display());

    if total_lines > SUMMARY_HEAD_LINES + SUMMARY_TAIL_LINES {
        let _ = writeln!(out, "\n--- First {} lines ---", SUMMARY_HEAD_LINES);
        for line in &lines[..SUMMARY_HEAD_LINES] {
            let _ = writeln!(out, "{}", line);
        }

        let omitted = total_lines - SUMMARY_HEAD_LINES - SUMMARY_TAIL_LINES;
        let _ = writeln!(out, "\n... ({} lines omitted) ...", omitted);

        let _ = writeln!(out, "\n--- Last {} lines ---", SUMMARY_TAIL_LINES);
        for line in &lines[total_lines - SUMMARY_TAIL_LINES..] {
            let _ = writeln!(out, "{}", line);
        }
    } else {
        // Few lines but large bytes (very long lines) — include all
        let _ = write!(out, "\n{}", stdout);
    }

    out
}

/// Fallback: truncate stdout at a char boundary when disk write fails.
fn truncate_inline(stdout: &str) -> String {
    let mut end = MAX_INLINE_OUTPUT;
    while !stdout.is_char_boundary(end) && end > 0 {
        end -= 1;
    }
    let mut s = stdout[..end].to_string();
    s.push_str("\n\n[Output truncated — failed to save to disk]");
    s
}
