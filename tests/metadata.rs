use ssh_hub::metadata::SystemMetadata;
use ssh_hub::metadata::{diff, parse_output};

#[test]
fn test_parse_linux_output() {
    let output = "\
ARCH=x86_64\n\
OS=linux\n\
DISTRO=Ubuntu 22.04.3 LTS\n\
SHELL=/bin/bash\n\
PKG_MANAGER=apt\n";

    let meta = parse_output(output).unwrap();
    assert_eq!(meta.os.as_deref(), Some("linux"));
    assert_eq!(meta.distro.as_deref(), Some("Ubuntu 22.04.3 LTS"));
    assert_eq!(meta.arch.as_deref(), Some("x86_64"));
    assert_eq!(meta.shell.as_deref(), Some("/bin/bash"));
    assert_eq!(meta.package_manager.as_deref(), Some("apt"));
    assert!(meta.collected_at.is_some());
}

#[test]
fn test_parse_macos_output() {
    let output = "\
ARCH=arm64\n\
OS=darwin\n\
DISTRO=macOS 14.2.1\n\
SHELL=/bin/zsh\n\
PKG_MANAGER=brew\n";

    let meta = parse_output(output).unwrap();
    assert_eq!(meta.os.as_deref(), Some("darwin"));
    assert_eq!(meta.distro.as_deref(), Some("macOS 14.2.1"));
    assert_eq!(meta.arch.as_deref(), Some("arm64"));
    assert_eq!(meta.shell.as_deref(), Some("/bin/zsh"));
    assert_eq!(meta.package_manager.as_deref(), Some("brew"));
}

#[test]
fn test_parse_partial_output() {
    let output = "ARCH=x86_64\nOS=linux\n";

    let meta = parse_output(output).unwrap();
    assert_eq!(meta.arch.as_deref(), Some("x86_64"));
    assert_eq!(meta.os.as_deref(), Some("linux"));
    assert!(meta.distro.is_none());
    assert!(meta.shell.is_none());
    assert!(meta.package_manager.is_none());
}

#[test]
fn test_parse_empty_output() {
    let meta = parse_output("").unwrap();
    assert!(meta.os.is_none());
    assert!(meta.distro.is_none());
    assert!(meta.arch.is_none());
    assert!(meta.shell.is_none());
    assert!(meta.package_manager.is_none());
    assert!(meta.collected_at.is_some());
}

#[test]
fn test_parse_ignores_unknown_keys() {
    let output = "ARCH=x86_64\nUNKNOWN_KEY=whatever\nOS=linux\n";
    let meta = parse_output(output).unwrap();
    assert_eq!(meta.arch.as_deref(), Some("x86_64"));
    assert_eq!(meta.os.as_deref(), Some("linux"));
}

#[test]
fn test_parse_skips_empty_values() {
    let output = "ARCH=\nOS=linux\n";
    let meta = parse_output(output).unwrap();
    assert!(meta.arch.is_none());
    assert_eq!(meta.os.as_deref(), Some("linux"));
}

#[test]
fn test_diff_no_change() {
    let a = SystemMetadata {
        os: Some("linux".into()),
        distro: Some("Ubuntu 22.04".into()),
        ..Default::default()
    };
    let b = a.clone();
    assert!(diff(&a, &b).is_none());
}

#[test]
fn test_diff_with_changes() {
    let a = SystemMetadata {
        os: Some("linux".into()),
        distro: Some("Ubuntu 22.04".into()),
        ..Default::default()
    };
    let b = SystemMetadata {
        os: Some("linux".into()),
        distro: Some("Ubuntu 24.04".into()),
        ..Default::default()
    };
    let msg = diff(&a, &b).expect("expected diff to report changes");
    assert!(msg.contains("distro"));
    assert!(msg.contains("Ubuntu 22.04"));
    assert!(msg.contains("Ubuntu 24.04"));
}

#[test]
fn test_diff_ignores_collected_at() {
    let a = SystemMetadata {
        os: Some("linux".into()),
        collected_at: Some(1000),
        ..Default::default()
    };
    let b = SystemMetadata {
        os: Some("linux".into()),
        collected_at: Some(2000),
        ..Default::default()
    };
    assert!(diff(&a, &b).is_none());
}

#[test]
fn test_diff_from_none_to_some() {
    let a = SystemMetadata::default();
    let b = SystemMetadata {
        os: Some("linux".into()),
        ..Default::default()
    };
    let msg = diff(&a, &b).expect("expected diff to report changes");
    assert!(msg.contains("os"));
    assert!(msg.contains("(none)"));
    assert!(msg.contains("linux"));
}

#[test]
fn test_diff_from_some_to_none() {
    let a = SystemMetadata {
        os: Some("linux".into()),
        arch: Some("x86_64".into()),
        ..Default::default()
    };
    let b = SystemMetadata {
        os: Some("linux".into()),
        ..Default::default()
    };
    let msg = diff(&a, &b).expect("expected diff to report changes");
    assert!(msg.contains("arch"));
    assert!(msg.contains("x86_64"));
    assert!(msg.contains("(none)"));
}

#[test]
fn test_diff_multiple_fields_changed() {
    let a = SystemMetadata {
        os: Some("linux".into()),
        distro: Some("Ubuntu 22.04".into()),
        arch: Some("x86_64".into()),
        shell: Some("/bin/bash".into()),
        package_manager: Some("apt".into()),
        ..Default::default()
    };
    let b = SystemMetadata {
        os: Some("darwin".into()),
        distro: Some("macOS 14.2".into()),
        arch: Some("arm64".into()),
        shell: Some("/bin/zsh".into()),
        package_manager: Some("brew".into()),
        ..Default::default()
    };
    let msg = diff(&a, &b).expect("expected diff to report changes");
    assert!(msg.contains("os"));
    assert!(msg.contains("distro"));
    assert!(msg.contains("arch"));
    assert!(msg.contains("shell"));
    assert!(msg.contains("package_manager"));
}
