mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Barrier;
use std::thread;

use common::{TempFolder, commit_all, find_waiting_worktrees, run_git, use_test_waiting_list};
use pde_comment_review::shared::ErrorCode;
use pde_comment_review::shared::git::find_worktree_git_folder;
use pde_comment_review::shared::review_session::{
    FeedbackRequest, ReviewStatus, add_to_waiting_list, load, save_user_feedback,
};
use pde_comment_review::shared::user_feedback::{UserAnswer, UserFeedback, UserNote};
use serde_json::{Value, json};

/// Makes a new Git repository, standing in for the user's worktree, and keeps the waiting list in the build folder.
fn make_repo() -> TempFolder {
    use_test_waiting_list();
    let folder = TempFolder::new();
    let output = Command::new("git")
        .args(["init", "--quiet"])
        .arg(folder.path())
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    folder
}

/// Returns where the worktree's Review Session is saved, in its own Git folder.
fn find_review_file(repo: &Path) -> PathBuf {
    find_worktree_git_folder(repo)
        .unwrap()
        .join("pde/review.json")
}

/// Builds a saved Review Session with one Feedback Request List and no User Feedback yet.
fn make_review_session(status: &str, request_count: u32) -> Value {
    let requests: Vec<Value> = (1..=request_count)
        .map(|id| {
            json!({
                "id": id,
                "file": "src/lib.rs",
                "name": format!("item_{id}"),
                "reason": "new",
                "text": "/// Takes the lock.\n",
                "can_reword": true,
                "line": id,
                "suggestion": {
                    "kind": "keep",
                    "reasoning": "Says what it locks.",
                    "also_at": []
                }
            })
        })
        .collect();
    json!({
        "status": status,
        "origin": {
            "repo": "personal-dev-env",
            "branch": "feat/lock-and-save",
            "agent_session": "ses_123"
        },
        "lists": [{ "base": "4f2a9c1e", "requests": requests }],
        "feedback": {}
    })
}

/// Writes `review.json` directly, as if another program had saved it.
fn write_review_session(repo: &Path, saved_text: &str) {
    let review_file = find_review_file(repo);
    fs::create_dir_all(review_file.parent().unwrap()).unwrap();
    fs::write(review_file, saved_text).unwrap();
}

/// Returns the Feedback Requests of the first Feedback Request List, as a screen shows them.
fn load_requests(repo: &Path) -> Vec<FeedbackRequest> {
    let mut review_session = load(repo).unwrap().unwrap();
    review_session.lists.remove(0).requests
}

/// `load` must refuse a file from a newer version and leave it where it is.
/// Otherwise an older program would move it aside, and the newer version would lose its Review Session.
#[test]
fn load_leaves_newer_version_file_in_place() {
    let mut newer_field = make_review_session("waiting_for_user_feedback", 1);
    newer_field["paused_at"] = json!("2026-10-31");
    let cases = [
        ("unknown-field", newer_field.to_string()),
        (
            "unknown-value",
            make_review_session("paused", 1).to_string(),
        ),
    ];
    for (case, saved_text) in cases {
        let repo = make_repo();
        write_review_session(repo.path(), &saved_text);
        let review_file = find_review_file(repo.path());

        let error = load(repo.path()).unwrap_err();

        // The file keeps its name and every byte, and no backup appears.
        assert_eq!(error.code, ErrorCode::UnknownSavedField, "{case}");
        assert_eq!(
            fs::read_to_string(&review_file).unwrap(),
            saved_text,
            "{case}"
        );
        let backup = review_file.with_file_name("review.json.unreadable");
        assert!(!backup.exists(), "{case}");
    }
}

