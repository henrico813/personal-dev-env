// Each test file uses only some of these helpers.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use pde_comment_review::shared::approvals::{self, Approval};
use pde_comment_review::shared::git::resolve_commit;
use serde_json::{Value, json};

/// Names the Go file most tests change.
pub const STORE: &str = "review_store.go";
/// Holds the comment text the user approved.
pub const APPROVED: &str = "// Saves the answer under the lock.\n";
/// Holds an agent's new wording of the approved comment.
pub const REWORDED: &str = "// Writes the answer while holding the lock.\n";

/// Makes an empty temporary folder and deletes it when dropped.
pub struct TempFolder(PathBuf);

impl TempFolder {
    pub fn new() -> Self {
        let output = Command::new("mktemp").arg("-d").output().unwrap();
        assert!(output.status.success());
        let path = String::from_utf8(output.stdout).unwrap();
        Self(PathBuf::from(path.trim_end_matches('\n')))
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempFolder {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Runs Git without the test machine's own settings, so they cannot change the result.
pub fn run_git(dir: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_AUTHOR_NAME", "Review Test")
        .env("GIT_AUTHOR_EMAIL", "review@example.invalid")
        .env("GIT_COMMITTER_NAME", "Review Test")
        .env("GIT_COMMITTER_EMAIL", "review@example.invalid")
        .output()
        .unwrap();
    assert!(output.status.success(), "{:?}: {:?}", args, output);
}

/// Makes main hold one Go file with one comment, then starts a branch from it.
pub fn make_repo() -> TempFolder {
    let folder = TempFolder::new();
    let dir = folder.path();
    run_git(dir, &["init", "-b", "main"]);
    write_store(dir, APPROVED, "");
    commit_all(dir);
    run_git(dir, &["checkout", "-b", "review-comments"]);
    folder
}

/// Returns Go source with `comment` above a `SaveAnswer` function whose body is `body`.
pub fn store_source(comment: &str, body: &str) -> String {
    format!("package review\n\n{comment}func SaveAnswer() {{{body}}}\n")
}

/// Writes `store_source(comment, body)` to the store file on disk.
pub fn write_store(dir: &Path, comment: &str, body: &str) {
    fs::write(dir.join(STORE), store_source(comment, body)).unwrap();
}

/// Commits every change, or makes an empty commit when nothing changed.
pub fn commit_all(dir: &Path) {
    run_git(dir, &["add", "--all"]);
    run_git(dir, &["commit", "--allow-empty", "-m", "Change the store"]);
}

/// Saves the user's approval of `text` for `name`, or a cut when `text` is empty.
pub fn add_decision(dir: &Path, file: &str, name: &str, text: &str) {
    let decision = Approval {
        file: PathBuf::from(file),
        name: name.to_owned(),
        text: text.to_owned(),
    };
    approvals::add(dir, &[decision]).unwrap();
}

/// Saves a Review Session in `status` that sent each of `lists` from HEAD, with `feedback` by request ID.
pub fn save_review(dir: &Path, status: &str, lists: &[Value], feedback: Value) {
    let base = resolve_commit(dir, "HEAD").unwrap();
    let lists: Vec<Value> = lists
        .iter()
        .map(|requests| json!({"base": base.as_str(), "requests": requests}))
        .collect();
    let review = json!({
        "status": status,
        "origin": {"repo": "personal-dev-env", "branch": "review-comments", "agent_session": "ses_123"},
        "lists": lists,
        "feedback": feedback,
    });
    fs::create_dir_all(dir.join(".git/pde")).unwrap();
    fs::write(dir.join(".git/pde/review.json"), review.to_string()).unwrap();
}

/// Returns a sent Feedback Request for the store file, with reason `new` and a keep Agent Suggestion.
pub fn request_json(id: u32, name: Option<&str>, text: &str, line: usize) -> Value {
    json!({
        "id": id,
        "file": STORE,
        "name": name,
        "reason": "new",
        "text": text,
        "can_reword": true,
        "line": line,
        "suggestion": {"kind": "keep", "reasoning": "Says what it does.", "also_at": []},
    })
}
