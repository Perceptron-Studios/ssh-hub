use ssh_hub::metadata::SystemMetadata;
use ssh_hub::server_registry::{AuthMethod, ServerEntry, ServerRegistry};

#[test]
fn test_parse_config() {
    let toml_str = r#"
[servers.staging]
host = "staging.example.com"
user = "deploy"
port = 2222
remote_path = "/var/www/app"
identity = "~/.ssh/id_staging"
auth = "key"

[servers.prod]
host = "prod.example.com"
user = "deploy"
remote_path = "/var/www/app"
auth = "agent"
"#;
    let config: ServerRegistry = toml::from_str(toml_str).unwrap();
    assert_eq!(config.servers.len(), 2);

    let staging = config.get("staging").unwrap();
    assert_eq!(staging.host, "staging.example.com");
    assert_eq!(staging.port, 2222);
    assert_eq!(staging.auth, AuthMethod::Key);
    assert_eq!(staging.identity.as_deref(), Some("~/.ssh/id_staging"));

    let prod = config.get("prod").unwrap();
    assert_eq!(prod.host, "prod.example.com");
    assert_eq!(prod.port, 22); // default
    assert_eq!(prod.auth, AuthMethod::Agent);
}

#[test]
fn test_default_auth_method() {
    let toml_str = r#"
[servers.dev]
host = "dev.local"
user = "user"
remote_path = "/home/user"
"#;
    let config: ServerRegistry = toml::from_str(toml_str).unwrap();
    let dev = config.get("dev").unwrap();
    assert_eq!(dev.auth, AuthMethod::Auto);
}

#[test]
fn test_roundtrip() {
    let mut config = ServerRegistry::default();
    config.insert(
        "test".to_string(),
        ServerEntry {
            host: "test.local".to_string(),
            user: "testuser".to_string(),
            port: 22,
            remote_path: "/home/test".to_string(),
            identity: None,
            auth: AuthMethod::Auto,
            metadata: None,
        },
    );

    let serialized = toml::to_string_pretty(&config).unwrap();
    let deserialized: ServerRegistry = toml::from_str(&serialized).unwrap();

    let entry = deserialized.get("test").unwrap();
    assert_eq!(entry.host, "test.local");
    assert_eq!(entry.user, "testuser");
}

#[test]
fn test_empty_config() {
    let config: ServerRegistry = toml::from_str("").unwrap();
    assert!(config.servers.is_empty());
}

#[test]
fn test_metadata_backward_compat() {
    // Existing config without metadata fields should parse fine
    let toml_str = r#"
[servers.staging]
host = "staging.example.com"
user = "deploy"
remote_path = "/var/www"
"#;
    let config: ServerRegistry = toml::from_str(toml_str).unwrap();
    let staging = config.get("staging").unwrap();
    assert!(staging.metadata.is_none());
}

#[test]
fn test_metadata_roundtrip() {
    let mut config = ServerRegistry::default();
    let mut entry = ServerEntry {
        host: "test.local".to_string(),
        user: "testuser".to_string(),
        port: 22,
        remote_path: "/home/test".to_string(),
        identity: None,
        auth: AuthMethod::Auto,
        metadata: None,
    };
    entry.metadata = Some(SystemMetadata {
        os: Some("linux".into()),
        distro: Some("Ubuntu 22.04".into()),
        arch: Some("x86_64".into()),
        shell: Some("/bin/bash".into()),
        package_manager: Some("apt".into()),
        collected_at: Some(1_700_000_000),
    });
    config.insert("test".to_string(), entry);

    let serialized = toml::to_string_pretty(&config).unwrap();
    let deserialized: ServerRegistry = toml::from_str(&serialized).unwrap();

    let meta = deserialized.get("test").unwrap().metadata.as_ref().unwrap();
    assert_eq!(meta.os.as_deref(), Some("linux"));
    assert_eq!(meta.distro.as_deref(), Some("Ubuntu 22.04"));
    assert_eq!(meta.arch.as_deref(), Some("x86_64"));
    assert_eq!(meta.shell.as_deref(), Some("/bin/bash"));
    assert_eq!(meta.package_manager.as_deref(), Some("apt"));
    assert_eq!(meta.collected_at, Some(1_700_000_000));
}
