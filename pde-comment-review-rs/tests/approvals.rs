mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Barrier;
use std::thread;

use common::TempFolder;
use pde_comment_review::shared::ErrorCode;
use pde_comment_review::shared::approvals::{self, Approval, DecisionMatch};
use pde_comment_review::shared::comments::{Comment, CommentStyle};
use serde_json::{Value, json};

/// An approvals file as today's checker writes it, with a cut and an
/// avoided word.
const TODAYS_FILE: &str = r#"{
 "approved": [
  {
   "file": "review_store.go",
   "name": "SaveAnswer",
   "text": "// Saves the answer under the lock.\n"
  },
  {
   "file": "review_store.go",
   "name": "LoadAnswer",
   "text": ""
  },
  {
   "file": "review.py",
   "name": "Review.save",
   "text": "    \"\"\"Save the user\u2019s answer.\"\"\"\n"
  }
 ],
 "avoid": {
  "client": "say customer"
 }
}
"#;

fn run_git(dir: &Path, args: &[&str]) {
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

fn make_repo() -> TempFolder {
    let folder = TempFolder::new();
    run_git(folder.path(), &["init", "-b", "main"]);
    run_git(
        folder.path(),
        &["commit", "--allow-empty", "-m", "Start review"],
    );
    folder
}

fn approvals_file(repo: &Path) -> PathBuf {
    repo.join(".git/pde/code-comments.json")
}

fn write_approvals_file(repo: &Path, text: &str) {
    fs::create_dir_all(repo.join(".git/pde")).unwrap();
    fs::write(approvals_file(repo), text).unwrap();
}

fn read_approvals_file(repo: &Path) -> Value {
    serde_json::from_slice(&fs::read(approvals_file(repo)).unwrap()).unwrap()
}

fn approval(file: &str, name: &str, text: &str) -> Approval {
    Approval {
        file: PathBuf::from(file),
        name: name.to_owned(),
        text: text.to_owned(),
    }
}

/// Without it, saving new answers could lose the user's earlier answers or avoided words.
#[test]
fn add_keeps_old_entries_and_avoid_map() {
    let repo = make_repo();
    write_approvals_file(repo.path(), TODAYS_FILE);

    approvals::add(
        repo.path(),
        &[
            approval("review_store.go", "SaveAnswer", "// Saves the answer.\n"),
            approval("review.py", "Review.save", ""),
        ],
    )
    .unwrap();

    let mut expected: Value = serde_json::from_str(TODAYS_FILE).unwrap();
    let entries = expected["approved"].as_array_mut().unwrap();
    entries.push(
        json!({"file": "review_store.go", "name": "SaveAnswer", "text": "// Saves the answer.\n"}),
    );
    entries.push(json!({"file": "review.py", "name": "Review.save", "text": ""}));
    assert_eq!(read_approvals_file(repo.path()), expected);
}

/// Without it, a damaged or newer approvals file would be overwritten and the user's answers lost.
#[test]
fn add_refuses_bad_file_and_keeps_it() {
    let repo = make_repo();
    let cases = [
        (
            "broken-json",
            r#"{"approved": ["#,
            ErrorCode::SavedFileUnreadable,
        ),
        (
            "entry-without-text",
            r#"{"approved": [{"file": "review.py", "name": "Review.save"}]}"#,
            ErrorCode::SavedFileUnreadable,
        ),
        (
            "unknown-top-level-field",
            r#"{"approved": [], "avoid": {}, "rules": []}"#,
            ErrorCode::UnknownSavedField,
        ),
        (
            "unknown-entry-field",
            r#"{"approved": [{"file": "review.py", "name": "Review.save", "text": "", "at": "2026-10-31"}]}"#,
            ErrorCode::UnknownSavedField,
        ),
    ];

    let new_answer = [approval(
        "review_store.go",
        "SaveAnswer",
        "// Saves the answer.\n",
    )];

    for (case, text, code) in cases {
        write_approvals_file(repo.path(), text);

        let error = approvals::add(repo.path(), &new_answer).unwrap_err();

        assert_eq!(error.code, code, "{case}");
        let kept = fs::read_to_string(approvals_file(repo.path())).unwrap();
        assert_eq!(kept, text, "{case}");
    }
}

/// Without it, two worktrees saving at once could each overwrite the other's answers.
#[test]
fn concurrent_adds_keep_every_entry() {
    let repo = make_repo();
    let threads = 8;
    let adds_per_thread = 4;
    let start = Barrier::new(threads);

    thread::scope(|scope| {
        for thread_number in 0..threads {
            let start = &start;
            let dir = repo.path();
            scope.spawn(move || {
                start.wait();
                for add_number in 0..adds_per_thread {
                    let name = format!("Save{thread_number}x{add_number}");
                    approvals::add(dir, &[approval("review_store.go", &name, "// Saves.\n")])
                        .unwrap();
                }
            });
        }
    });

    let saved = read_approvals_file(repo.path());
    assert_eq!(
        saved["approved"].as_array().unwrap().len(),
        threads * adds_per_thread
    );
}

/// Without it, a keep in one worktree would not undo a cut made in another.
#[test]
fn worktrees_share_the_newest_answer() {
    let repo = make_repo();
    let second = TempFolder::new();
    let second_path = second.path().to_str().unwrap();
    run_git(
        repo.path(),
        &["worktree", "add", "-b", "second", second_path],
    );
    let text = "// Saves the answer.\n";

    approvals::add(
        repo.path(),
        &[approval("review_store.go", "SaveAnswer", "")],
    )
    .unwrap();
    approvals::add(
        second.path(),
        &[approval("review_store.go", "SaveAnswer", text)],
    )
    .unwrap();

    let saved = Comment {
        file: PathBuf::from("review_store.go"),
        name: Some("SaveAnswer".to_owned()),
        line: 1,
        end_line: 1,
        style: CommentStyle::Line,
        text: text.to_owned(),
        byte_range: 0..text.len(),
        can_cut: true,
    };
    let approvals = approvals::load(repo.path()).unwrap();
    assert_eq!(approvals.compare(&saved), DecisionMatch::Approved);
}