/// `load` must move a damaged file to a backup name no earlier backup uses.
/// Otherwise the damaged file would block every later Review Session in the worktree, or replace the earlier backup.
#[test]
fn load_moves_damaged_file_to_new_backup() {
    // An earlier damaged file was already moved aside.
    let repo = make_repo();
    write_review_session(repo.path(), "not json");
    load(repo.path()).unwrap_err();

    // A Review Session cut short in the middle is saved next.
    let whole = make_review_session("waiting_for_user_feedback", 1).to_string();
    let cut_short = &whole[..whole.len() / 2];
    write_review_session(repo.path(), cut_short);
    let review_file = find_review_file(repo.path());

    let error = load(repo.path()).unwrap_err();

    // Both backups hold their own text, and review.json is gone.
    assert_eq!(error.code, ErrorCode::SavedFileUnreadable);
    let first_backup = review_file.with_file_name("review.json.unreadable");
    assert_eq!(fs::read_to_string(&first_backup).unwrap(), "not json");
    let second_backup = review_file.with_file_name("review.json.unreadable.2");
    assert_eq!(fs::read_to_string(&second_backup).unwrap(), cut_short);
    assert!(!review_file.exists());
}

/// Two screens that save User Feedback at the same time must keep every piece of it.
/// Otherwise one screen would write over the other's User Feedback, and the user would have to give it again.
#[test]
fn two_screens_lose_no_user_feedback() {
    let repo = make_repo();
    write_review_session(
        repo.path(),
        &make_review_session("waiting_for_user_feedback", 40).to_string(),
    );
    let requests = load_requests(repo.path());
    let start = Barrier::new(2);

    // Each thread opens its own lock, which Linux treats like a second process.
    // The two threads start together and each saves User Feedback on 20 Feedback Requests.
    thread::scope(|scope| {
        for half in requests.chunks(20) {
            let (dir, start) = (repo.path(), &start);
            scope.spawn(move || {
                start.wait();
                for request in half {
                    save_user_feedback(dir, request, UserFeedback::Answer(UserAnswer::Keep))
                        .unwrap();
                }
            });
        }
    });

    let review_session = load(repo.path()).unwrap().unwrap();
    assert_eq!(review_session.feedback.len(), 40);
}

/// The Review Session must go to the agent only after the last User Feedback, and only when a User Note waits.
/// Otherwise the agent could start before the user is done, or get a Review Session with no User Note to act on.
#[test]
fn agent_gets_user_notes_after_last_user_feedback() {
    let cases = [
        (
            "user-note-first",
            UserFeedback::Note(UserNote("Say why it locks.".to_owned())),
            ReviewStatus::UserNotesWaiting,
        ),
        (
            "no-user-note",
            UserFeedback::Answer(UserAnswer::Cut),
            ReviewStatus::WaitingForUserFeedback,
        ),
    ];
    for (case, first_feedback, status_after_last) in cases {
        let repo = make_repo();
        write_review_session(
            repo.path(),
            &make_review_session("waiting_for_user_feedback", 2).to_string(),
        );
        let requests = load_requests(repo.path());

        // The user gives User Feedback on the first of two Feedback Requests, then the last.
        let status_after_first =
            save_user_feedback(repo.path(), &requests[0], first_feedback).unwrap();
        let returned_after_last = save_user_feedback(
            repo.path(),
            &requests[1],
            UserFeedback::Answer(UserAnswer::Keep),
        )
        .unwrap();

        // The status waits after the first, and both the reply and the file agree after the last.
        assert_eq!(
            status_after_first,
            ReviewStatus::WaitingForUserFeedback,
            "{case}"
        );
        assert_eq!(returned_after_last, status_after_last, "{case}");
        let saved_status = load(repo.path()).unwrap().unwrap().status;
        assert_eq!(saved_status, status_after_last, "{case}");
    }
}

/// `save_user_feedback` must refuse User Feedback once the Review Session stops waiting for it, and change nothing.
/// Otherwise the screen could not tell the user that this User Feedback came too late to count.
#[test]
fn late_user_feedback_is_refused_and_unsaved() {
    for status in ["user_notes_waiting", "writing_user_feedback", "done"] {
        let repo = make_repo();
        write_review_session(repo.path(), &make_review_session(status, 1).to_string());
        let review_file = find_review_file(repo.path());
        let saved_before = fs::read(&review_file).unwrap();
        let requests = load_requests(repo.path());

        let error = save_user_feedback(
            repo.path(),
            &requests[0],
            UserFeedback::Answer(UserAnswer::Keep),
        )
        .unwrap_err();

        assert_eq!(error.code, ErrorCode::LateUserFeedback, "{status}");
        assert_eq!(fs::read(&review_file).unwrap(), saved_before, "{status}");
    }
}

