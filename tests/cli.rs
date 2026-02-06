use ssh_hub::cli::parse_connection_string;

#[test]
fn test_parse_simple_connection() {
    let info = parse_connection_string("user@host:/path/to/project", None).unwrap();
    assert_eq!(info.user, "user");
    assert_eq!(info.host, "host");
    assert_eq!(info.port, 22);
    assert_eq!(info.remote_path, "/path/to/project");
}

#[test]
fn test_parse_connection_with_port() {
    let info =
        parse_connection_string("deploy@staging.example.com:2222:/var/www/app", None).unwrap();
    assert_eq!(info.user, "deploy");
    assert_eq!(info.host, "staging.example.com");
    assert_eq!(info.port, 2222);
    assert_eq!(info.remote_path, "/var/www/app");
}

#[test]
fn test_port_override() {
    let info = parse_connection_string("user@host:2222:/path", Some(3333)).unwrap();
    assert_eq!(info.port, 3333);
}

#[test]
fn test_invalid_no_user() {
    assert!(parse_connection_string("host:/path", None).is_err());
}

#[test]
fn test_no_path_defaults_to_home() {
    let info = parse_connection_string("user@host", None).unwrap();
    assert_eq!(info.user, "user");
    assert_eq!(info.host, "host");
    assert_eq!(info.port, 22);
    assert_eq!(info.remote_path, "~");
}

#[test]
fn test_port_no_path() {
    let info = parse_connection_string("user@host:2222", None).unwrap();
    assert_eq!(info.user, "user");
    assert_eq!(info.host, "host");
    assert_eq!(info.port, 2222);
    assert_eq!(info.remote_path, "~");
}

#[test]
fn test_ip_address_no_path() {
    let info = parse_connection_string("vicente@34.41.145.215", None).unwrap();
    assert_eq!(info.user, "vicente");
    assert_eq!(info.host, "34.41.145.215");
    assert_eq!(info.port, 22);
    assert_eq!(info.remote_path, "~");
}

#[test]
fn test_invalid_not_a_port_or_path() {
    assert!(parse_connection_string("user@host:notaport", None).is_err());
}
