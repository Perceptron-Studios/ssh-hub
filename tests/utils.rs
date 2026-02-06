use ssh_hub::utils::checksum::md5_hash;
use ssh_hub::utils::path::{format_with_line_numbers, normalize_remote_path};

#[test]
fn test_md5_hash() {
    let hash = md5_hash(b"hello world");
    assert_eq!(hash, "5eb63bbbe01eeed093cb22bb8f5acdc3");
}

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
