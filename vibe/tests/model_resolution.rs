#![cfg(unix)]

use std::{fs, process::Command};

fn fixture_home() -> tempfile::TempDir {
    let home = tempfile::tempdir().expect("fixture home");
    let agent = home.path().join(".pi/agent");
    fs::create_dir_all(&agent).expect("agent dir");
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pi-agent");
    fs::copy(fixture.join("models.json"), agent.join("models.json")).expect("models fixture");
    fs::copy(fixture.join("auth.json"), agent.join("auth.json")).expect("auth fixture");
    home
}

fn build_image() {
    let root = env!("CARGO_MANIFEST_DIR");
    let output = Command::new("docker")
        .args([
            "build",
            "-t",
            "vibe-pi:0.5.0",
            "-f",
            "docker/Dockerfile",
            ".",
        ])
        .current_dir(root)
        .output()
        .expect("docker build");
    assert!(
        output.status.success(),
        "docker build failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore = "requires Docker and network access"]
fn run_json_persists_requested_and_resolved_selectors() {
    build_image();
    let home = fixture_home();
    let prompt = tempfile::NamedTempFile::new().expect("prompt");
    fs::write(prompt.path(), "fixture run").expect("prompt contents");
    let output = Command::new(env!("CARGO_BIN_EXE_vibe"))
        .args([
            "run",
            "--key",
            "fixture-model-resolution",
            "--prompt-file",
            prompt.path().to_str().expect("prompt path"),
            "--model",
            "dynamic-model",
        ])
        .env("HOME", home.path())
        .output()
        .expect("run vibe run");
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("run JSON");
    assert_eq!(value["requested_model"], "dynamic-model");
    assert_eq!(value["model"], "openai-codex/dynamic-model");
    assert!(!String::from_utf8_lossy(&output.stdout).contains("fixture-key"));
}

#[test]
#[ignore = "requires Docker and network access"]
fn resolve_json_persists_requested_and_resolved_selectors() {
    build_image();
    let home = fixture_home();
    let output = Command::new(env!("CARGO_BIN_EXE_vibe"))
        .args(["resolve-model", "--model", "dynamic-model"])
        .env("HOME", home.path())
        .output()
        .expect("run vibe resolve-model");
    assert!(
        output.status.success(),
        "resolve failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("resolve JSON");
    assert_eq!(value["requested_model"], "dynamic-model");
    assert_eq!(value["model"], "openai-codex/dynamic-model");
    assert!(!String::from_utf8_lossy(&output.stdout).contains("fixture-key"));
}
