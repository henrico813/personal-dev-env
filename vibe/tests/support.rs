use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

pub struct TestEnv {
    _temp: TempDir,
    pub repo: PathBuf,
    pub home: PathBuf,
    fake_bin: PathBuf,
    plan: PathBuf,
}

impl TestEnv {
    pub fn new() -> Self {
        let temp = tempfile::tempdir().expect("tempdir");
        let bare = temp.path().join("remote.git");
        let repo = temp.path().join("repo");
        let home = temp.path().join("home");
        let fake_bin = temp.path().join("fake-bin");
        let plan = temp
            .path()
            .join("PDEV-042 Add Vibe characterization tests.md");

        fs::create_dir_all(&home).expect("home dir");
        fs::create_dir_all(&fake_bin).expect("fake bin dir");
        fs::write(&plan, "# plan\n").expect("write plan");

        init_repo(temp.path(), &bare, &repo);
        install_planner(&home);
        install_docker(&fake_bin);

        Self {
            _temp: temp,
            repo,
            home,
            fake_bin,
            plan,
        }
    }

    pub fn run(&self, mode: &str) -> Output {
        self.run_with(mode, &[])
    }

    pub fn run_with(&self, mode: &str, extra_env: &[(&str, &str)]) -> Output {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_vibe"));
        let step = extra_env
            .iter()
            .find_map(|(key, value)| (*key == "VIBE_TEST_STEP").then_some(*value))
            .unwrap_or("1");
        cmd.current_dir(&self.repo)
            .env("HOME", &self.home)
            .env("PATH", self.path_env())
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("FAKE_DOCKER_MODE", mode)
            .args([
                "run",
                self.plan.to_str().unwrap(),
                "--step",
                step,
                "--model",
                "openai-codex/gpt-5.5",
            ]);

        for (key, value) in extra_env {
            cmd.env(key, value);
        }

        cmd.output().expect("run vibe")
    }

    pub fn set_identity(&self) {
        git(&self.repo, &["config", "user.name", "Vibe Test"]);
        git(&self.repo, &["config", "user.email", "vibe@example.com"]);
    }

    fn path_env(&self) -> String {
        format!("{}:/usr/bin:/bin", self.fake_bin.display())
    }
}

pub fn git(cwd: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .status()
        .expect("spawn git");
    assert!(status.success(), "git command failed: {:?}", args);
}

pub fn git_stdout(cwd: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .output()
        .expect("spawn git");
    assert!(output.status.success(), "git command failed: {:?}", args);
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn init_repo(root: &Path, bare: &Path, repo: &Path) {
    git_bootstrap(root, &["init", "--bare", bare.to_str().unwrap()]);
    git_bootstrap(root, &["init", repo.to_str().unwrap()]);
    git_bootstrap(repo, &["checkout", "-b", "main"]);
    fs::create_dir_all(repo.join("vibe/docker")).expect("docker dir");
    fs::create_dir_all(repo.join("vibe/hooks")).expect("hooks dir");
    fs::write(repo.join("vibe/docker/Dockerfile"), "FROM scratch\n").expect("dockerfile");
    fs::write(repo.join("README.md"), "seed\n").expect("seed readme");
    git_bootstrap(repo, &["add", "README.md", "vibe/docker/Dockerfile"]);
    git_bootstrap(repo, &["commit", "-m", "seed"]);
    git_bootstrap(repo, &["remote", "add", "github", bare.to_str().unwrap()]);
    git_bootstrap(repo, &["push", "-u", "github", "main"]);
}

fn install_planner(home: &Path) {
    let planner = home.join(".claude/bin/planner");
    fs::create_dir_all(planner.parent().unwrap()).expect("planner dir");
    fs::write(
        &planner,
        r#"#!/bin/sh
set -eu
if [ "${1:-}" = "inspect" ]; then
cat <<'JSON'
{
  "implementation": [
    {
      "title": "Create generated.txt",
      "summary": "Write one file in the worktree",
      "file_changes": [
        {
          "filename": "generated.txt",
          "explanation": "Test output file",
          "diff": "placeholder"
        }
      ]
    }
  ]
}
JSON
else
  echo "unsupported planner command" >&2
  exit 1
fi
"#,
    )
    .expect("write planner");
    make_executable(&planner);
}

fn install_docker(fake_bin: &Path) {
    let docker = fake_bin.join("docker");
    fs::write(
        &docker,
        r#"#!/bin/sh
set -eu
cmd="$1"
shift
case "$cmd" in
  version)
    if [ "${FAKE_DOCKER_VERSION_FAIL:-0}" = "1" ]; then
      exit 1
    fi
    exit 0
    ;;
  build)
    echo "fake docker build stdout"
    echo "fake docker build stderr" >&2
    exit 0
    ;;
  run)
    workdir=""
    artifacts=""
    while [ "$#" -gt 0 ]; do
      if [ "$1" = "-w" ]; then
        workdir="$2"
        shift 2
      elif [ "$1" = "-v" ]; then
        case "$2" in
          *:/artifacts)
            artifacts=${2%%:/artifacts}
            ;;
        esac
        shift 2
      else
        shift
      fi
    done
    cd "$workdir"
    echo '{"type":"agent","event":"start"}'
    echo "fake docker stderr" >&2
    case "${FAKE_DOCKER_MODE:-noop}" in
      noop)
        exit 0
        ;;
      write_success)
        printf 'generated by fake docker\n' > generated.txt
        exit 0
        ;;
      write_success_with_snapshots)
        printf 'generated by fake docker\n' > generated.txt
        mkdir -p "$artifacts"
        printf '{"sha":"abc"}\n{"sha":"def"}\n' > "$artifacts/snapshots.jsonl"
        exit 0
        ;;
      fail_clean)
        exit 9
        ;;
      write_fail)
        printf 'generated by fake docker\n' > generated.txt
        exit 9
        ;;
      *)
        echo "unsupported fake mode: ${FAKE_DOCKER_MODE:-}" >&2
        exit 98
        ;;
    esac
    ;;
  *)
    echo "unsupported docker command: $cmd" >&2
    exit 99
    ;;
esac
"#,
    )
    .expect("write docker");
    make_executable(&docker);
}

fn make_executable(path: &Path) {
    let mut perms = fs::metadata(path).expect("metadata").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).expect("chmod");
}

fn git_bootstrap(cwd: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .env("GIT_AUTHOR_NAME", "Bootstrap")
        .env("GIT_AUTHOR_EMAIL", "bootstrap@example.com")
        .env("GIT_COMMITTER_NAME", "Bootstrap")
        .env("GIT_COMMITTER_EMAIL", "bootstrap@example.com")
        .args(args)
        .status()
        .expect("spawn bootstrap git");
    assert!(status.success(), "bootstrap git command failed: {:?}", args);
}
