use std::process::Command;

#[test]
fn prints_normalized_retry_count() {
    let output = Command::new(env!("CARGO_BIN_EXE_retry-count"))
        .arg(" 3 ")
        .output()
        .expect("the test binary can be started");

    assert!(output.status.success());
    assert_eq!(output.stdout, b"3\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn invalid_count_returns_failure() {
    let output = Command::new(env!("CARGO_BIN_EXE_retry-count"))
        .arg("three")
        .output()
        .expect("the test binary can be started");

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty());
}

#[test]
fn missing_count_shows_usage() {
    let output = Command::new(env!("CARGO_BIN_EXE_retry-count"))
        .output()
        .expect("the test binary can be started");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("usage is UTF-8");
    assert!(stderr.contains("usage: retry-count COUNT"));
}
