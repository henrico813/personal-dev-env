#![cfg(target_os = "linux")]

//! Live interoperability tests for Vibe, Docker, Pi, and providers.
//!
//! The ignored tests require Docker, network access, and their respective
//! credentials. `integration` exercises file authentication and links the host
//! Pi state; `integration-compatible` uses only the compatible provider
//! variables and an isolated home without host Pi state.
//!
//! The result's `model` field confirms what Vibe reports receiving. The
//! assistant `message_start` event comes from Pi and independently confirms
//! which provider and model Pi selected.

use std::{
    fs,
    os::unix::fs::symlink,
    path::{Path, PathBuf},
    process::Command,
};

const MODEL: &str = "gpt-5.6-luna";
const SELECTOR: &str = "openai-codex/gpt-5.6-luna";
const FILE_AUTH_VARS: [&str; 7] = [
    "ANTHROPIC_API_KEY",
    "OPENAI_API_KEY",
    "GEMINI_API_KEY",
    "DEEPSEEK_API_KEY",
    "AZURE_OPENAI_API_KEY",
    "AZURE_OPENAI_BASE_URL",
    "OPENCODE_API_KEY",
];

fn run_git(repo: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn link_pi_state(source_home: &Path, test_home: &Path) {
    let source_agent = source_home
        .join(".pi/agent")
        .canonicalize()
        .expect("find Pi agent directory");
    let auth_file = source_agent.join("auth.json");
    assert!(
        auth_file.is_file(),
        "integration requires {}",
        auth_file.display()
    );

    let test_pi = test_home.join(".pi");
    fs::create_dir_all(&test_pi).expect("create test Pi directory");
    symlink(source_agent, test_pi.join("agent")).expect("link Pi agent directory");
}

#[test]
#[ignore = "requires Docker, network, and Pi credentials"]
fn vibe_forwards_model_selector_to_pi() {
    // Arrange: create the committed repository Vibe needs for its worktree.
    let source_home = PathBuf::from(std::env::var_os("HOME").expect("HOME"));
    let temp = tempfile::tempdir().expect("tempdir");
    let test_home = temp.path().join("home");
    let repo = temp.path().join("repo");
    fs::create_dir_all(&test_home).expect("create test home");
    fs::create_dir_all(&repo).expect("create test repo");
    fs::write(
        test_home.join(".gitconfig"),
        "[user]\nname = Vibe Test\nemail = vibe@example.invalid\n",
    )
    .expect("write git config");
    link_pi_state(&source_home, &test_home);

    run_git(&repo, &["init"]);
    fs::write(repo.join("README.md"), "fixture\n").expect("write fixture");
    run_git(&repo, &["add", "README.md"]);
    run_git(
        &repo,
        &[
            "-c",
            "user.name=Vibe Test",
            "-c",
            "user.email=vibe@example.invalid",
            "commit",
            "-m",
            "fixture",
        ],
    );
    let prompt = temp.path().join("prompt.txt");
    fs::write(&prompt, "Inspect the repository and make no changes.\n").expect("write prompt");

    // Act: run the compiled Vibe binary with only Pi file authentication.
    // GNU timeout bounds failures involving Docker, Pi, or the provider.
    let mut command = Command::new("timeout");
    command
        .current_dir(&repo)
        .args([
            "--kill-after",
            "30",
            "600",
            env!("CARGO_BIN_EXE_vibe"),
            "run",
        ])
        .args(["--key", "pi-model-resolution"])
        .args(["--base", "HEAD"])
        .arg("--prompt-file")
        .arg(&prompt)
        .args(["--model", SELECTOR])
        .env("HOME", &test_home);
    for key in FILE_AUTH_VARS {
        command.env_remove(key);
    }
    let output = command.output().expect("run Vibe");

    assert!(
        matches!(output.status.code(), Some(0) | Some(1)),
        "Vibe exited with {}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let result: serde_json::Value =
        serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
            panic!(
                "parse Vibe result: {error}\nstdout: {}\nstderr: {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            )
        });

    // Assert: Vibe completed and reports the selector supplied by the caller.
    // A no-op exits with 1; a committed change exits with 0.
    assert!(
        matches!(result["status"].as_str(), Some("noop") | Some("completed")),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(result["model"], SELECTOR);

    // events.jsonl contains Pi's unmodified JSON stream. This event confirms
    // the provider and model Pi actually selected.
    let events_path = result["events_log_path"].as_str().expect("events log path");
    let events = fs::read_to_string(events_path).expect("read events");
    let selection = events
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find(|event| event["type"] == "message_start" && event["message"]["role"] == "assistant")
        .expect("assistant model selection event");
    assert_eq!(selection["message"]["model"], MODEL);
    assert_eq!(selection["message"]["provider"], "openai-codex");

    // The same call verifies that Pi can create its settings lock through
    // Vibe's writable Pi-state mount.
    let stderr_path = result["stderr_path"].as_str().expect("stderr path");
    let agent_stderr = fs::read_to_string(stderr_path).expect("read agent stderr");
    assert!(!agent_stderr.contains("Failed to acquire settings lock"));
    assert!(!agent_stderr.contains("settings.json.lock"));
}

#[test]
#[ignore = "requires Docker, network, and compatible endpoint"]
fn vibe_discovers_compatible_model() {
    let temp = tempfile::tempdir().expect("tempdir");
    let test_home = temp.path().join("home");
    let repo = temp.path().join("repo");
    fs::create_dir_all(&test_home).expect("create test home");
    fs::create_dir_all(&repo).expect("create test repo");
    fs::write(
        test_home.join(".gitconfig"),
        "[user]\nname = Vibe Test\nemail = vibe@example.invalid\n",
    )
    .expect("write git config");
    run_git(&repo, &["init"]);
    fs::write(repo.join("README.md"), "fixture\n").expect("write fixture");
    run_git(&repo, &["add", "README.md"]);
    run_git(
        &repo,
        &[
            "-c",
            "user.name=Vibe Test",
            "-c",
            "user.email=vibe@example.invalid",
            "commit",
            "-m",
            "fixture",
        ],
    );
    let prompt = temp.path().join("prompt.txt");
    fs::write(&prompt, "Inspect the repository and make no changes.\n").expect("write prompt");
    let model =
        std::env::var("OPENAI_COMPATIBLE_MODEL").expect("OPENAI_COMPATIBLE_MODEL is required");

    let mut command = Command::new("timeout");
    command
        .current_dir(&repo)
        .args([
            "--kill-after",
            "30",
            "600",
            env!("CARGO_BIN_EXE_vibe"),
            "run",
        ])
        .args(["--key", "pi-compatible-discovery"])
        .args(["--base", "HEAD"])
        .arg("--prompt-file")
        .arg(&prompt)
        .args(["--model", &format!("openai-compatible/{model}")])
        .env("HOME", &test_home);
    for key in FILE_AUTH_VARS {
        command.env_remove(key);
    }
    command
        .env(
            "OPENAI_COMPATIBLE_BASE_URL",
            std::env::var("OPENAI_COMPATIBLE_BASE_URL")
                .expect("OPENAI_COMPATIBLE_BASE_URL is required"),
        )
        .env(
            "OPENAI_COMPATIBLE_API_KEY",
            std::env::var("OPENAI_COMPATIBLE_API_KEY")
                .expect("OPENAI_COMPATIBLE_API_KEY is required"),
        );
    let output = command.output().expect("run Vibe");

    assert!(
        matches!(output.status.code(), Some(0) | Some(1)),
        "Vibe exited with {}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value =
        serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
            panic!(
                "parse Vibe result: {error}\nstdout: {}\nstderr: {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            )
        });
    assert!(matches!(
        result["status"].as_str(),
        Some("noop") | Some("completed")
    ));
    assert_eq!(result["model"], format!("openai-compatible/{model}"));

    let events_path = result["events_log_path"].as_str().expect("events log path");
    let events = fs::read_to_string(events_path).expect("read events");
    let selection = events
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find(|event| event["type"] == "message_start" && event["message"]["role"] == "assistant")
        .expect("assistant model selection event");
    assert_eq!(selection["message"]["model"], model);
    assert_eq!(selection["message"]["provider"], "openai-compatible");
}
