use sha2::{Digest, Sha256};
use serde_json::{json, Value};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/merge")
        .join(name)
}

fn run_merge(arguments: &[PathBuf]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_surveil"))
        .arg("merge")
        .args(arguments)
        .output()
        .expect("run surveil merge")
}

fn temp_root(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir().join(format!("surveil-cli-{name}-{stamp}"))
}

fn run_managed_create(state_home: &Path, task: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_surveil"))
        .args(["new", "task", "--task", task])
        .env("XDG_STATE_HOME", state_home)
        .output()
        .expect("run managed create")
}

fn run_gather(repo: &Path, task_file: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_surveil"))
        .arg("gather")
        .arg("--repo")
        .arg(repo)
        .arg("--task-file")
        .arg(task_file)
        .output()
        .expect("run gather")
}

fn populate_task(path: &Path, name: &str) {
    fs::write(
        path,
        serde_json::to_vec_pretty(&json!({
            "summary": name,
            "explicit_files": ["src/lib.rs"],
            "search_areas": ["src"],
            "query": ["Where is session behavior implemented?"],
            "terms": ["session"]
        }))
        .expect("serialize task"),
    )
    .expect("populate task");
}

fn run_session(repo: &Path, root: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_surveil"))
        .args(["session", "run", "--repo"])
        .arg(repo)
        .arg("--root")
        .arg(root)
        .output()
        .expect("run session")
}

fn run_help(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_surveil"))
        .args(arguments)
        .arg("--help")
        .output()
        .expect("run help")
}

fn stdout_path(output: &Output) -> PathBuf {
    PathBuf::from(
        String::from_utf8(output.stdout.clone())
            .expect("UTF-8 path")
            .trim(),
    )
}

#[test]
fn merges_reports_deterministically() {
    let arguments = [
        fixture("architecture-report.json"),
        fixture("tests-report.json"),
    ];
    let first = run_merge(&arguments);
    let second = run_merge(&arguments);
    assert!(first.status.success());
    assert!(first.stderr.is_empty());
    assert!(second.status.success());
    assert!(second.stderr.is_empty());
    assert_eq!(first.stdout, second.stdout);

    let value: Value = serde_json::from_slice(&first.stdout).expect("parse evidence JSON");
    assert_eq!(value["schema_version"], "surveil.evidence.v2");
    assert_eq!(
        value["reports"],
        json!([
            {
                "task_name": "architecture",
                "summary": "architecture report"
            },
            {
                "task_name": "tests",
                "summary": "tests report"
            }
        ])
    );
    assert_eq!(value["findings"].as_array().expect("findings").len(), 1);
    assert_eq!(
        value["findings"][0],
        json!({
            "path": "surveil/src/cli.rs",
            "line": 143,
            "excerpt": "Command::Merge(args) => merge::run(&args.reports),",
            "occurrences": [
                {
                    "task_name": "architecture",
                    "query": "Where?",
                    "rank": 1,
                    "source": "lexical",
                    "matched_from": "merge",
                    "symbol_kind": "function",
                    "symbol_name": "run",
                    "symbol_start_line": 120,
                    "symbol_end_line": 146
                },
                {
                    "task_name": "tests",
                    "query": "How?",
                    "rank": 1,
                    "source": "explicit_file",
                    "matched_from": "merge",
                    "symbol_kind": "function",
                    "symbol_name": "run",
                    "symbol_start_line": 120,
                    "symbol_end_line": 146
                }
            ]
        })
    );
    assert_eq!(value["negative_evidence"][0]["text"], "no existing route");
    assert_eq!(value["blockers"][0]["text"], "strict input required");
    assert_eq!(
        value["open_questions"][0]["text"],
        "which caller adopts this?"
    );
    let output_text = String::from_utf8(first.stdout).expect("UTF-8 output");
    assert!(!output_text.contains(fixture("").to_string_lossy().as_ref()));
}

#[test]
fn merge_help_describes_reports() {
    let output = Command::new(env!("CARGO_BIN_EXE_surveil"))
        .args(["merge", "--help"])
        .output()
        .expect("run merge help");
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 help");
    assert!(stdout.contains("<TASK_REPORT>..."));
}

#[test]
fn rejects_duplicate_task_names() {
    let report = fixture("architecture-report.json");
    let output = run_merge(&[report.clone(), report]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("duplicate task name"));
}

