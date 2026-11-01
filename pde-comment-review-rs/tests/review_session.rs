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

fn find_review_file(repo: &Path) -> PathBuf {
    find_worktree_git_folder(repo)
        .unwrap()
        .join("pde/review.json")
}

fn make_review(status: &str, request_count: u32) -> Value {
    let requests: Vec<Value> = (1..=request_count)
        .map(|id| {
            json!({
                "id": id,
                "file": "src/lib.rs",
                "name": format!("item_{id}"),
                "reason": "new",
                "text": "/// Saves one answer.\n",
                "can_reword": true,
                "line": id,
                "suggestion": {
                    "kind": "keep",
                    "reasoning": "Says what it saves.",
                    "also_at": []
                }
            })
        })
        .collect();
    json!({
        "status": status,
        "origin": {
            "repo": "personal-dev-env",
            "branch": "feat/review-lock",
            "agent_session": "ses_123"
        },
        "lists": [{ "base": "4f2a9c1e", "requests": requests }],
        "feedback": {}
    })
}

fn write_review(repo: &Path, saved_text: &str) {
    let review_file = find_review_file(repo);
    fs::create_dir_all(review_file.parent().unwrap()).unwrap();
    fs::write(review_file, saved_text).unwrap();
}

fn load_requests(repo: &Path) -> Vec<FeedbackRequest> {
    let mut review = load(repo).unwrap().unwrap();
    review.lists.remove(0).requests
}

/// Keeps an older version from losing a newer review, and a broken file from blocking reviews or replacing a backup.
#[test]
fn refused_saved_files_stay_put_unless_unreadable() {
    let mut newer_field = make_review("waiting_for_user_feedback", 1);
    newer_field["paused_at"] = json!("2026-10-31");
    let whole = make_review("waiting_for_user_feedback", 1).to_string();
    let cases = [
        (
            "unknown field",
            newer_field.to_string(),
            ErrorCode::UnknownSavedField,
            "review.json",
        ),
        (
            "unknown value",
            make_review("paused", 1).to_string(),
            ErrorCode::UnknownSavedField,
            "review.json",
        ),
        (
            "cut short",
            whole[..whole.len() / 2].to_owned(),
            ErrorCode::SavedFileUnreadable,
            "review.json.unreadable.2",
        ),
    ];
    for (case, saved_text, code, kept_as) in cases {
        let repo = make_repo();
        // An earlier damaged file was already moved aside.
        write_review(repo.path(), "not json");
        load(repo.path()).unwrap_err();
        write_review(repo.path(), &saved_text);
        let review_file = find_review_file(repo.path());

        let error = load(repo.path()).unwrap_err();

        assert_eq!(error.code, code, "{case}");
        let first_backup = review_file.with_file_name("review.json.unreadable");
        assert_eq!(
            fs::read_to_string(&first_backup).unwrap(),
            "not json",
            "{case}"
        );
        let kept_file = review_file.with_file_name(kept_as);
        assert_eq!(
            fs::read_to_string(&kept_file).unwrap(),
            saved_text,
            "{case}"
        );
        // Only a file kept in place leaves review.json behind.
        assert_eq!(review_file.exists(), kept_file == review_file, "{case}");
    }
}

/// Keeps two open review screens from losing each other's answers.
#[test]
fn concurrent_saves_keep_all_user_feedback() {
    let repo = make_repo();
    write_review(
        repo.path(),
        &make_review("waiting_for_user_feedback", 40).to_string(),
    );
    let requests = load_requests(repo.path());
    let start = Barrier::new(2);

    // Each thread opens its own lock, which Linux treats like a second process.

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

    let review = load(repo.path()).unwrap().unwrap();
    assert_eq!(review.feedback.len(), 40);
}

/// Keeps a review from going to the agent before the last answer, or with no note to act on.
#[test]
fn status_changes_after_last_feedback_if_noted() {
    let cases = [
        (
            "note first",
            UserFeedback::Note(UserNote("Say why it locks.".to_owned())),
            ReviewStatus::UserNotesWaiting,
        ),
        (
            "answers only",
            UserFeedback::Answer(UserAnswer::Cut),
            ReviewStatus::WaitingForUserFeedback,
        ),
    ];
    for (case, first_feedback, status_after_last) in cases {
        let repo = make_repo();
        write_review(
            repo.path(),
            &make_review("waiting_for_user_feedback", 2).to_string(),
        );
        let requests = load_requests(repo.path());

        let status_after_first =
            save_user_feedback(repo.path(), &requests[0], first_feedback).unwrap();
        let returned_after_last = save_user_feedback(
            repo.path(),
            &requests[1],
            UserFeedback::Answer(UserAnswer::Keep),
        )
        .unwrap();

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

/// Tells the user an answer came too late to be written, instead of silently dropping it.
#[test]
fn feedback_refused_and_unsaved_unless_review_waits() {
    for status in ["user_notes_waiting", "writing_user_feedback", "done"] {
        let repo = make_repo();
        write_review(repo.path(), &make_review(status, 1).to_string());
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

/// Keeps a screen left open from approving text the user never saw.
#[test]
fn answer_to_changed_request_refused_and_unsaved() {
    let cases = [
        (
            "comment reworded",
            "/text",
            json!("/// Saves one answer under the lock.\n"),
        ),
        (
            "suggestion changed",
            "/suggestion/kind",
            json!({ "reword": { "text": "Saves one answer under the lock." } }),
        ),
    ];
    for (case, changed_field, new_value) in cases {
        let repo = make_repo();
        let mut review = make_review("waiting_for_user_feedback", 1);
        write_review(repo.path(), &review.to_string());
        let shown_request = load_requests(repo.path()).remove(0);
        // The agent sends the request again with a change this screen never showed.
        let mut sent_again = review["lists"][0].clone();
        *sent_again["requests"][0]
            .pointer_mut(changed_field)
            .unwrap() = new_value;
        review["lists"].as_array_mut().unwrap().push(sent_again);
        write_review(repo.path(), &review.to_string());
        let review_file = find_review_file(repo.path());
        let saved_before = fs::read(&review_file).unwrap();

        let error = save_user_feedback(
            repo.path(),
            &shown_request,
            UserFeedback::Answer(UserAnswer::Accept),
        )
        .unwrap_err();

        assert_eq!(error.code, ErrorCode::TextChanged, "{case}");
        assert_eq!(fs::read(&review_file).unwrap(), saved_before, "{case}");
    }
}
