use ssh_hub::tools::remote_bash::handler::detect_background_pattern;

// --- nohup detection ---

#[test]
fn detects_nohup_prefix() {
    assert_eq!(
        detect_background_pattern("nohup python3 train.py &"),
        Some("nohup")
    );
}

#[test]
fn detects_nohup_with_redirects() {
    assert_eq!(
        detect_background_pattern("nohup sh -c 'cmd' > /tmp/log 2>&1 < /dev/null & echo $!"),
        Some("nohup")
    );
}

#[test]
fn detects_nohup_mid_command() {
    assert_eq!(
        detect_background_pattern("cd /tmp && nohup python3 train.py &"),
        Some("nohup")
    );
}

// --- setsid detection ---

#[test]
fn detects_setsid_prefix() {
    assert_eq!(
        detect_background_pattern("setsid python3 train.py > /tmp/log 2>&1 &"),
        Some("setsid")
    );
}

#[test]
fn detects_setsid_mid_command() {
    assert_eq!(
        detect_background_pattern("cd /work && setsid sh -c 'train' &"),
        Some("setsid")
    );
}

// --- trailing & detection ---

#[test]
fn detects_trailing_ampersand() {
    assert_eq!(
        detect_background_pattern("python3 train.py > /tmp/log 2>&1 &"),
        Some("trailing &")
    );
}

#[test]
fn detects_ampersand_with_echo_pid() {
    assert_eq!(
        detect_background_pattern("sleep 60 & echo $!"),
        Some("trailing &")
    );
}

#[test]
fn detects_ampersand_with_disown() {
    assert_eq!(
        detect_background_pattern("cmd & disown"),
        Some("trailing &")
    );
}

// --- normal commands (no false positives) ---

#[test]
fn allows_simple_commands() {
    assert_eq!(detect_background_pattern("echo hello"), None);
    assert_eq!(detect_background_pattern("ls -la"), None);
    assert_eq!(detect_background_pattern("python3 train.py"), None);
}

#[test]
fn allows_double_ampersand() {
    assert_eq!(detect_background_pattern("ls -la && echo done"), None);
    assert_eq!(
        detect_background_pattern("cd /tmp && python3 train.py"),
        None
    );
}

#[test]
fn allows_nohup_in_non_command_position() {
    // Reading a file called nohup.out, grep for "nohup" in logs, etc.
    assert_eq!(detect_background_pattern("cat /tmp/nohup.out"), None);
    assert_eq!(detect_background_pattern("grep nohup logfile"), None);
}