#[test]
fn rejects_empty_task_name() {
    let output = run_merge(&[fixture("empty-task-name-report.json")]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("invalid task_name"));
}

#[test]
fn rejects_missing_task_name() {
    let output = run_merge(&[fixture("missing-task-name-report.json")]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("missing field `task_name`"));
}

#[test]
fn rejects_old_report_version() {
    let output = run_merge(&[fixture("old-schema-report.json")]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("expected surveil.v7, got surveil.v6"));
}

#[test]
fn rejects_unknown_report_field() {
    let output = run_merge(&[fixture("unknown-field-report.json")]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unknown-field-report.json"));
    assert!(stderr.contains("unexpected"));
}

#[test]
fn rejects_malformed_report_json() {
    let output = run_merge(&[fixture("malformed-report.json")]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("malformed-report.json"));
    assert!(stderr.contains("EOF while parsing a value"));
}

#[test]
fn managed_create_prints_xdg_root() {
    let state_home = temp_root("managed-create");
    let output = run_managed_create(&state_home, "architecture");
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let root = stdout_path(&output);
    assert!(root.starts_with(state_home.join("surveil/runs")));
    assert!(root.join(".surveil-managed").is_file());
    assert!(root.join("architecture/task.json").is_file());
    let _ = fs::remove_dir_all(state_home);
}

#[test]
fn managed_append_creates_task() {
    let state_home = temp_root("managed-append");
    let created = run_managed_create(&state_home, "architecture");
    let root = stdout_path(&created);
    let output = Command::new(env!("CARGO_BIN_EXE_surveil"))
        .args(["new", "task", "--root"])
        .arg(&root)
        .args(["--task", "tests verification"])
        .env("XDG_STATE_HOME", &state_home)
        .output()
        .expect("run managed append");
    assert!(output.status.success());
    assert_eq!(stdout_path(&output), root);
    assert!(root.join("tests verification/task.json").is_file());
    let _ = fs::remove_dir_all(state_home);
}

#[test]
fn managed_duplicate_fails_cleanly() {
    let state_home = temp_root("managed-duplicate");
    let created = run_managed_create(&state_home, "architecture");
    let root = stdout_path(&created);
    let output = Command::new(env!("CARGO_BIN_EXE_surveil"))
        .args(["new", "task", "--root"])
        .arg(&root)
        .args(["--task", "architecture"])
        .env("XDG_STATE_HOME", &state_home)
        .output()
        .expect("run duplicate append");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(root.join("architecture/task.json").is_file());
    let _ = fs::remove_dir_all(state_home);
}

#[test]
fn managed_rejects_foreign_root() {
    let state_home = temp_root("foreign-root");
    let foreign = state_home.join("foreign");
    fs::create_dir_all(&foreign).expect("create foreign root");
    let output = Command::new(env!("CARGO_BIN_EXE_surveil"))
        .args(["new", "task", "--root"])
        .arg(&foreign)
        .args(["--task", "architecture"])
        .env("XDG_STATE_HOME", &state_home)
        .output()
        .expect("run foreign append");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!foreign.join("architecture").exists());
    let _ = fs::remove_dir_all(state_home);
}

