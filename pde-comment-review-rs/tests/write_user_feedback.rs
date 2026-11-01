mod common;

use std::fs::{self, File};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use common::TempFolder;
use pde_comment_review::shared::git::{find_shared_git_folder, find_worktree_git_folder};
use pde_comment_review::shared::{Error, ErrorCode};
use pde_comment_review::steps::write_user_feedback::{WriteResult, write_user_feedback};
use serde_json::{Value, json};

/// Holds a Go file with three named comments and one inside a function.
const STORE: &str = "package store\n\n// Saves it.\nfunc Save() {}\n\n// Loads it.\nfunc Load() {}\n\n// Drops it.\nfunc Drop() {}\n\nfunc Run() {\n\t// Runs it.\n}\n";
/// Holds a second Go file with one named comment.
const OTHER: &str = "package store\n\n// Keeps it.\nfunc Keep() {}\n";
/// Holds a Go file with `\r\n` line ends.
const CRLF: &str = "package store\r\n\r\n// Ends its lines.\r\nfunc End() {}\r\n";
/// Holds a Go file with one comment, on `Save`.
const SAVE: &str = "package store\n\n// Saves it.\nfunc Save() {}\n";
/// Holds the one-comment file after the user edits its comment by hand.
const EDITED: &str = "package store\n\n// Saves it, edited by hand.\nfunc Save() {}\n";

/// Makes a Git repository holding each file with its source.
fn make_repo(files: &[(&str, &str)]) -> TempFolder {
    let repo = TempFolder::new();
    let output = Command::new("git")
        .args(["init", "--quiet"])
        .arg(repo.path())
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    for (file, source) in files {
        let path = repo.path().join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, source).unwrap();
    }
    repo
}

/// Builds a Feedback Request about one comment, with a keep Agent Suggestion.
fn make_request(id: u32, file: &str, name: Option<&str>, text: &str, line: usize) -> Value {
    json!({
        "id": id,
        "file": file,
        "name": name,
        "reason": "new",
        "text": text,
        "can_reword": true,
        "line": line,
        "suggestion": { "kind": "keep", "reasoning": "Says what it does.", "also_at": [] }
    })
}

/// Saves a Review Session with each list of Feedback Requests and the User
/// Feedback by request ID.
fn write_review(repo: &Path, status: &str, lists: &[&[Value]], feedback: Value) -> Value {
    let lists: Vec<Value> = lists
        .iter()
        .map(|requests| json!({ "base": "4f2a9c1e", "requests": requests }))
        .collect();
    let review = json!({
        "status": status,
        "origin": { "repo": "store", "branch": "feat/store", "agent_session": "ses_123" },
        "lists": lists,
        "feedback": feedback
    });
    let review_file = find_review_file(repo);
    fs::create_dir_all(review_file.parent().unwrap()).unwrap();
    fs::write(review_file, review.to_string()).unwrap();
    review
}

/// Returns where the worktree's Review Session is saved, in its own Git folder.
fn find_review_file(repo: &Path) -> PathBuf {
    find_worktree_git_folder(repo)
        .unwrap()
        .join("pde/review.json")
}

/// Returns where `write_user_feedback` saves each file's original, in the
/// worktree's own Git folder.
fn find_backup_folder(repo: &Path) -> PathBuf {
    find_worktree_git_folder(repo)
        .unwrap()
        .join("pde/write-backup")
}

/// Returns the saved Review Session as JSON.
fn read_review(repo: &Path) -> Value {
    serde_json::from_slice(&fs::read(find_review_file(repo)).unwrap()).unwrap()
}

/// Returns where the clone's decisions are saved, in its shared Git folder.
fn find_approvals_file(repo: &Path) -> PathBuf {
    find_shared_git_folder(repo)
        .unwrap()
        .join("pde/code-comments.json")
}