/// A screen left open must not save User Feedback on a Feedback Request the agent changed since.
/// Otherwise the user could approve a comment or Agent Suggestion they never saw.
#[test]
fn outdated_screen_cannot_answer_a_changed_request() {
    let cases = [
        (
            "comment-reworded",
            "/text",
            json!("/// Takes the lock and reads the file.\n"),
        ),
        (
            "suggestion-changed",
            "/suggestion/kind",
            json!({ "reword": { "text": "Takes the lock and reads the file." } }),
        ),
    ];
    for (case, changed_field, new_value) in cases {
        let repo = make_repo();
        let mut review_session = make_review_session("waiting_for_user_feedback", 1);
        write_review_session(repo.path(), &review_session.to_string());
        let shown_request = load_requests(repo.path()).remove(0);

        // The agent sends the Feedback Request again with a change this screen never showed.
        let mut sent_again = review_session["lists"][0].clone();
        *sent_again["requests"][0]
            .pointer_mut(changed_field)
            .unwrap() = new_value;
        review_session["lists"]
            .as_array_mut()
            .unwrap()
            .push(sent_again);
        write_review_session(repo.path(), &review_session.to_string());
        let review_file = find_review_file(repo.path());
        let saved_before = fs::read(&review_file).unwrap();

        let error = save_user_feedback(
            repo.path(),
            &shown_request,
            UserFeedback::Answer(UserAnswer::Accept),
        )
        .unwrap_err();

        assert_eq!(error.code, ErrorCode::RequestChanged, "{case}");
        assert_eq!(fs::read(&review_file).unwrap(), saved_before, "{case}");
    }
}

/// `save_user_feedback` must take the worktree off the waiting list once every Feedback Request has User Feedback and any of it is a User Note, and not before.
/// Otherwise Ctrl+b e would offer a Review Session the agent must act on, or lose one whose User Feedback step 7 has yet to write.
#[test]
fn last_user_note_takes_worktree_off_list() {
    let cases = [
        (
            "user-note-first",
            UserFeedback::Note(UserNote("Say why it locks.".to_owned())),
            false,
        ),
        ("no-user-note", UserFeedback::Answer(UserAnswer::Cut), true),
    ];
    for (case, first_feedback, listed_after_last) in cases {
        let repo = make_repo();
        write_review_session(
            repo.path(),
            &make_review_session("waiting_for_user_feedback", 2).to_string(),
        );
        // Step 5 put the worktree on the waiting list when it sent the list.
        add_to_waiting_list(repo.path()).unwrap();
        let requests = load_requests(repo.path());

        // The user gives User Feedback on the first of two Feedback Requests, then the last.
        save_user_feedback(repo.path(), &requests[0], first_feedback).unwrap();
        let listed_after_first = !find_waiting_worktrees(repo.path()).is_empty();
        save_user_feedback(
            repo.path(),
            &requests[1],
            UserFeedback::Answer(UserAnswer::Keep),
        )
        .unwrap();

        assert!(listed_after_first, "{case}");
        let listed = !find_waiting_worktrees(repo.path()).is_empty();
        assert_eq!(listed, listed_after_last, "{case}");
    }
}

/// `read_waiting_list` must skip a worktree the user removed while its Review Session waited.
/// Otherwise Ctrl+b e would keep offering a Review Session that no command can drop.
#[test]
fn read_waiting_list_skips_removed_worktree() {
    let repo = make_repo();
    commit_all(repo.path());
    run_git(repo.path(), &["worktree", "add", "--quiet", "linked"]);
    let linked = repo.path().join("linked");
    add_to_waiting_list(repo.path()).unwrap();
    add_to_waiting_list(&linked).unwrap();

    // The user removes the linked worktree while its Review Session waits.
    run_git(repo.path(), &["worktree", "remove", "linked"]);

    let root = fs::canonicalize(repo.path()).unwrap();
    assert_eq!(find_waiting_worktrees(repo.path()), [root]);
}

