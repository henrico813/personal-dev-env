use std::{
    ffi::OsString,
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use tempfile::TempDir;

pub struct Fixture {
    pub _temp: TempDir,
    pub home: PathBuf,
    pub repo: PathBuf,
    pub worktree: PathBuf,
    pub prompt: PathBuf,
    pub bin: PathBuf,
}

pub fn run_git(repo: &Path, args: &[&str]) {
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

pub fn write_executable(path: &Path, contents: &str) {
    fs::write(path, contents).expect("write executable");
    let mut permissions = fs::metadata(path)
        .expect("executable metadata")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("chmod executable");
}

pub fn setup_fixture(key: &str) -> Fixture {
    let temp = tempfile::tempdir().expect("tempdir");
    let home = temp.path().join("home");
    let repo = temp.path().join("repo");
    let bin = temp.path().join("bin");
    fs::create_dir_all(&home).expect("create home");
    fs::create_dir_all(&repo).expect("create repo");
    fs::create_dir_all(&bin).expect("create bin");
    run_git(&repo, &["init"]);
    run_git(&repo, &["config", "user.name", "Vibe Test"]);
    run_git(&repo, &["config", "user.email", "vibe@example.invalid"]);
    let skill = repo.join(".agents/skills/reviewed/SKILL.md");
    fs::create_dir_all(skill.parent().expect("skill parent")).expect("create skills");
    fs::write(&skill, "reviewed\n").expect("write skill");
    fs::write(
        repo.join(".gitignore"),
        "/worktrees/\n.agents/skills/local.txt\n",
    )
    .expect("write gitignore");
    run_git(&repo, &["add", "."]);
    run_git(
        &repo,
        &["-c", "core.hooksPath=/dev/null", "commit", "-m", "fixture"],
    );
    let worktree = repo.join("worktrees").join(key);
    run_git(
        &repo,
        &[
            "worktree",
            "add",
            "-b",
            &format!("vibe/{key}"),
            worktree.to_str().expect("worktree path"),
            "HEAD",
        ],
    );
    let prompt = temp.path().join("prompt.txt");
    fs::write(&prompt, "Inspect the repository and make no changes.\n").expect("write prompt");
    // The fake finds the worktree and artifacts through `-v` mounts, writes the
    // snapshots file consumed by the snapshot stage, removes it for
    // `snapshot_failed`, and supports noop, completed, agent_failed, and
    // snapshot_failed through VIBE_FAKE_MODE.
    write_executable(
        &bin.join("docker"),
        r##"#!/usr/bin/env bash
set -euo pipefail
mode="${VIBE_FAKE_MODE:-noop}"
command="${1:-}"
if [[ "$command" == "version" || "$command" == "build" ]]; then exit 0; fi
if [[ "$command" != "run" ]]; then exit 98; fi
worktree=""
artifacts=""
while [[ "$#" -gt 0 ]]; do
  if [[ "${1:-}" == "-v" ]]; then
    mount="${2:-}"
    case "$mount" in
      */worktrees/*:*) worktree="${mount%%:*}" ;;
      *:/artifacts) artifacts="${mount%:/artifacts}" ;;
    esac
    shift 2
  else
    shift
  fi
done
if [[ "$mode" == "snapshot_failed" ]]; then
  rm -f "$artifacts/snapshots.jsonl"
  exit 0
fi
printf '%s\n' '{"sha":"snapshot-sha"}' > "$artifacts/snapshots.jsonl"
case "$mode" in
  completed) printf 'changed\n' > "$worktree/agent-output.txt"; exit 0 ;;
  agent_failed) exit 3 ;;
  noop) exit 0 ;;
  *) exit 98 ;;
esac
"##,
    );
    Fixture {
        _temp: temp,
        home,
        repo,
        worktree,
        prompt,
        bin,
    }
}

pub fn path_with_bin(fixture: &Fixture) -> OsString {
    std::env::join_paths(
        std::iter::once(fixture.bin.clone()).chain(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_else(|| OsString::from("/usr/bin:/bin")),
        )),
    )
    .expect("compose PATH")
}

pub fn run_vibe(fixture: &Fixture, key: &str, mode: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_vibe"))
        .current_dir(&fixture.repo)
        .args(["run", "--key", key, "--model", "openai-codex/test"])
        .arg("--prompt-file")
        .arg(&fixture.prompt)
        .env("HOME", &fixture.home)
        .env("PATH", path_with_bin(fixture))
        .env("OPENAI_API_KEY", "test-key")
        .env("VIBE_FAKE_MODE", mode)
        .output()
        .expect("run Vibe")
}

pub fn parse_result(output: &Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "parse Vibe result: {error}\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}
