use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("surveil-cli-{name}-{stamp}"));
    fs::create_dir_all(&path).expect("create temp dir");
    path
}

fn write_file(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent dirs");
    }
    fs::write(path, contents).expect("write file");
}

#[test]
fn research_cli_ranked_run() {
    let temp = temp_dir("research");
    let repo = temp.join("repo");
    fs::create_dir_all(&repo).expect("create repo");
    write_file(
        &repo.join("docs/guide.md"),
        "attach attach attach attach\nattach attach attach attach\n",
    );
    write_file(
        &repo.join("src/lib.rs"),
        "fn attach_handler() {\n    // attach handler\n}\n",
    );

    let task_file = temp.join("task.md");
    write_file(
        &task_file,
        "# Task\n\n## Summary\nsummary\n\n## Explicit Files\n\n## Search Areas\n- docs/\n- src/\n\n## Query\n- Where should attach handler live?\n\n## Terms\n- attach\n- handler\n",
    );

    let gather = Command::new(env!("CARGO_BIN_EXE_surveil"))
        .args(["gather", "--repo"])
        .arg(&repo)
        .args(["--task-file"])
        .arg(&task_file)
        .output()
        .expect("run gather");
    assert!(
        gather.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&gather.stderr)
    );

    let context_path = temp.join("context.json");
    fs::write(&context_path, &gather.stdout).expect("write context");

    let index = Command::new(env!("CARGO_BIN_EXE_surveil"))
        .args(["index", "--repo"])
        .arg(&repo)
        .output()
        .expect("run index");
    assert!(
        index.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&index.stderr)
    );

    let trace_path = temp.join("trace.json");
    let research = Command::new(env!("CARGO_BIN_EXE_surveil"))
        .args(["research", "--context"])
        .arg(&context_path)
        .args(["--trace-out"])
        .arg(&trace_path)
        .output()
        .expect("run research");
    assert!(
        research.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&research.stderr)
    );

    let report: Value = serde_json::from_slice(&research.stdout).expect("parse report");
    let trace: Value = serde_json::from_slice(&fs::read(&trace_path).expect("read trace"))
        .expect("parse trace");

    assert_eq!(report["result"][0]["findings"][0]["path"], "src/lib.rs");
    assert_eq!(report["result"][0]["findings"][0]["source"], "lexical");
    assert_eq!(trace["schema_version"], report["schema_version"]);

    let _ = fs::remove_dir_all(temp);
}
