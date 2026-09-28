#![cfg(target_os = "linux")]

use std::{
    ffi::OsString,
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use tempfile::TempDir;

struct Fixture {
    _temp: TempDir,
    home: PathBuf,
    repo: PathBuf,
    worktree: PathBuf,
    prompt: PathBuf,
    bin: PathBuf,
}

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

fn write_executable(path: &Path, contents: &str) {
    fs::write(path, contents).expect("write executable");
    let mut permissions = fs::metadata(path)
        .expect("executable metadata")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("chmod executable");
}

fn setup_fixture(key: &str) -> Fixture {
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
    write_executable(
        &bin.join("docker"),
        "#!/usr/bin/env bash\nset -euo pipefail\ncase \"${1:-}\" in\n  version|build) exit 0 ;;\n  *) exit 98 ;;\nesac\n",
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

fn run_vibe(fixture: &Fixture, key: &str) -> Output {
    let path = std::env::join_paths(std::iter::once(fixture.bin.clone()).chain(
        std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_else(|| OsString::from("/usr/bin:/bin")),
        ),
    ))
    .expect("compose PATH");
    Command::new(env!("CARGO_BIN_EXE_vibe"))
        .current_dir(&fixture.repo)
        .args(["run", "--key", key, "--model", "openai-codex/test"])
        .arg("--prompt-file")
        .arg(&fixture.prompt)
        .env("HOME", &fixture.home)
        .env("PATH", path)
        .env("OPENAI_API_KEY", "test-key")
        .output()
        .expect("run Vibe")
}

fn parse_result(output: &Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "parse Vibe result: {error}\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

#[test]
fn invalid_repository_skills_return_setup_error() {
    let key = "invalid-repository-skills";
    let fixture = setup_fixture(key);
    fs::write(
        fixture.worktree.join(".agents/skills/local.txt"),
        "ignored\n",
    )
    .expect("write ignored skill file");

    let output = run_vibe(&fixture, key);
    let result = parse_result(&output);

    assert_eq!(output.status.code(), Some(7));
    assert_eq!(result["status"], "setup_error");
    assert!(result["run_id"].is_null());
    assert!(result["artifacts_dir"].is_null());
}

#[test]
fn changed_repository_skills_return_wrapper_failure() {
    let key = "changed-repository-skills";
    let fixture = setup_fixture(key);
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
    write_executable(
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
    let result = parse_result(&output);

    assert_eq!(output.status.code(), Some(6));
    assert_eq!(result["status"], "wrapper_failed");
    assert!(result["run_id"].is_string());
    assert!(result["artifacts_dir"].is_string());
    assert!(result["error_message"]
        .as_str()
        .expect("error message")
        .contains("must match HEAD"));
}