/// Saves a Review Session with User Feedback of each kind, over files with `\n`
/// and `\r\n` line ends. The agent sent the first Feedback Request twice, the
/// second time with a reword.
fn make_answered_review() -> TempFolder {
    let repo = make_repo(&[("store.go", STORE), ("other.go", OTHER), ("crlf.go", CRLF)]);
    let save = make_request(1, "store.go", Some("Save"), "// Saves it.\n", 3);
    let mut save_again = save.clone();
    save_again["suggestion"]["kind"] = json!({ "reword": { "text": "Saves the answer." } });
    let requests = [
        save,
        make_request(2, "store.go", Some("Load"), "// Loads it.\n", 6),
        make_request(3, "store.go", Some("Drop"), "// Drops it.\n", 9),
        make_request(4, "store.go", None, "\t// Runs it.\n", 13),
        make_request(5, "other.go", Some("Keep"), "// Keeps it.\n", 3),
        make_request(6, "crlf.go", Some("End"), "// Ends its lines.\n", 3),
    ];
    let feedback = json!({
        "1": { "answer": "accept" },
        "2": { "answer": "keep" },
        "3": { "answer": "cut" },
        "4": { "answer": { "edited": { "text": "Runs every step." } } },
        "5": { "answer": "keep" },
        "6": { "answer": { "edited": { "text": "Keeps its line ends." } } }
    });
    write_review(
        repo.path(),
        "waiting_for_user_feedback",
        &[&requests, &[save_again]],
        feedback,
    );
    repo
}

/// Saves a waiting Review Session that rewords the comment in each file.
fn make_reworded_review(files: &[(&str, &str)]) -> (TempFolder, Value) {
    let repo = make_repo(files);
    let mut requests = Vec::new();
    let mut feedback = json!({});
    for (id, (file, _)) in (1..).zip(files) {
        requests.push(make_request(id, file, Some("Save"), "// Saves it.\n", 3));
        feedback[id.to_string()] =
            json!({ "answer": { "edited": { "text": "Saves the answer." } } });
    }
    let review = write_review(
        repo.path(),
        "waiting_for_user_feedback",
        &[&requests],
        feedback,
    );
    (repo, review)
}

/// Runs `write_user_feedback` on two reworded files and edits the first file
/// while the approvals save waits, then makes that save fail, so the first file
/// cannot go back.
fn write_while_first_file_is_edited() -> (TempFolder, Value, Error) {
    let (repo, review) = make_reworded_review(&[("a.go", SAVE), ("b.go", SAVE)]);
    let approvals_folder = find_shared_git_folder(repo.path()).unwrap().join("pde");
    fs::create_dir_all(&approvals_folder).unwrap();
    // The held approvals lock stops `write_user_feedback` once it has written both files.
    let approvals_lock = File::create(approvals_folder.join("code-comments.json.lock")).unwrap();
    approvals_lock.lock().unwrap();
    let error = thread::scope(|scope| {
        let writing = scope.spawn(|| write_user_feedback(repo.path()));
        wait_for_change(&repo.path().join("b.go"), SAVE);
        fs::write(repo.path().join("a.go"), EDITED).unwrap();
        // A folder named like the approvals file makes `approvals::add` fail.
        fs::create_dir(approvals_folder.join("code-comments.json")).unwrap();
        approvals_lock.unlock().unwrap();
        writing.join().unwrap().unwrap_err()
    });
    (repo, review, error)
}

/// Waits until the file no longer holds `old`, for at most ten seconds.
fn wait_for_change(path: &Path, old: &str) {
    let started = Instant::now();
    while fs::read_to_string(path).unwrap() == old && started.elapsed() < Duration::from_secs(10) {
        thread::sleep(Duration::from_millis(10));
    }
}

mod write_user_feedback {
    use super::*;

    /// `write_user_feedback` must write each Answered Request into its file, in its newest version.
    /// Otherwise the code gets wording the user never chose.
    #[test]
    fn writes_each_answered_request_in_newest_version() {
        let repo = make_answered_review();

        write_user_feedback(repo.path()).unwrap();

        assert_eq!(
            fs::read_to_string(repo.path().join("store.go")).unwrap(),
            "package store\n\n// Saves the answer.\nfunc Save() {}\n\n// Loads it.\nfunc Load() {}\n\nfunc Drop() {}\n\nfunc Run() {\n\t// Runs every step.\n}\n"
        );
    }

