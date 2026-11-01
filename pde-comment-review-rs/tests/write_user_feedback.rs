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

const STORE: &str = "package store\n\n// Saves it.\nfunc Save() {}\n\n// Loads it.\nfunc Load() {}\n\n// Drops it.\nfunc Drop() {}\n\nfunc Run() {\n\t// Runs it.\n}\n";
const OTHER: &str = "package store\n\n// Keeps it.\nfunc Keep() {}\n";
const CRLF: &str = "package store\r\n\r\n// Ends its lines.\r\nfunc End() {}\r\n";
const SAVE: &str = "package store\n\n// Saves it.\nfunc Save() {}\n";
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

/// Builds a request about one comment, which the agent suggests keeping.
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

/// Saves a review with each list of requests and the feedback by request ID.
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

fn find_review_file(repo: &Path) -> PathBuf {
    find_worktree_git_folder(repo)
        .unwrap()
        .join("pde/review.json")
}

fn find_backup_folder(repo: &Path) -> PathBuf {
    find_worktree_git_folder(repo)
        .unwrap()
        .join("pde/write-backup")
}

fn read_review(repo: &Path) -> Value {
    serde_json::from_slice(&fs::read(find_review_file(repo)).unwrap()).unwrap()
}

/// Saves a review with one answer of each kind, over files with `\n` and
/// `\r\n` line ends. The agent sent the first request twice, the second time
/// with a reword.
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

/// Saves a waiting review that rewords the comment in each file.
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

