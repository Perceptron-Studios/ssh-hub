use ssh_hub::utils::path::{
    format_with_line_numbers, normalize_remote_path, shell_escape, validate_path_within,
};

#[test]
fn test_normalize_absolute_path() {
    assert_eq!(
        normalize_remote_path("/etc/config", "/home/user"),
        "/etc/config"
    );
}

#[test]
fn test_normalize_relative_path() {
    assert_eq!(
        normalize_remote_path("src/main.rs", "/home/user/project"),
        "/home/user/project/src/main.rs"
    );
}

#[test]
fn test_format_with_line_numbers() {
    let content = "line1\nline2\nline3";
    let formatted = format_with_line_numbers(content, 0);
    assert!(formatted.contains("1\u{2192}line1"));
    assert!(formatted.contains("2\u{2192}line2"));
}

// ── shell_escape tests ──────────────────────────────────────────────

#[test]
fn test_shell_escape_simple() {
    assert_eq!(shell_escape("hello"), "'hello'");
}

#[test]
fn test_shell_escape_empty() {
    assert_eq!(shell_escape(""), "''");
}

#[test]
fn test_shell_escape_with_single_quote() {
    assert_eq!(shell_escape("it's"), "'it'\\''s'");
}

#[test]
fn test_shell_escape_with_spaces() {
    assert_eq!(shell_escape("path with spaces"), "'path with spaces'");
}

#[test]
fn test_shell_escape_command_substitution() {
    assert_eq!(shell_escape("$(rm -rf /)"), "'$(rm -rf /)'");
}

#[test]
fn test_shell_escape_backticks() {
    assert_eq!(shell_escape("`whoami`"), "'`whoami`'");
}

// ── validate_path_within tests ──────────────────────────────────────

#[test]
fn test_validate_path_within_normal() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("test.txt");
    std::fs::write(&file_path, "hello").unwrap();
    assert!(validate_path_within(dir.path(), "test.txt").is_ok());
}

#[test]
fn test_validate_path_within_traversal_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let result = validate_path_within(dir.path(), "../../etc/passwd");
    assert!(result.is_err());
}

#[test]
#[cfg(unix)]
fn test_validate_path_within_symlink_escape() {
    let dir = tempfile::tempdir().unwrap();
    let link_path = dir.path().join("escape");
    std::os::unix::fs::symlink("/etc", &link_path).unwrap();
    let result = validate_path_within(dir.path(), "escape/hosts");
    assert!(result.is_err());
}

#[test]
fn test_validate_path_within_nested_ok() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("sub/deep")).unwrap();
    let file_path = dir.path().join("sub/deep/file.txt");
    std::fs::write(&file_path, "ok").unwrap();
    assert!(validate_path_within(dir.path(), "sub/deep/file.txt").is_ok());
}
