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

fn fake_docker(bin: &std::path::Path, marker: &std::path::Path) {
    let docker = bin.join("docker");
    fs::write(
        &docker,
        format!(
            "#!/bin/sh\n/usr/bin/touch {}\nif [ \"$1\" = \"run\" ]; then\n  printf '%s\\n' 'openai-codex dynamic-model' 'vibe-fixture dynamic-model'\nfi\n",
            marker.display()
        ),
    )
    .expect("write docker");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        fs::set_permissions(&docker, fs::Permissions::from_mode(0o755)).expect("chmod docker");
    }
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

#[test]
fn resolve_model_honors_explicit_provider_without_docker() {
    let home = fixture_home();
    let bin = tempfile::tempdir().expect("bin");
    let marker = bin.path().join("docker-ran");
    fake_docker(bin.path(), &marker);
    let path = format!(
        "{}:{}",
        bin.path().display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let output = Command::new(env!("CARGO_BIN_EXE_vibe"))
        .args([
            "resolve-model",
            "--model",
            "dynamic-model",
            "--provider",
            "vibe-fixture",
        ])
        .env("HOME", home.path())
        .env("PATH", path)
        .output()
        .expect("run vibe resolve-model");

    assert!(output.status.success());
    assert!(marker.exists());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("resolve JSON");
    assert_eq!(value["requested_model"], "dynamic-model");
    assert_eq!(value["model"], "vibe-fixture/dynamic-model");
}

#[test]
fn run_auth_precedes_docker_discovery() {
    let home = tempfile::tempdir().expect("home");
    let bin = tempfile::tempdir().expect("bin");
    let marker = bin.path().join("docker-ran");
    let docker = bin.path().join("docker");
    fs::write(
        &docker,
        format!("#!/bin/sh\n/usr/bin/touch {}\n", marker.display()),
    )
    .expect("write docker");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        fs::set_permissions(&docker, fs::Permissions::from_mode(0o755)).expect("chmod docker");
    }
    let prompt = tempfile::NamedTempFile::new().expect("prompt");
    let path = format!(
        "{}:{}",
        bin.path().display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let output = Command::new(env!("CARGO_BIN_EXE_vibe"))
        .args([
            "run",
            "--key",
            "fixture-auth-order",
            "--prompt-file",
            prompt.path().to_str().expect("prompt path"),
            "--model",
            "dynamic-model",
        ])
        .env("HOME", home.path())
        .env("PATH", path)
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("OPENAI_API_KEY")
        .env_remove("GEMINI_API_KEY")
        .env_remove("DEEPSEEK_API_KEY")
        .env_remove("AZURE_OPENAI_API_KEY")
        .env_remove("AZURE_OPENAI_BASE_URL")
        .output()
        .expect("run vibe");

    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("run JSON");

    assert_eq!(value["status"], "setup_error");
    assert_eq!(
        value["error_message"],
        "vibe requires provider auth via env vars or ~/.pi/agent/auth.json"
    );
    assert!(!marker.exists());
}

#[test]
fn prompt_validation_precedes_model_discovery() {
    let home = tempfile::tempdir().expect("home");
    let bin = tempfile::tempdir().expect("bin");
    let marker = bin.path().join("docker-ran");
    fake_docker(bin.path(), &marker);
    let prompt = tempfile::NamedTempFile::new().expect("prompt");
    fs::write(prompt.path(), [0xff_u8, 0xfe_u8]).expect("write invalid prompt");
    let path = format!(
        "{}:{}",
        bin.path().display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let output = Command::new(env!("CARGO_BIN_EXE_vibe"))
        .args([
            "run",
            "--key",
            "fixture-prompt-order",
            "--prompt-file",
            prompt.path().to_str().expect("prompt path"),
            "--model",
            "dynamic-model",
        ])
        .env("HOME", home.path())
        .env("PATH", path)
        .output()
        .expect("run vibe");

    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("run JSON");

    assert_eq!(value["status"], "setup_error");
    assert!(value["error_message"]
        .as_str()
        .expect("error message")
        .starts_with("read prompt file as UTF-8:"));
    assert!(!marker.exists());
}