#[test]
fn explicit_mode_stays_unchanged() {
    let root = temp_root("explicit");
    let output_dir = root.join("nested/task");
    let output = Command::new(env!("CARGO_BIN_EXE_surveil"))
        .args(["new", "task"])
        .arg(&output_dir)
        .output()
        .expect("run explicit create");
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    assert!(output_dir.join("task.json").is_file());
    assert!(!root.join(".surveil-managed").exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn test_e2e_taskfile_generation() {
    let state_home = temp_root("task-e2e-state");
    let repo = temp_root("task-e2e-repo");
    fs::create_dir_all(repo.join("src")).expect("create repo source");
    fs::write(repo.join("src/lib.rs"), "fn main() {}\n").expect("write source");

    let created = run_managed_create(&state_home, "architecture");
    assert!(created.status.success());
    let root = stdout_path(&created);
    let task_file = root.join("architecture/task.json");
    let generated: Value =
        serde_json::from_slice(&fs::read(&task_file).expect("read generated task"))
            .expect("parse generated task");
    assert_eq!(
        generated,
        json!({
            "summary": "",
            "explicit_files": [],
            "search_areas": [],
            "query": [],
            "terms": []
        }),
    );

    fs::write(
        &task_file,
        serde_json::to_vec_pretty(&json!({
            "summary": "architecture",
            "explicit_files": ["src/lib.rs"],
            "search_areas": ["src"],
            "query": ["Where is the implementation?"],
            "terms": ["implementation"]
        }))
        .expect("serialize task"),
    )
    .expect("populate task");

    let gathered = run_gather(&repo, &task_file);
    assert!(gathered.status.success());
    assert!(gathered.stderr.is_empty());
    let context: Value = serde_json::from_slice(&gathered.stdout).expect("parse gather output");
    assert_eq!(context["task_name"], "architecture");
    assert_eq!(context["summary"], "architecture");
    assert_eq!(context["search_areas"], json!(["src"]));
    assert_eq!(context["explicit_files"][0]["found"], true);

    let _ = fs::remove_dir_all(state_home);
    let _ = fs::remove_dir_all(repo);
}

#[test]
fn root_help_describes_session() {
    let output = run_help(&[]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let help = String::from_utf8(output.stdout).expect("UTF-8 help");
    assert!(help.contains("Research repositories with structured tasks and evidence"));
    assert!(help.contains("Run all tasks in an existing managed root"));
    assert!(help.contains("-V, --version"));
}

#[test]
fn session_help_describes_run() {
    let output = run_help(&["session"]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let help = String::from_utf8(output.stdout).expect("UTF-8 help");
    assert!(help.contains("Usage: surveil session <COMMAND>"));
    assert!(help.contains("Run every populated task in a managed root"));
}

#[test]
fn session_run_help_explains_behavior() {
    let output = run_help(&["session", "run"]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let help = String::from_utf8(output.stdout).expect("UTF-8 help");
    for expected in [
        "Usage: surveil session run --repo <REPO> --root <MANAGED_ROOT>",
        "Repository root researched by every task",
        "does not create or rebuild an index",
        "processed sequentially in sorted task-name order",
        "All files are staged below the managed root",
        "Completed output is atomically published",
        ".surveil-session/tasks/<TASK>/context.json",
        ".surveil-session/receipt.json",
        "Failures before publication create no .surveil-session path",
        "surveil session run --repo /path/to/repo --root /path/to/managed-root",
    ] {
        assert!(help.contains(expected), "missing help text: {expected}");
    }
}

#[test]
fn session_writes_complete_artifacts() {
    let state_home = temp_root("session-state");
    let repo = temp_root("session-repo");
    fs::create_dir_all(repo.join("src")).expect("create source");
    fs::write(repo.join("src/lib.rs"), "fn session() {}\n").expect("write source");
    let created = run_managed_create(&state_home, "tests");
    let root = stdout_path(&created);
    let appended = Command::new(env!("CARGO_BIN_EXE_surveil"))
        .args(["new", "task", "--root"])
        .arg(&root)
        .args(["--task", "architecture"])
        .output()
        .expect("append task");
    assert!(appended.status.success());
    populate_task(&root.join("tests/task.json"), "tests");
    populate_task(&root.join("architecture/task.json"), "architecture");

    let output = run_session(&repo, &root);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let receipt_path = stdout_path(&output);
    assert_eq!(receipt_path, root.join(".surveil-session/receipt.json"));
    let receipt: Value = serde_json::from_slice(
        &fs::read(&receipt_path).expect("read receipt"),
    )
    .expect("parse receipt");
    assert_eq!(receipt["schema_version"], "surveil.session.v1");
    assert_eq!(receipt["status"], "complete");
    assert_eq!(receipt["task_names"], json!(["architecture", "tests"]));
    let artifacts = receipt["artifacts"].as_array().expect("artifacts");
    let identities = artifacts
        .iter()
        .map(|artifact| {
            json!({
                "kind": artifact["kind"].clone(),
                "task_name": artifact["task_name"].clone(),
                "path": artifact["path"].clone(),
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        identities,
        vec![
            json!({"kind": "context", "task_name": "architecture", "path": "tasks/architecture/context.json"}),
            json!({"kind": "trace", "task_name": "architecture", "path": "tasks/architecture/trace.json"}),
            json!({"kind": "report", "task_name": "architecture", "path": "tasks/architecture/report.json"}),
            json!({"kind": "context", "task_name": "tests", "path": "tasks/tests/context.json"}),
            json!({"kind": "trace", "task_name": "tests", "path": "tasks/tests/trace.json"}),
            json!({"kind": "report", "task_name": "tests", "path": "tasks/tests/report.json"}),
            json!({"kind": "evidence", "task_name": null, "path": "evidence.json"}),
        ],
    );
    for artifact in artifacts {
        let path = artifact["path"].as_str().expect("artifact path");
        assert!(Path::new(path)
            .components()
            .all(|part| matches!(part, Component::Normal(_))));
        let bytes = fs::read(root.join(".surveil-session").join(path))
            .expect("read artifact");
        assert_eq!(artifact["byte_len"], bytes.len() as u64);
        assert_eq!(
            artifact["sha256"],
            format!("{:x}", Sha256::digest(&bytes)),
        );
    }
    let evidence: Value = serde_json::from_slice(
        &fs::read(root.join(".surveil-session/evidence.json"))
            .expect("read evidence"),
    )
    .expect("parse evidence");
    assert_eq!(
        evidence["reports"]
            .as_array()
            .expect("reports")
            .iter()
            .map(|report| report["task_name"].as_str().expect("task name"))
            .collect::<Vec<_>>(),
        vec!["architecture", "tests"],
    );
    let _ = fs::remove_dir_all(state_home);
    let _ = fs::remove_dir_all(repo);
}

#[test]
fn session_rejects_completed_root() {
    let state_home = temp_root("session-rerun-state");
    let repo = temp_root("session-rerun-repo");
    fs::create_dir_all(repo.join("src")).expect("create source");
    fs::write(repo.join("src/lib.rs"), "fn session() {}\n").expect("write source");
    let created = run_managed_create(&state_home, "architecture");
    let root = stdout_path(&created);
    populate_task(&root.join("architecture/task.json"), "architecture");
    assert!(run_session(&repo, &root).status.success());

    let rerun = run_session(&repo, &root);
    assert!(!rerun.status.success());
    assert!(rerun.stdout.is_empty());
    assert!(String::from_utf8_lossy(&rerun.stderr)
        .contains("already contains .surveil-session"));
    let _ = fs::remove_dir_all(state_home);
    let _ = fs::remove_dir_all(repo);
}

#[test]
fn session_failure_publishes_nothing() {
    let state_home = temp_root("session-failure-state");
    let repo = temp_root("session-failure-repo");
    fs::create_dir_all(repo.join("src")).expect("create source");
    fs::write(repo.join("src/lib.rs"), "fn session() {}\n").expect("write source");
    let created = run_managed_create(&state_home, "a-valid");
    let root = stdout_path(&created);
    populate_task(&root.join("a-valid/task.json"), "a-valid");
    let appended = Command::new(env!("CARGO_BIN_EXE_surveil"))
        .args(["new", "task", "--root"])
        .arg(&root)
        .args(["--task", "z-invalid"])
        .output()
        .expect("append invalid task");
    assert!(appended.status.success());

    let output = run_session(&repo, &root);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!root.join(".surveil-session").exists());
    assert!(fs::read_dir(&root)
        .expect("read root")
        .all(|entry| !entry
            .expect("entry")
            .file_name()
            .to_string_lossy()
            .starts_with(".surveil-session-")));
    let _ = fs::remove_dir_all(state_home);
    let _ = fs::remove_dir_all(repo);
}

#[test]
fn session_allows_artifact_like_task_names() {
    for name in ["session", "evidence.json", "receipt.json"] {
        let state_home = temp_root(name);
        let repo = temp_root(&format!("{name}-repo"));
        fs::create_dir_all(repo.join("src")).expect("create source");
        fs::write(repo.join("src/lib.rs"), "fn session() {}\n").expect("write source");
        let root = stdout_path(&run_managed_create(&state_home, name));
        populate_task(&root.join(name).join("task.json"), name);
        assert!(run_session(&repo, &root).status.success(), "{name}");
        let _ = fs::remove_dir_all(state_home);
        let _ = fs::remove_dir_all(repo);
    }
}

#[test]
fn session_rejects_task_named_for_output_namespace() {
    let state_home = temp_root("session-reserved-state");
    let repo = temp_root("session-reserved-repo");
    fs::create_dir_all(&repo).expect("create repo");
    let root = stdout_path(&run_managed_create(&state_home, ".surveil-session"));
    populate_task(
        &root.join(".surveil-session/task.json"),
        ".surveil-session",
    );

    let output = run_session(&repo, &root);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("reserved"));
    let _ = fs::remove_dir_all(state_home);
    let _ = fs::remove_dir_all(repo);
}

#[cfg(unix)]
#[test]
fn session_rejects_symlinked_task_file() {
    use std::os::unix::fs::symlink;

    let state_home = temp_root("session-symlink-state");
    let repo = temp_root("session-symlink-repo");
    fs::create_dir_all(&repo).expect("create repo");
    let root = stdout_path(&run_managed_create(&state_home, "architecture"));
    let task_file = root.join("architecture/task.json");
    let target = root.join("outside-task.json");
    fs::rename(&task_file, &target).expect("move task file");
    symlink(&target, &task_file).expect("symlink task file");

    let output = run_session(&repo, &root);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!root.join(".surveil-session").exists());
    let _ = fs::remove_dir_all(state_home);
    let _ = fs::remove_dir_all(repo);
}

#[cfg(unix)]
#[test]
fn session_rejects_symlinked_task_directory() {
    use std::os::unix::fs::symlink;

    let state_home = temp_root("session-directory-symlink-state");
    let repo = temp_root("session-directory-symlink-repo");
    let target = temp_root("session-directory-symlink-target");
    fs::create_dir_all(repo.join("src")).expect("create source");
    fs::write(repo.join("src/lib.rs"), "fn session() {}\n").expect("write source");
    fs::create_dir_all(&target).expect("create target");
    populate_task(&target.join("task.json"), "linked");
    let root = stdout_path(&run_managed_create(&state_home, "architecture"));
    populate_task(&root.join("architecture/task.json"), "architecture");
    symlink(&target, root.join("linked")).expect("symlink task directory");

    let output = run_session(&repo, &root);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("managed root entries must not be symlinks"));
    assert!(!root.join(".surveil-session").exists());
    let _ = fs::remove_dir_all(state_home);
    let _ = fs::remove_dir_all(repo);
    let _ = fs::remove_dir_all(target);
}

#[cfg(unix)]
#[test]
fn session_rejects_non_utf8_root() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let state_home = temp_root("session-native-state")
        .join(OsString::from_vec(b"non-utf8-\xff".to_vec()));
    let repo = temp_root("session-native-repo");
    fs::create_dir_all(&repo).expect("create repo");
    let created = run_managed_create(&state_home, "architecture");
    assert!(created.status.success());
    let root = fs::read_dir(state_home.join("surveil/runs"))
        .expect("read runs")
        .next()
        .expect("managed root")
        .expect("read managed root")
        .path();
    populate_task(&root.join("architecture/task.json"), "architecture");

    let output = run_session(&repo, &root);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("must be UTF-8"));
    assert!(!root.join(".surveil-session").exists());
    let _ = fs::remove_dir_all(state_home);
    let _ = fs::remove_dir_all(repo);
}

#[cfg(target_os = "linux")]
#[test]
fn stdout_failure_preserves_published_session() {
    use std::process::Stdio;

    let state_home = temp_root("session-stdout-state");
    let repo = temp_root("session-stdout-repo");
    fs::create_dir_all(repo.join("src")).expect("create source");
    fs::write(repo.join("src/lib.rs"), "fn session() {}\n").expect("write source");
    let root = stdout_path(&run_managed_create(&state_home, "architecture"));
    populate_task(&root.join("architecture/task.json"), "architecture");
    let full = fs::OpenOptions::new()
        .write(true)
        .open("/dev/full")
        .expect("open /dev/full");

    let output = Command::new(env!("CARGO_BIN_EXE_surveil"))
        .args(["session", "run", "--repo"])
        .arg(&repo)
        .arg("--root")
        .arg(&root)
        .stdout(Stdio::from(full))
        .output()
        .expect("run session");
    assert!(!output.status.success());
    assert!(root.join(".surveil-session/receipt.json").is_file());
    let _ = fs::remove_dir_all(state_home);
    let _ = fs::remove_dir_all(repo);
}