/// Runs step 7 on two reworded files and edits the first file while the
/// approvals save waits, then makes that save fail, so the first file cannot go
/// back.
fn write_while_first_file_is_edited() -> (TempFolder, Value, Error) {
    let (repo, review) = make_reworded_review(&[("a.go", SAVE), ("b.go", SAVE)]);
    let approvals_folder = find_shared_git_folder(repo.path()).unwrap().join("pde");
    fs::create_dir_all(&approvals_folder).unwrap();
    // Holding the approvals lock stops step 7 once it has written both files.
    let approvals_lock = File::create(approvals_folder.join("code-comments.json.lock")).unwrap();
    approvals_lock.lock().unwrap();
    let error = thread::scope(|scope| {
        let writing = scope.spawn(|| write_user_feedback(repo.path()));
        wait_for_change(&repo.path().join("b.go"), SAVE);
        fs::write(repo.path().join("a.go"), EDITED).unwrap();
        // A folder where the approvals file belongs makes saving them fail.
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

    /// Without it, the user's answers would not reach the code, or a file would
    /// lose its permission to run or its line ends.
    #[test]
    fn writes_newest_answers_keeping_modes_and_newlines() {
        let repo = make_answered_review();
        let store = repo.path().join("store.go");
        fs::set_permissions(&store, fs::Permissions::from_mode(0o755)).unwrap();

        let result = write_user_feedback(repo.path()).unwrap();

        let WriteResult::Written { files, .. } = result else {
            panic!("the call writes, but returned {result:?}");
        };
        assert_eq!(files, ["crlf.go", "store.go"].map(PathBuf::from));
        assert_eq!(
            fs::read_to_string(&store).unwrap(),
            "package store\n\n// Saves the answer.\nfunc Save() {}\n\n// Loads it.\nfunc Load() {}\n\nfunc Drop() {}\n\nfunc Run() {\n\t// Runs every step.\n}\n"
        );
        assert_eq!(
            fs::read_to_string(repo.path().join("crlf.go")).unwrap(),
            "package store\r\n\r\n// Keeps its line ends.\r\nfunc End() {}\r\n"
        );
        let mode = fs::metadata(&store).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o755);
    }

    /// Without it, an agent could later change a comment the user kept or bring
    /// back one they cut, or a second screen could save the decisions twice.
    #[test]
    fn saves_decisions_once_for_named_comments_only() {
        let repo = make_answered_review();

        let first = write_user_feedback(repo.path()).unwrap();
        let second = write_user_feedback(repo.path()).unwrap();

        assert!(matches!(first, WriteResult::Written { approved: 5, .. }));
        assert_eq!(second, WriteResult::AlreadyWritten);
        let approvals_file = find_shared_git_folder(repo.path())
            .unwrap()
            .join("pde/code-comments.json");
        let saved: Value = serde_json::from_slice(&fs::read(approvals_file).unwrap()).unwrap();
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

    /// Without it, the originals saved during a write would stay behind, and the
    /// next write in this worktree would stop and ask the user about them.
    #[test]
    fn removes_backup_after_writing_every_answer() {
        let (repo, _) = make_reworded_review(&[("a.go", SAVE)]);

        write_user_feedback(repo.path()).unwrap();

        assert!(!find_backup_folder(repo.path()).exists());
    }

    /// Without it, step 7 would write over a comment the agent changed or removed
    /// after the user answered, or lose the user's answers.
    #[test]
    fn changed_comment_refuses_write_and_keeps_answers() {
        let reworded = SAVE.replace("Saves it.", "Saves it now.");
        for (case, changed) in [("reworded", Some(reworded.as_str())), ("deleted", None)] {
            let (repo, _) = make_reworded_review(&[("a.go", SAVE), ("b.go", SAVE)]);
            let b_file = repo.path().join("b.go");
            match changed {
                Some(source) => fs::write(&b_file, source).unwrap(),
                None => fs::remove_file(&b_file).unwrap(),
            }
            let saved_before = fs::read(find_review_file(repo.path())).unwrap();

            let error = write_user_feedback(repo.path()).unwrap_err();

            assert_eq!(error.code, ErrorCode::TextChanged, "{case}");
            let a_source = fs::read_to_string(repo.path().join("a.go")).unwrap();
            assert_eq!(a_source, SAVE, "{case}");
            assert_eq!(
                fs::read_to_string(&b_file).ok().as_deref(),
                changed,
                "{case}"
            );
            let saved_after = fs::read(find_review_file(repo.path())).unwrap();
            assert_eq!(saved_after, saved_before, "{case}");
        }
    }

    /// Without it, a link in the worktree would let step 7 write a file outside
    /// it.
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

    /// Without it, a write that fails partway would leave some files with the new
    /// answers, and the user could not try again.
    #[test]
    fn failed_write_restores_files_and_allows_retry() {
        let (repo, review) = make_reworded_review(&[("a.go", SAVE), ("locked/b.go", SAVE)]);
        let locked = repo.path().join("locked");
        // A file cannot be replaced in a folder that cannot be written.
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o555)).unwrap();

        let error = write_user_feedback(repo.path()).unwrap_err();

        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(error.code, ErrorCode::WriteFailed, "{}", error.message);
        assert_eq!(fs::read_to_string(repo.path().join("a.go")).unwrap(), SAVE);
        assert_eq!(fs::read_to_string(locked.join("b.go")).unwrap(), SAVE);
        assert_eq!(read_review(repo.path()), review);
        assert!(!find_backup_folder(repo.path()).exists());
    }

    /// Without it, code could keep the user's answers while their approvals are
    /// lost, so the user would be asked again.
    #[test]
    fn failed_approval_save_restores_files_and_allows_retry() {
        let (repo, review) = make_reworded_review(&[("a.go", SAVE)]);
        let approvals_file = find_shared_git_folder(repo.path())
            .unwrap()
            .join("pde/code-comments.json");
        // A folder where the approvals file belongs makes saving them fail.
        fs::create_dir_all(approvals_file).unwrap();

        let error = write_user_feedback(repo.path()).unwrap_err();

        assert_eq!(error.code, ErrorCode::WriteFailed, "{}", error.message);
        assert_eq!(fs::read_to_string(repo.path().join("a.go")).unwrap(), SAVE);
        assert_eq!(read_review(repo.path()), review);
        assert!(!find_backup_folder(repo.path()).exists());
    }

    /// Without it, a write could hide what a crashed write left behind, or mark a
    /// review written while some comments were never answered.
    #[test]
    fn refuses_unfinished_or_unanswered_reviews_writing_nothing() {
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

    /// Without it, putting files back could erase an edit made during the write,
    /// or give up on the files after it.
    #[test]
    fn failed_restore_keeps_other_files_and_edit() {
        let (repo, _, error) = write_while_first_file_is_edited();

        assert_eq!(error.code, ErrorCode::WriteNotFinished, "{}", error.message);
        assert_eq!(
            fs::read_to_string(repo.path().join("a.go")).unwrap(),
            EDITED
        );
        assert_eq!(fs::read_to_string(repo.path().join("b.go")).unwrap(), SAVE);
    }

    /// Without it, a file that could not go back would lose its original and the
    /// user's answers, or look like a finished write.
    #[test]
    fn failed_restore_keeps_backup_and_writing_review() {
        let (repo, mut review, _) = write_while_first_file_is_edited();

        let backup_folder = find_backup_folder(repo.path());
        assert_eq!(
            fs::read_to_string(backup_folder.join("a.go")).unwrap(),
            SAVE
        );
        review["status"] = json!("writing_user_feedback");
        assert_eq!(read_review(repo.path()), review);
    }
}
