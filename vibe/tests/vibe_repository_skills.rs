#![cfg(target_os = "linux")]

mod common;

use std::{ffi::OsString, fs, process::Command};

#[test]
fn invalid_repository_skills_return_setup_error() {
    let key = "invalid-repository-skills";
    let fixture = common::setup_fixture(key);
    fs::write(
        fixture.worktree.join(".agents/skills/local.txt"),
        "ignored\n",
    )
    .expect("write ignored skill file");

    let output = common::run_vibe(&fixture, key, "noop");
    let result = common::parse_result(&output);

    assert_eq!(output.status.code(), Some(7));
    assert_eq!(result["status"], "setup_error");
    assert!(result["run_id"].is_null());
    assert!(result["artifacts_dir"].is_null());
}

#[test]
fn changed_repository_skills_return_wrapper_failure() {
    let key = "changed-repository-skills";
    let fixture = common::setup_fixture(key);
    let real_git = Command::new("sh")
        .args(["-c", "command -v git"])
        .output()
        .expect("find git");
    assert!(real_git.status.success());
    let real_git = String::from_utf8(real_git.stdout)
        .expect("git path is UTF-8")
        .trim()
        .to_string();
    let count = fixture.home.join("git-status-count");
    common::write_executable(
        &fixture.bin.join("git"),
        r#"#!/usr/bin/env bash
set -euo pipefail
repository_status=false
for arg in "$@"; do
  if [[ "${arg}" == "--ignored=matching" ]]; then
    repository_status=true
  fi
done
if [[ "${repository_status}" == true ]]; then
  current=0
  if [[ -f "${VIBE_TEST_STATUS_COUNT}" ]]; then
    current="$(cat "${VIBE_TEST_STATUS_COUNT}")"
  fi
  current=$((current + 1))
  printf '%s' "${current}" > "${VIBE_TEST_STATUS_COUNT}"
  if [[ "${current}" -eq 2 ]]; then
    printf 'changed\n' > "${VIBE_TEST_SKILLS_DIR}/local.txt"
  fi
fi
exec "${REAL_GIT}" "$@"
"#,
    );
    let path = std::env::join_paths(std::iter::once(fixture.bin.clone()).chain(
        std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_else(|| OsString::from("/usr/bin:/bin")),
        ),
    ))
    .expect("compose PATH");

    let output = Command::new(env!("CARGO_BIN_EXE_vibe"))
        .current_dir(&fixture.repo)
        .args(["run", "--key", key, "--model", "openai-codex/test"])
        .arg("--prompt-file")
        .arg(&fixture.prompt)
        .env("HOME", &fixture.home)
        .env("PATH", path)
        .env("OPENAI_API_KEY", "test-key")
        .env("REAL_GIT", real_git)
        .env("VIBE_TEST_STATUS_COUNT", &count)
        .env(
            "VIBE_TEST_SKILLS_DIR",
            fixture.worktree.join(".agents/skills"),
        )
        .output()
        .expect("run Vibe");
    let result = common::parse_result(&output);

    assert_eq!(output.status.code(), Some(6));
    assert_eq!(result["status"], "wrapper_failed");
    assert!(result["run_id"].is_string());
    assert!(result["artifacts_dir"].is_string());
    assert!(result["error_message"]
        .as_str()
        .expect("error message")
        .contains("must match HEAD"));
}
