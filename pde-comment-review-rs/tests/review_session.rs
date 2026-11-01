mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Barrier;
use std::thread;

use common::TempFolder;
use pde_comment_review::shared::ErrorCode;
use pde_comment_review::shared::git::find_worktree_git_folder;
use pde_comment_review::shared::review_session::{
    FeedbackRequest, ReviewStatus, load, save_user_feedback,
};
use pde_comment_review::shared::user_feedback::{UserAnswer, UserFeedback, UserNote};
use serde_json::{Value, json};

/// Makes a new Git repository, standing in for the user's worktree.
fn make_repo() -> TempFolder {
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
