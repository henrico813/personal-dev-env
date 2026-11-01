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

/// A file of decisions the current Python checker already wrote on a user's
/// machine, with a cut comment and a word the user asked agents to avoid.
const FILE_FROM_PYTHON_CHECKER: &str = r#"{
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

/// Runs Git without the test machine's own settings, so they cannot change the result.
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

/// Makes a new Git repository with one commit, standing in for the user's checkout.
fn make_repo() -> TempFolder {
    let folder = TempFolder::new();
    run_git(folder.path(), &["init", "-b", "main"]);
    run_git(
        folder.path(),
        &["commit", "--allow-empty", "-m", "Start review"],
    );
    folder
}

/// Returns where the user's decisions live, in the Git folder all worktrees share.
fn approvals_file(repo: &Path) -> PathBuf {
    repo.join(".git/pde/code-comments.json")
}

/// Puts a file of decisions in place, as if an earlier review had added them.
fn write_approvals_file(repo: &Path, text: &str) {
    fs::create_dir_all(repo.join(".git/pde")).unwrap();
    fs::write(approvals_file(repo), text).unwrap();
}

/// Reads the stored decisions back as JSON, so a test can compare their content.
fn read_approvals_file(repo: &Path) -> Value {
    serde_json::from_slice(&fs::read(approvals_file(repo)).unwrap()).unwrap()
}

/// Builds one decision the user made, with an empty text for a cut.
fn approval(file: &str, name: &str, text: &str) -> Approval {
    Approval {
        file: PathBuf::from(file),
        name: name.to_owned(),
        text: text.to_owned(),
    }
}

/// Adding new decisions must keep every decision and avoided word already in the file.
/// Otherwise the user would be asked again about decided comments, and agents could use words the user banned.
#[test]
fn add_keeps_earlier_decisions_and_avoided_words() {
    // Start from a file the Python checker wrote.
    let repo = make_repo();
    write_approvals_file(repo.path(), FILE_FROM_PYTHON_CHECKER);

    // Add a new approved text for one comment and a cut for another.
    approvals::add(
        repo.path(),
        &[
            approval("review_store.go", "SaveAnswer", "// Saves the answer.\n"),
            approval("review.py", "Review.save", ""),
        ],
    )
    .unwrap();

    // The file holds the old content unchanged, with the new decisions after it.
    let mut expected: Value = serde_json::from_str(FILE_FROM_PYTHON_CHECKER).unwrap();
    let decisions = expected["approved"].as_array_mut().unwrap();
    decisions.push(
        json!({"file": "review_store.go", "name": "SaveAnswer", "text": "// Saves the answer.\n"}),
    );
    decisions.push(json!({"file": "review.py", "name": "Review.save", "text": ""}));
    assert_eq!(read_approvals_file(repo.path()), expected);
}

/// Adding must stop and leave the file alone when it is damaged or came from a newer version.
/// Writing over it would throw away the user's decisions or the newer version's data.
#[test]
fn add_leaves_a_damaged_or_newer_file_untouched() {
    let repo = make_repo();
    let cases = [
        (
            "broken-json",
            r#"{"approved": ["#,
            ErrorCode::SavedFileUnreadable,
        ),
        (
            "decision-without-text",
            r#"{"approved": [{"file": "review.py", "name": "Review.save"}]}"#,
            ErrorCode::SavedFileUnreadable,
        ),
        (
            "unknown-top-level-field",
            r#"{"approved": [], "avoid": {}, "rules": []}"#,
            ErrorCode::UnknownSavedField,
        ),
        (
            "unknown-decision-field",
            r#"{"approved": [{"file": "review.py", "name": "Review.save", "text": "", "at": "2026-10-31"}]}"#,
            ErrorCode::UnknownSavedField,
        ),
    ];

    let new_decision = [approval(
        "review_store.go",
        "SaveAnswer",
        "// Saves the answer.\n",
    )];

    for (case, text, code) in cases {
        write_approvals_file(repo.path(), text);

        let error = approvals::add(repo.path(), &new_decision).unwrap_err();

        // The add fails with the right reason, and the file keeps every byte.
        assert_eq!(error.code, code, "{case}");
        let unchanged = fs::read_to_string(approvals_file(repo.path())).unwrap();
        assert_eq!(unchanged, text, "{case}");
    }
}

/// Two reviews can add decisions at the same moment, such as one in each worktree.
/// No add may overwrite another, or the user would lose decisions and be asked again.
#[test]
fn simultaneous_adds_lose_no_decisions() {
    let repo = make_repo();
    let threads = 8;
    let adds_per_thread = 4;
    let start = Barrier::new(threads);

    // Start every thread at once, and have each add a few distinct decisions.
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

    // Every decision from every thread is in the file.
    let saved = read_approvals_file(repo.path());
    assert_eq!(
        saved["approved"].as_array().unwrap().len(),
        threads * adds_per_thread
    );
}

/// All worktrees of a clone must share one set of decisions, with the newest decision winning.
/// Otherwise approving a comment in one worktree would not undo a cut made in another.
#[test]
fn every_worktree_sees_the_newest_decision() {
    // Make a second worktree of the same clone.
    let repo = make_repo();
    let second = TempFolder::new();
    let second_path = second.path().to_str().unwrap();
    run_git(
        repo.path(),
        &["worktree", "add", "-b", "second", second_path],
    );
    let text = "// Saves the answer.\n";

    // Add a cut from the main checkout, then an approval from the second worktree.
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

    // The main checkout sees the approval, so the comment counts as approved.
    let in_code = Comment {
        file: PathBuf::from("review_store.go"),
        name: Some("SaveAnswer".to_owned()),
        line: 1,
        end_line: 1,
        style: CommentStyle::Line,
        text: text.to_owned(),
    };
    let approvals = approvals::load(repo.path()).unwrap();
    assert_eq!(approvals.compare(&in_code), DecisionMatch::Approved);
}