    /// `write_user_feedback` must write a reword into a file with `\r\n` line ends using those line ends.
    /// Otherwise the reword is refused, or the file ends up with mixed line ends.
    #[test]
    fn reword_keeps_crlf_line_ends() {
        let repo = make_answered_review();

        write_user_feedback(repo.path()).unwrap();

        assert_eq!(
            fs::read_to_string(repo.path().join("crlf.go")).unwrap(),
            "package store\r\n\r\n// Keeps its line ends.\r\nfunc End() {}\r\n"
        );
    }

    /// `write_user_feedback` must give each written file the permissions it had.
    /// Otherwise a script would lose its permission to run.
    #[test]
    fn written_file_keeps_its_permissions() {
        let (repo, _) = make_reworded_review(&[("a.go", SAVE)]);
        let a_file = repo.path().join("a.go");
        fs::set_permissions(&a_file, fs::Permissions::from_mode(0o755)).unwrap();

        write_user_feedback(repo.path()).unwrap();

        let mode = fs::metadata(&a_file).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o755);
    }

    /// `write_user_feedback` must save one decision for each named comment, and none for a comment without a name.
    /// Otherwise an agent could later change a comment the user kept, or bring back one they cut.
    #[test]
    fn saves_one_decision_per_named_comment() {
        let repo = make_answered_review();

        let result = write_user_feedback(repo.path()).unwrap();

        assert!(matches!(result, WriteResult::Written { approved: 5, .. }));
        let saved: Value =
            serde_json::from_slice(&fs::read(find_approvals_file(repo.path())).unwrap()).unwrap();
        assert_eq!(
            saved["approved"],
            json!([
                { "file": "store.go", "name": "Save", "text": "// Saves the answer.\n" },
                { "file": "store.go", "name": "Load", "text": "// Loads it.\n" },
                { "file": "store.go", "name": "Drop", "text": "" },
                { "file": "other.go", "name": "Keep", "text": "// Keeps it.\n" },
                { "file": "crlf.go", "name": "End", "text": "// Keeps its line ends.\n" }
            ])
        );
    }

    /// `write_user_feedback` must return `AlreadyWritten` once the Review Session is done.
    /// Otherwise a second screen would try the write again and fail on the comments the first one changed.
    #[test]
    fn second_write_returns_already_written() {
        let (repo, _) = make_reworded_review(&[("a.go", SAVE)]);
        write_user_feedback(repo.path()).unwrap();

        let second = write_user_feedback(repo.path()).unwrap();

        assert_eq!(second, WriteResult::AlreadyWritten);
    }

    /// `write_user_feedback` must remove the saved originals once every file is written.
    /// Otherwise the next write in this worktree would stop and ask the user about them.
    #[test]
    fn removes_backup_once_every_file_is_written() {
        let (repo, _) = make_reworded_review(&[("a.go", SAVE)]);

        write_user_feedback(repo.path()).unwrap();

        assert!(!find_backup_folder(repo.path()).exists());
    }

    /// `write_user_feedback` must refuse to write when a comment changed after the user answered it, and keep the User Feedback.
    /// Otherwise the write would replace the agent's new text, or lose the user's work.
    #[test]
    fn reworded_comment_refuses_write_and_keeps_feedback() {
        let (repo, _) = make_reworded_review(&[("a.go", SAVE), ("b.go", SAVE)]);
        let reworded = SAVE.replace("Saves it.", "Saves it now.");
        fs::write(repo.path().join("b.go"), &reworded).unwrap();
        let saved_before = fs::read(find_review_file(repo.path())).unwrap();

        let error = write_user_feedback(repo.path()).unwrap_err();

        assert_eq!(error.code, ErrorCode::TextChanged, "{}", error.message);
        assert_eq!(fs::read_to_string(repo.path().join("a.go")).unwrap(), SAVE);
        assert_eq!(
            fs::read_to_string(repo.path().join("b.go")).unwrap(),
            reworded
        );
        let saved_after = fs::read(find_review_file(repo.path())).unwrap();
        assert_eq!(saved_after, saved_before);
    }

    /// `write_user_feedback` must refuse with `TextChanged` when a file it would write is gone.
    /// Otherwise the Feedback Requests about that file could never go back to the agent.
    #[test]
    fn deleted_file_refuses_write_as_text_changed() {
        let (repo, _) = make_reworded_review(&[("a.go", SAVE), ("b.go", SAVE)]);
        fs::remove_file(repo.path().join("b.go")).unwrap();

        let error = write_user_feedback(repo.path()).unwrap_err();

        assert_eq!(error.code, ErrorCode::TextChanged, "{}", error.message);
        assert_eq!(fs::read_to_string(repo.path().join("a.go")).unwrap(), SAVE);
    }

    /// `write_user_feedback` must find a comment without a name at the line the user saw.
    /// Otherwise User Feedback on one comment could land on another with the same text.
    #[test]
    fn unnamed_comment_reword_changes_only_its_line() {
        let source =
            "package store\n\nfunc Run() {\n\t// Runs it.\n}\n\nfunc Rerun() {\n\t// Runs it.\n}\n";
        let repo = make_repo(&[("a.go", source)]);
        let requests = [make_request(1, "a.go", None, "\t// Runs it.\n", 8)];
        let feedback = json!({ "1": { "answer": { "edited": { "text": "Runs it again." } } } });
        write_review(
            repo.path(),
            "waiting_for_user_feedback",
            &[&requests],
            feedback,
        );

        write_user_feedback(repo.path()).unwrap();

        assert_eq!(
            fs::read_to_string(repo.path().join("a.go")).unwrap(),
            "package store\n\nfunc Run() {\n\t// Runs it.\n}\n\nfunc Rerun() {\n\t// Runs it again.\n}\n"
        );
    }

    /// `write_user_feedback` must refuse a file whose link leads outside the worktree, before writing any file.
    /// Otherwise User Feedback could change a file outside the worktree.
    #[test]
    fn link_outside_worktree_refuses_every_write() {
        let outside = TempFolder::new();
        let outside_file = outside.path().join("c.go");
        fs::write(&outside_file, SAVE).unwrap();
        let (repo, _) = make_reworded_review(&[("a.go", SAVE), ("c.go", SAVE)]);
        let link = repo.path().join("c.go");
        fs::remove_file(&link).unwrap();
        std::os::unix::fs::symlink(&outside_file, &link).unwrap();

        let error = write_user_feedback(repo.path()).unwrap_err();

        assert_eq!(error.code, ErrorCode::WriteFailed, "{}", error.message);
        assert_eq!(fs::read_to_string(repo.path().join("a.go")).unwrap(), SAVE);
        assert_eq!(fs::read_to_string(&outside_file).unwrap(), SAVE);
    }

    /// `write_user_feedback` must leave every file, permission and decision as it was when a file write fails, and let the user try again.
    /// Otherwise the code could hold part of the User Feedback, or a decision could be saved for text that is not in the file.
    #[test]
    fn failed_write_changes_nothing_and_allows_retry() {
        let (repo, review) = make_reworded_review(&[("a.go", SAVE), ("locked/b.go", SAVE)]);
        let a_file = repo.path().join("a.go");
        fs::set_permissions(&a_file, fs::Permissions::from_mode(0o755)).unwrap();
        let locked = repo.path().join("locked");
        // A file cannot be replaced in a folder that cannot be written.
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o555)).unwrap();

        let error = write_user_feedback(repo.path()).unwrap_err();

        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(error.code, ErrorCode::WriteFailed, "{}", error.message);
        assert_eq!(fs::read_to_string(&a_file).unwrap(), SAVE);
        let mode = fs::metadata(&a_file).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o755);
        assert_eq!(fs::read_to_string(locked.join("b.go")).unwrap(), SAVE);
        assert!(!find_approvals_file(repo.path()).exists());
        assert_eq!(read_review(repo.path()), review);
        assert!(!find_backup_folder(repo.path()).exists());
    }

    /// `write_user_feedback` must put every file back and let the user try again when the decisions cannot be saved.
    /// Otherwise the code would keep the User Feedback while its decisions are lost, so the user would be asked again.
    #[test]
    fn failed_approval_save_restores_files_and_allows_retry() {
        let (repo, review) = make_reworded_review(&[("a.go", SAVE)]);
        // A folder named like the approvals file makes `approvals::add` fail.
        fs::create_dir_all(find_approvals_file(repo.path())).unwrap();

        let error = write_user_feedback(repo.path()).unwrap_err();

        assert_eq!(error.code, ErrorCode::WriteFailed, "{}", error.message);
        assert_eq!(fs::read_to_string(repo.path().join("a.go")).unwrap(), SAVE);
        assert_eq!(read_review(repo.path()), review);
        assert!(!find_backup_folder(repo.path()).exists());
    }

    /// `write_user_feedback` must write nothing for a Review Session whose last write did not finish, or that has a Feedback Request with no answer.
    /// Otherwise a write could hide what a crashed write left behind, or mark a Review Session done with comments unanswered.
    #[test]
    fn unfinished_or_unanswered_review_session_writes_nothing() {
        let requests = [make_request(1, "a.go", Some("Save"), "// Saves it.\n", 3)];
        let cases = [
            (
                "unfinished write",
                "writing_user_feedback",
                json!({ "1": { "answer": "cut" } }),
                ErrorCode::WriteNotFinished,
            ),
            (
                "unanswered",
                "waiting_for_user_feedback",
                json!({}),
                ErrorCode::NothingToWrite,
            ),
        ];
        for (case, status, feedback, code) in cases {
            let repo = make_repo(&[("a.go", SAVE)]);
            write_review(repo.path(), status, &[&requests], feedback);
            let saved_before = fs::read(find_review_file(repo.path())).unwrap();

            let error = write_user_feedback(repo.path()).unwrap_err();

            assert_eq!(error.code, code, "{case}");
            assert_eq!(
                fs::read_to_string(repo.path().join("a.go")).unwrap(),
                SAVE,
                "{case}"
            );
            let saved_after = fs::read(find_review_file(repo.path())).unwrap();
            assert_eq!(saved_after, saved_before, "{case}");
        }
    }

    /// `write_user_feedback` must leave a file the user edited during the write as the user left it, even when the write fails.
    /// Otherwise putting the files back would erase the user's edit.
    #[test]
    fn failed_restore_keeps_edit_made_during_write() {
        let (repo, _, _) = write_while_first_file_is_edited();

        assert_eq!(
            fs::read_to_string(repo.path().join("a.go")).unwrap(),
            EDITED
        );
    }

    /// `write_user_feedback` must put back the files after one it cannot put back.
    /// Otherwise one edited file would leave the rest holding part of the User Feedback.
    #[test]
    fn failed_restore_still_restores_later_files() {
        let (repo, _, _) = write_while_first_file_is_edited();

        assert_eq!(fs::read_to_string(repo.path().join("b.go")).unwrap(), SAVE);
    }

    /// `write_user_feedback` must keep the saved originals and the `writing_user_feedback` status when a file cannot go back.
    /// Otherwise the user would lose the only copy of that file's original, or see the write as finished.
    #[test]
    fn failed_restore_keeps_originals_and_unfinished_status() {
        let (repo, mut review, error) = write_while_first_file_is_edited();

        assert_eq!(error.code, ErrorCode::WriteNotFinished, "{}", error.message);
        let backup_folder = find_backup_folder(repo.path());
        assert_eq!(
            fs::read_to_string(backup_folder.join("a.go")).unwrap(),
            SAVE
        );
        review["status"] = json!("writing_user_feedback");
        assert_eq!(read_review(repo.path()), review);
    }
}
