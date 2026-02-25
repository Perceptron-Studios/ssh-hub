use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::schema::{RemoteBashBackgroundOutput, RemoteBashInput, RemoteBashOutput};
use crate::connection::SshConnection;
use crate::utils::path::shell_escape;

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

/// Timeout for the background wrapper command itself (get PID back).
const BACKGROUND_TIMEOUT_MS: u64 = 10_000;

/// SI kilobyte (1000 bytes), used for human-readable size display.
const BYTES_PER_KB: usize = 1_000;
/// SI megabyte (1,000,000 bytes), used for human-readable size display.
const BYTES_PER_MB: usize = 1_000_000;
const TENTHS_MB_DIVISOR: usize = 100_000;

/// Execute a bash command on the remote server.
///
/// Dispatches to foreground or background execution based on `input.run_in_background`.
/// Rejects commands that attempt shell-level backgrounding without using the
/// `run_in_background` flag, since those break the SSH channel.
pub async fn handle(conn: Arc<SshConnection>, input: RemoteBashInput) -> String {
    let run_in_background = input.run_in_background.unwrap_or(false);

    if !run_in_background {
        if let Some(reason) = detect_background_pattern(&input.command) {
            return format!(
                "Error: command appears to use shell-level backgrounding ({reason}). \
                 This will hang the SSH channel. Use the `run_in_background` parameter \
                 instead and pass the raw command without nohup/setsid/& wrappers."
            );
        }
    }

    if run_in_background {
        handle_background(conn, input).await
    } else {
        handle_foreground(conn, input).await
    }
}

/// Run the command detached on the remote server and return immediately with PID and log path.
async fn handle_background(conn: Arc<SshConnection>, input: RemoteBashInput) -> String {
    let log_file = format!("/tmp/ssh-hub-bg-{}.log", timestamp_suffix());

    // Detach the background process from the SSH session so the channel
    // closes immediately after echoing the PID.
    //
    // `setsid` (Linux, part of util-linux) creates a new session, fully
    // detaching from the SSH session's process group. Without it, sshd
    // keeps the channel open until ALL processes in the session exit —
    // even when stdout/stderr are redirected away from the pipe.
    //
    // Falls back to `nohup ... &` on systems without `setsid` (macOS, BSD).
    // The fallback works for most cases (FD redirection breaks the pipe
    // connection) but can fail with long-running processes that inherit
    // session membership.
    let cmd = shell_escape(&input.command);
    let log = shell_escape(&log_file);
    let wrapped = format!(
        "if command -v setsid >/dev/null 2>&1; then \
             setsid sh -c {cmd} > {log} 2>&1 < /dev/null & \
         else \
             nohup sh -c {cmd} > {log} 2>&1 < /dev/null & \
         fi; echo $!",
    );

    let result = match conn.exec(&wrapped, Some(BACKGROUND_TIMEOUT_MS)).await {
        Ok(result) => result,
        Err(e) => return format!("Error launching background command: {e}"),
    };

    let pid = result.stdout.trim().to_string();
    if pid.is_empty() || pid.parse::<u32>().is_err() {
        return format!(
            "Error: background launch did not return a valid PID. Output: {}",
            result.stdout.trim(),
        );
    }

    let output = RemoteBashBackgroundOutput {
        pid,
        log_file,
        message: "Command launched in background.".to_string(),
    };
    serde_json::to_string_pretty(&output)
        .unwrap_or_else(|e| format!(r#"{{"error": "serialization failed: {e}"}}"#))
}

async fn handle_foreground(conn: Arc<SshConnection>, input: RemoteBashInput) -> String {
    let timeout = input
        .timeout
        .unwrap_or(DEFAULT_TIMEOUT_MS)
        .min(MAX_TIMEOUT_MS);

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
                .unwrap_or_else(|e| format!(r#"{{"error": "serialization failed: {e}"}}"#))
        }
        Err(e) => format!("Error: {e}"),
    }
}

/// Generate a millisecond-precision timestamp suffix for unique file names.
fn timestamp_suffix() -> String {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}.{:03}", ts.as_secs(), ts.subsec_millis())
}

/// Save stdout to a temp file and return the path.
async fn save_output_to_disk(stdout: &str) -> Result<PathBuf, std::io::Error> {
    let dir = std::env::temp_dir().join("ssh-hub");
    tokio::fs::create_dir_all(&dir).await?;

    let path = dir.join(format!("output-{}.log", timestamp_suffix()));
    tokio::fs::write(&path, stdout).await?;
    Ok(path)
}

/// Build a head/tail summary with a pointer to the full output on disk.
fn build_output_summary(stdout: &str, file_path: &Path) -> String {
    let lines: Vec<&str> = stdout.lines().collect();
    let total_lines = lines.len();
    let total_bytes = stdout.len();

    let size_str = if total_bytes >= BYTES_PER_MB {
        let tenths_mb = total_bytes / TENTHS_MB_DIVISOR;
        format!("{}.{} MB", tenths_mb / 10, tenths_mb % 10)
    } else {
        format!("{} KB", total_bytes / BYTES_PER_KB)
    };

    let mut out = String::with_capacity(32 * 1024);

    let _ = writeln!(
        out,
        "[Output too large for context ({size_str}, {total_lines} lines)]",
    );
    let _ = writeln!(out, "Full output saved to: {}", file_path.display());

    if total_lines > SUMMARY_HEAD_LINES + SUMMARY_TAIL_LINES {
        let _ = writeln!(out, "\n--- First {SUMMARY_HEAD_LINES} lines ---");
        for line in &lines[..SUMMARY_HEAD_LINES] {
            let _ = writeln!(out, "{line}");
        }

        let omitted = total_lines - SUMMARY_HEAD_LINES - SUMMARY_TAIL_LINES;
        let _ = writeln!(out, "\n... ({omitted} lines omitted) ...");

        let _ = writeln!(out, "\n--- Last {SUMMARY_TAIL_LINES} lines ---");
        for line in &lines[total_lines - SUMMARY_TAIL_LINES..] {
            let _ = writeln!(out, "{line}");
        }
    } else {
        // Few lines but large bytes (very long lines) — include all
        let _ = write!(out, "\n{stdout}");
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

/// Detect shell-level backgrounding patterns that would hang the SSH channel.
///
/// Returns a short description of the detected pattern, or `None` if the
/// command looks like a normal foreground command.
#[must_use]
pub fn detect_background_pattern(command: &str) -> Option<&'static str> {
    let trimmed = command.trim();

    // Check if a keyword appears in command position (start of command or
    // after a shell operator like &&, ||, ;) rather than as an argument.
    let in_command_position = |keyword: &str| -> bool {
        if trimmed.starts_with(keyword) && trimmed[keyword.len()..].starts_with(' ') {
            return true;
        }
        for sep in ["&& ", "|| ", "; "] {
            for part in trimmed.split(sep) {
                let part = part.trim();
                if part.starts_with(keyword) && part[keyword.len()..].starts_with(' ') {
                    return true;
                }
            }
        }
        false
    };

    if in_command_position("nohup") {
        return Some("nohup");
    }

    if in_command_position("setsid") {
        return Some("setsid");
    }

    // Trailing & (but not &&) — strip common suffixes like `echo $!` and `disown`
    let stripped = trimmed
        .trim_end()
        .trim_end_matches("echo $!")
        .trim_end_matches(';')
        .trim_end_matches("disown")
        .trim_end_matches(';')
        .trim_end();
    if stripped.ends_with('&') && !stripped.ends_with("&&") {
        return Some("trailing &");
    }

    None
}
