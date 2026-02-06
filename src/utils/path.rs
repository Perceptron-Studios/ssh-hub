use std::path::Path;

/// Normalize a path relative to the base remote path
pub fn normalize_remote_path(path: &str, base_path: &str) -> String {
    if path.starts_with('/') {
        // Absolute path - use as-is
        path.to_string()
    } else {
        // Relative path - join with base
        let base = Path::new(base_path);
        base.join(path).to_string_lossy().to_string()
    }
}

/// Format file content with line numbers (like Claude Code's Read tool output)
pub fn format_with_line_numbers(content: &str, offset: usize) -> String {
    content
        .lines()
        .enumerate()
        .map(|(i, line)| {
            let line_num = offset + i + 1;
            format!("{:>6}\u{2192}{}", line_num, line)
        })
        .collect::<Vec<_>>()
        .join("\n")
}