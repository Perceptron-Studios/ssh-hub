use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};

/// Escape a string for safe interpolation into a POSIX shell command.
/// Wraps in single quotes with internal `'` escaped as `'\''`.
#[must_use]
pub fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Shell-escape a remote path, expanding `~` to `$HOME` so tilde expansion
/// isn't broken by single-quoting. Use this for any path that might be `~` or
/// `~/...` and will appear inside a shell command string.
#[must_use]
pub fn shell_escape_remote_path(path: &str) -> String {
    if path == "~" {
        "$HOME".to_string()
    } else if let Some(rest) = path.strip_prefix("~/") {
        format!("$HOME/{}", shell_escape(rest))
    } else {
        shell_escape(path)
    }
}

/// Validate that a relative path stays within the base directory after resolution.
/// Canonicalizes both paths to catch `..` traversal and symlink escapes.
/// Returns the canonical path on success.
///
/// # Errors
///
/// Returns an error if either path cannot be canonicalized or the resolved
/// path escapes the base directory.
pub fn validate_path_within(base_dir: &Path, relative: &str) -> Result<PathBuf> {
    let full_path = base_dir.join(relative);
    let canon_base = base_dir
        .canonicalize()
        .map_err(|e| anyhow!("Cannot canonicalize base dir: {e}"))?;
    let canon_full = full_path
        .canonicalize()
        .map_err(|e| anyhow!("Cannot resolve '{relative}': {e}"))?;

    if !canon_full.starts_with(&canon_base) {
        return Err(anyhow!(
            "Path traversal rejected: '{relative}' resolves outside base directory"
        ));
    }

    Ok(canon_full)
}

/// Normalize a path relative to the base remote path
#[must_use]
pub fn normalize_remote_path(path: &str, base_path: &str) -> String {
    if path.starts_with('/') || path.starts_with('~') {
        // Absolute or home-relative path - use as-is
        path.to_string()
    } else {
        // Relative path - join with base
        let base = Path::new(base_path);
        base.join(path).to_string_lossy().to_string()
    }
}

/// Format file content with line numbers (like Claude Code's Read tool output).
///
/// Uses a single pre-allocated `String` instead of collecting into a `Vec` and joining.
#[must_use]
pub fn format_with_line_numbers(content: &str, offset: usize) -> String {
    use std::fmt::Write;

    // Estimate: ~8 chars prefix + average line length
    let estimated = content.len() + content.lines().count() * 8;
    let mut result = String::with_capacity(estimated);

    for (i, line) in content.lines().enumerate() {
        if i > 0 {
            result.push('\n');
        }
        let _ = write!(result, "{:>6}\u{2192}{}", offset + i + 1, line);
    }

    result
}
