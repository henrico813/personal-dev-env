mod common;

use common::{APPROVED, make_repo, request_json, save_review};
use pde_comment_review::steps::NextStep;
use pde_comment_review::steps::show_status::{Waiting, show_status};
use serde_json::{Value, json};

/// Describes what the Review Session waits on as JSON, or `null` for nothing.
fn describe_waiting(waiting: Waiting) -> Value {
    match waiting {
        Waiting::Nothing => json!(null),
        Waiting::FeedbackRequests(ids) => json!(ids),
        Waiting::UserNotes(notes) => json!(notes),
    }
}

/// `show_status` must name the next step and the requests or User Notes that each Review Session status waits on.
/// Otherwise an agent could call the wrong step next, or miss what the user waits for.
#[test]
fn tells_agent_what_comes_next() {
    let requests = || {
        json!([
            request_json(1, Some("SaveAnswer"), APPROVED, 3),
            request_json(2, None, "\t// Holds the lock.\n", 5),
        ])
    };
    let keep = || json!({"answer": "keep"});
    let cases = [
        (
            "waiting",
            "waiting_for_user_feedback",
            json!({"1": keep()}),
            NextStep::TellUserAndWait,
            json!([2]),
        ),
        (
            "notes",
            "user_notes_waiting",
            json!({"1": {"note": "Say which lock."}, "2": keep()}),
            NextStep::MakeFeedbackRequestList,
            json!({"1": "Say which lock."}),
        ),
        (
            "writing",
            "writing_user_feedback",
            json!({"1": keep(), "2": keep()}),
            NextStep::TellUserAndWait,
            json!(null),
        ),
        (
            "done",
            "done",
            json!({"1": keep(), "2": keep()}),
            NextStep::CheckComments,
            json!(null),
        ),
    ];

    for (case, status, feedback, next, waiting) in cases {
        let repo = make_repo();
        let dir = repo.path();
        save_review(dir, status, &[requests()], feedback);

        let output = show_status(dir).unwrap();

        assert_eq!(output.next, next, "{case}");
        assert_eq!(describe_waiting(output.result.waiting), waiting, "{case}");
    }
}

/// `show_status` must name `check_comments` as the next step, with nothing waiting, when there is no Review Session.
/// Otherwise an agent could call the wrong step before its first review.
#[test]
fn points_to_check_comments_without_review_session() {
    let repo = make_repo();

    let output = show_status(repo.path()).unwrap();

    assert_eq!(output.next, NextStep::CheckComments);
    assert_eq!(describe_waiting(output.result.waiting), json!(null));
}