/// `read_waiting_list` must return each worktree once, by its top folder, however often and from wherever it was added.
/// Otherwise the Review Session Picker would offer one Review Session twice.
#[test]
fn read_waiting_list_returns_each_worktree_once() {
    let repo = make_repo();
    let subfolder = repo.path().join("src");
    fs::create_dir(&subfolder).unwrap();

    add_to_waiting_list(repo.path()).unwrap();
    add_to_waiting_list(&subfolder).unwrap();

    let root = fs::canonicalize(repo.path()).unwrap();
    assert_eq!(find_waiting_worktrees(repo.path()), [root]);
}

/// `add_to_waiting_list` must keep a listed worktree it cannot reach while another worktree adds itself.
/// Otherwise a worktree that was briefly out of reach, such as on a drive that is not mounted, would leave Ctrl+b e for good.
#[test]
fn add_to_waiting_list_keeps_unreachable_worktree() {
    use_test_waiting_list();
    let folder = TempFolder::new();
    run_git(folder.path(), &["init", "--quiet", "first"]);
    run_git(folder.path(), &["init", "--quiet", "second"]);
    let first = folder.path().join("first");
    add_to_waiting_list(&first).unwrap();

    // The first worktree is out of reach while the second adds itself, then comes back.
    let away = folder.path().join("away");
    fs::rename(&first, &away).unwrap();
    add_to_waiting_list(&folder.path().join("second")).unwrap();
    fs::rename(&away, &first).unwrap();

    let real_folder = fs::canonicalize(folder.path()).unwrap();
    assert_eq!(
        find_waiting_worktrees(folder.path()),
        [real_folder.join("first"), real_folder.join("second")]
    );
}

/// `add_to_waiting_list` must keep every worktree when several worktrees add to the waiting list at once.
/// Otherwise Ctrl+b e would not offer a Review Session that waits for the user.
#[test]
fn add_to_waiting_list_loses_no_worktree() {
    use_test_waiting_list();
    let folder = TempFolder::new();
    let worktrees: Vec<PathBuf> = (0..40)
        .map(|number| {
            let name = format!("worktree-{number}");
            run_git(folder.path(), &["init", "--quiet", &name]);
            folder.path().join(name)
        })
        .collect();
    let start = Barrier::new(2);

    // Each thread opens its own lock, which Linux treats like a second process.
    // The two threads start together and each adds 20 worktrees.
    thread::scope(|scope| {
        for half in worktrees.chunks(20) {
            let start = &start;
            scope.spawn(move || {
                start.wait();
                for worktree in half {
                    add_to_waiting_list(worktree).unwrap();
                }
            });
        }
    });

    assert_eq!(find_waiting_worktrees(folder.path()).len(), 40);
}

/// `save_user_feedback` must drop why the last write failed once a User Note hands the Review Session to the agent.
/// Otherwise the user could later hear that a write failed for a list that was never written.
#[test]
fn user_note_drops_last_write_error() {
    let repo = make_repo();
    let mut failed_write = make_review_session("waiting_for_user_feedback", 1);
    failed_write["feedback"] = json!({ "1": { "answer": "keep" } });
    failed_write["write_error"] =
        json!({ "code": "write_failed", "detail": "Cannot write src/lib.rs." });
    write_review_session(repo.path(), &failed_write.to_string());
    let requests = load_requests(repo.path());

    let note = UserFeedback::Note(UserNote("Say why it locks.".to_owned()));
    save_user_feedback(repo.path(), &requests[0], note).unwrap();

    let review_session = load(repo.path()).unwrap().unwrap();
    assert_eq!(review_session.status, ReviewStatus::UserNotesWaiting);
    assert!(review_session.write_error.is_none());
}
