mod common;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use common::{
    APPROVED, REWORDED, STORE, add_decision, commit_all, find_waiting_worktrees, make_repo,
    request_json, save_review, store_source, use_test_waiting_list, write_store,
};
use pde_comment_review::shared::ErrorCode;
use pde_comment_review::shared::review_session::{
    AgentSuggestion, FeedbackRequestList, ReviewSession, ReviewStatus, load, save_user_feedback,
};
use pde_comment_review::shared::user_feedback::{UserAnswer, UserFeedback};
use pde_comment_review::steps::NextStep;
use pde_comment_review::steps::make_feedback_request_list::{
    ChangeRequest, make_feedback_request_list,
};
use pde_comment_review::steps::send_agent_suggestions::send_agent_suggestions;
use pde_comment_review::steps::write_user_feedback::{WriteResult, write_user_feedback};
use serde_json::{Value, json};

/// Names a second Go file, for answers that stay in another file.
const REVIEW_FILE: &str = "review_file.go";
/// Holds a Go comment that ends in a `Deprecated:` note, which Go's tools read.
const DEPRECATED_COMMENT: &str = "// Saves the answer.\n//\n// Deprecated: use SaveAll.\n";
/// Holds a Go comment with two `Deprecated:` notes.
const TWO_NOTES_COMMENT: &str =
    "// Saves the answer.\n//\n// Deprecated: use SaveAll.\n//\n// Deprecated: use SaveEach.\n";
/// Holds a Go file with the same unnamed comment in two functions, on lines 5 and 10.
const TWIN_COMMENTS: &str = "package review\n\n// Saves it.\nfunc SaveAnswer() {\n\t// Holds the lock.\n\tlock()\n}\n\nfunc LoadAnswer() {\n\t// Holds the lock.\n\tlock()\n}\n";
/// Holds a Rust doc comment whose code example rustdoc reads as Rust code.
const DOC_TEST_SOURCE: &str = "/// Saves the answer.\n///\n/// ```\n/// assert_eq!(1 + 1, 2);\n/// ```\npub fn save_answer() {}\n";

/// Returns an Agent Suggestion of `kind`, such as `json!("keep")`, or none for null.
fn make_suggestion(kind: &Value) -> Option<AgentSuggestion> {
    if kind.is_null() {
        return None;
    }
    let suggestion = json!({"kind": kind, "reasoning": "Says what the code does.", "also_at": []});
    Some(serde_json::from_value(suggestion).unwrap())
}

/// Returns the kind of a reword Agent Suggestion with `words`.
fn reword(words: &str) -> Value {
    json!({"reword": {"text": words}})
}

/// Builds step 3's Feedback Request List and gives every request an Agent Suggestion of `kind`, as the agent does.
fn build_list(dir: &Path, change_requests: &[ChangeRequest], kind: Value) -> FeedbackRequestList {
    let mut list = make_feedback_request_list(dir, None, change_requests)
        .unwrap()
        .result
        .list;
    for request in &mut list.requests {
        request.suggestion = make_suggestion(&kind);
    }
    list
}

/// Builds the list again after the agent answered the User Notes, suggests keeping each comment, sends it and loads the Review Session.
fn send_after_user_notes(dir: &Path) -> ReviewSession {
    let list = build_list(dir, &[], json!("keep"));
    send_agent_suggestions(dir, list).unwrap();
    load(dir).unwrap().unwrap()
}

/// Returns the ID of each Feedback Request in any of the Review Session's lists.
fn list_request_ids(review_session: &ReviewSession) -> BTreeSet<u64> {
    review_session
        .lists
        .iter()
        .flat_map(|list| &list.requests)
        .map(|request| json!(request.id).as_u64().unwrap())
        .collect()
}

mod send_agent_suggestions {
    use super::*;

    /// `send_agent_suggestions` must save the list as list 1 with its Agent Suggestions, wait for User Feedback and tell the agent to wait for the user.
    /// Otherwise the user would answer a list without the agent's advice, or the agent would go on before the user answered.
    #[test]
    fn sent_list_waits_for_user_feedback() {
        use_test_waiting_list();
        let repo = make_repo();
        let dir = repo.path();
        write_store(dir, REWORDED, "");
        let list = build_list(dir, &[], json!("keep"));

        let output = send_agent_suggestions(dir, list).unwrap();

        assert_eq!(output.result.list_number, 1);
        assert_eq!(output.next, NextStep::TellUserAndWait);
        let review_session = load(dir).unwrap().unwrap();
        assert_eq!(review_session.status, ReviewStatus::WaitingForUserFeedback);
        let sent = &review_session.lists[0].requests[0];
        assert_eq!(sent.suggestion, make_suggestion(&json!("keep")));
    }

    /// `send_agent_suggestions` must put the worktree on the waiting list once it sends the list.
    /// Otherwise the user would never see the list, since Ctrl+b e opens only the reviews on that list.
    #[test]
    fn sent_list_goes_on_waiting_list() {
        use_test_waiting_list();
        let repo = make_repo();
        let dir = repo.path();
        write_store(dir, REWORDED, "");
        let list = build_list(dir, &[], json!("keep"));

        send_agent_suggestions(dir, list).unwrap();

        assert_eq!(find_waiting_worktrees(dir).len(), 1);
    }

    /// `send_agent_suggestions` must leave the code as it is while it tries each reword in memory.
    /// Otherwise an Agent Suggestion would reach the code before the user answered it.
    #[test]
    fn sending_list_leaves_code_unchanged() {
        use_test_waiting_list();
        let repo = make_repo();
        let dir = repo.path();
        write_store(dir, REWORDED, "");
        let list = build_list(
            dir,
            &[],
            reword("Writes the answer, then lets the lock go."),
        );
        let before = fs::read_to_string(dir.join(STORE)).unwrap();

        send_agent_suggestions(dir, list).unwrap();

        assert_eq!(fs::read_to_string(dir.join(STORE)).unwrap(), before);
    }

    /// `send_agent_suggestions` must refuse, saving nothing, a list that `make_feedback_request_list` would now build differently.
    /// Otherwise a comment the agent added after step 3 would skip the user.
    #[test]
    fn refuses_list_after_code_changes() {
        let repo = make_repo();
        let dir = repo.path();
        write_store(dir, REWORDED, "");
        let list = build_list(dir, &[], json!("keep"));
        let added = "\n// Counts the answers.\nfunc CountAnswers() {}\n";
        fs::write(dir.join(STORE), store_source(REWORDED, "") + added).unwrap();

        let error = send_agent_suggestions(dir, list).unwrap_err();

        assert_eq!(error.code, ErrorCode::ListChanged);
        assert!(load(dir).unwrap().is_none());
    }

    /// `send_agent_suggestions` must refuse a request with no Agent Suggestion, a reword with no words, of a comment the user can only keep or cut, or that breaks a style rule, and a cut step 7 could not write, each with its own code and saving nothing.
    /// Otherwise the user could accept nothing, cut a comment by accident, accept a reword step 7 never writes, approve text that breaks a rule, or answer a list that is never written.
    #[test]
    fn refuses_suggestions_that_must_not_reach_user() {
        let cases = [
            (
                "no-suggestion",
                REWORDED,
                Value::Null,
                ErrorCode::MissingSuggestion,
            ),
            (
                "reword-without-words",
                REWORDED,
                reword(" \n"),
                ErrorCode::BadInput,
            ),
            (
                "reword-of-keep-or-cut-only-comment",
                "/* Saves the answer under the lock. */\n",
                reword("Saves the answer."),
                ErrorCode::BadInput,
            ),
            (
                "reword-with-filler",
                REWORDED,
                reword("This facilitates saving the answer."),
                ErrorCode::RewordFailsRules,
            ),
            (
                "cut-of-keep-only-comment",
                "/* Saves the answer under the lock. */\n",
                json!("cut"),
                ErrorCode::WouldNotWrite,
            ),
        ];

        for (case, comment, kind, code) in cases {
            let repo = make_repo();
            let dir = repo.path();
            write_store(dir, comment, "");
            let list = build_list(dir, &[], kind);

            let error = send_agent_suggestions(dir, list).unwrap_err();

            assert_eq!(error.code, code, "{case}");
            assert!(load(dir).unwrap().is_none(), "{case}");
        }
    }

    /// `send_agent_suggestions` must refuse a reword that changes a Rust doc test, and send a reword that changes only the prose around it or a cut.
    /// Otherwise the user could accept an Agent Suggestion that changes what `cargo test` runs, or the agent could not reword or cut a comment with a test.
    #[test]
    fn refuses_only_rewords_that_change_doc_tests() {
        use_test_waiting_list();
        let cases = [
            (
                "prose-only",
                reword("Writes the answer.\n\n```\nassert_eq!(1 + 1, 2);\n```"),
                Ok(1),
            ),
            (
                "example-changed",
                reword("Writes the answer.\n\n```\nassert_eq!(2 + 2, 4);\n```"),
                Err(ErrorCode::WouldNotWrite),
            ),
            ("cut", json!("cut"), Ok(1)),
        ];

        for (case, kind, expected) in cases {
            let repo = make_repo();
            let dir = repo.path();
            fs::write(dir.join("lib.rs"), DOC_TEST_SOURCE).unwrap();
            let list = build_list(dir, &[], kind);

            let sent = send_agent_suggestions(dir, list)
                .map(|output| output.result.list_number)
                .map_err(|error| error.code);

            assert_eq!(sent, expected, "{case}");
        }
    }

    /// `send_agent_suggestions` must refuse a reword or cut that drops a Go `Deprecated:` note, even one of two, and send a reword that keeps the note.
    /// Otherwise accepting an Agent Suggestion could hide a deprecation from Go's tools, or the agent could not reword a comment with a note.
    #[test]
    fn refuses_suggestions_that_drop_words_tools_read() {
        use_test_waiting_list();
        let deprecated = store_source(DEPRECATED_COMMENT, "");
        let cases = [
            (
                "deprecated-dropped-by-reword",
                STORE,
                deprecated.clone(),
                reword("Saves the answer."),
                Err(ErrorCode::WouldNotWrite),
            ),
            (
                "deprecated-cut",
                STORE,
                deprecated.clone(),
                json!("cut"),
                Err(ErrorCode::WouldNotWrite),
            ),
            (
                "one-of-two-notes-dropped",
                STORE,
                store_source(TWO_NOTES_COMMENT, ""),
                reword("Saves the answer.\n\nDeprecated: use SaveAll."),
                Err(ErrorCode::WouldNotWrite),
            ),
            (
                "deprecated-kept",
                STORE,
                deprecated,
                reword("Saves each answer.\n\nDeprecated: use SaveAll."),
                Ok(1),
            ),
        ];

        for (case, file, source, kind, expected) in cases {
            let repo = make_repo();
            let dir = repo.path();
            fs::write(dir.join(file), source).unwrap();
            let list = build_list(dir, &[], kind);

            let sent = send_agent_suggestions(dir, list)
                .map(|output| output.result.list_number)
                .map_err(|error| error.code);

            assert_eq!(sent, expected, "{case}");
        }
    }

    /// `send_agent_suggestions` must build the list again with the change requests behind its `agent_wants_change` requests.
    /// Otherwise every list that asks to reword an approved comment would be refused as changed.
    #[test]
    fn rebuild_keeps_change_requests() {
        use_test_waiting_list();
        let repo = make_repo();
        let dir = repo.path();
        add_decision(dir, STORE, "SaveAnswer", APPROVED);
        let change_request = ChangeRequest {
            file: PathBuf::from(STORE),
            name: "SaveAnswer".to_owned(),
        };
        let list = build_list(
            dir,
            &[change_request],
            reword("Writes the answer while it holds the lock."),
        );

        let output = send_agent_suggestions(dir, list).unwrap();

        assert_eq!(output.result.list_number, 1);
    }

    /// `send_agent_suggestions` must start a new Review Session when the last one is done.
    /// Otherwise the old User Feedback for request 1 would answer the new request 1 before the user saw it.
    #[test]
    fn starts_new_review_session_after_done_one() {
        use_test_waiting_list();
        let repo = make_repo();
        let dir = repo.path();
        // A finished Review Session already answered its request 1.
        let answered = request_json(1, Some("SaveAnswer"), APPROVED, 3);
        save_review(
            dir,
            "done",
            &[json!([answered])],
            json!({"1": {"answer": "keep"}}),
        );
        write_store(dir, REWORDED, "");
        let list = build_list(dir, &[], json!("keep"));

        let output = send_agent_suggestions(dir, list).unwrap();

        assert_eq!(output.result.list_number, 1);
        let review_session = load(dir).unwrap().unwrap();
        assert_eq!(review_session.lists.len(), 1);
        assert!(review_session.feedback.is_empty());
    }

    /// `send_agent_suggestions` must drop the User Note of each request it sends again, and take a noted request whose comment is gone out of every list.
    /// Otherwise the user would never see the resent request, and the review would wait forever on the gone one.
    #[test]
    fn resent_requests_drop_notes_and_gone_comments() {
        use_test_waiting_list();
        let repo = make_repo();
        let dir = repo.path();
        let source = "package review\n\n// Saves it.\nfunc SaveAnswer() {}\n\n// Loads it.\nfunc LoadAnswers() {}\n";
        fs::write(dir.join(STORE), source).unwrap();
        commit_all(dir);
        // The user noted both comments.
        let saves = request_json(1, Some("SaveAnswer"), "// Saves it.\n", 3);
        let loads = request_json(2, Some("LoadAnswers"), "// Loads it.\n", 6);
        let notes = json!({"1": {"note": "Say what it saves."}, "2": {"note": "Cut it."}});
        save_review(dir, "user_notes_waiting", &[json!([saves, loads])], notes);
        // The agent fixes one noted comment and deletes the other.
        let fixed = "package review\n\n// Saves each answer.\nfunc SaveAnswer() {}\n\nfunc LoadAnswers() {}\n";
        fs::write(dir.join(STORE), fixed).unwrap();

        let review_session = send_after_user_notes(dir);

        assert_eq!(list_request_ids(&review_session), BTreeSet::from([1]));
        assert!(review_session.feedback.is_empty());
    }

    /// `send_agent_suggestions` must take an answered request out of every list once the agent rewords its comment, while an unchanged one keeps its answer.
    /// Otherwise two requests would ask about one comment and step 7 would refuse the write, or the user would answer the unchanged comment again.
    #[test]
    fn answered_request_keeps_answer_only_while_unchanged() {
        use_test_waiting_list();
        let repo = make_repo();
        let dir = repo.path();
        let review_file = "package review\n\n// Opens the file.\nfunc OpenReview() {}\n\n// Closes the file.\nfunc CloseReview() {}\n";
        write_store(dir, "// Saves it.\n", "");
        fs::write(dir.join(REVIEW_FILE), review_file).unwrap();
        commit_all(dir);
        // The user noted one comment and kept two in another file.
        let saves = request_json(1, Some("SaveAnswer"), "// Saves it.\n", 3);
        let mut opens = request_json(2, Some("OpenReview"), "// Opens the file.\n", 3);
        opens["file"] = json!(REVIEW_FILE);
        let mut closes = request_json(3, Some("CloseReview"), "// Closes the file.\n", 6);
        closes["file"] = json!(REVIEW_FILE);
        let feedback = json!({
            "1": {"note": "Say what it saves."},
            "2": {"answer": "keep"},
            "3": {"answer": "keep"},
        });
        save_review(
            dir,
            "user_notes_waiting",
            &[json!([saves, opens, closes])],
            feedback,
        );
        // The agent fixes the noted comment and rewords one of the kept ones.
        write_store(dir, "// Saves each answer.\n", "");
        let reworded = review_file.replace("Closes the file.", "Closes the file and its lock.");
        fs::write(dir.join(REVIEW_FILE), reworded).unwrap();

        let review_session = send_after_user_notes(dir);

        assert_eq!(list_request_ids(&review_session), BTreeSet::from([1, 2, 4]));
        let feedback = serde_json::to_value(&review_session.feedback).unwrap();
        assert_eq!(feedback, json!({"2": {"answer": "keep"}}));
    }

    /// `send_agent_suggestions` must give a kept Answered Request the line its comment moved to, since step 7 finds a comment with no name only at its saved line.
    /// Otherwise a reword one line longer above an answered comment would make step 7 refuse the write and send the whole file back to the agent.
    #[test]
    fn kept_answered_request_moves_with_its_comment() {
        use_test_waiting_list();
        let repo = make_repo();
        let dir = repo.path();
        let body = "\n\t// Holds the lock.\n\tlock()\n";
        write_store(dir, "// Saves it.\n", body);
        commit_all(dir);
        // The user noted the function's comment and kept the comment inside it.
        let saves = request_json(1, Some("SaveAnswer"), "// Saves it.\n", 3);
        let holds = request_json(2, None, "\t// Holds the lock.\n", 5);
        let feedback = json!({"1": {"note": "Say what it saves."}, "2": {"answer": "keep"}});
        save_review(
            dir,
            "user_notes_waiting",
            &[json!([saves, holds])],
            feedback,
        );
        // The agent's new comment is one line longer, so the kept comment moves down.
        let longer = "// Saves the answer.\n// It holds the lock while it writes.\n";
        write_store(dir, longer, body);
        let review_session = send_after_user_notes(dir);
        // The user keeps the new comment, so every Feedback Request has an answer.
        let resent = &review_session.lists.last().unwrap().requests[0];
        save_user_feedback(dir, resent, UserFeedback::Answer(UserAnswer::Keep)).unwrap();

        let written = write_user_feedback(dir).unwrap();

        assert!(
            matches!(written, WriteResult::Written { .. }),
            "{written:?}"
        );
    }

    /// `send_agent_suggestions` must leave each Answered Request on the nearest comment with its words, matching every request in the file as step 3 does.
    /// Otherwise deleting one of two same-word comments would move its keep onto the one the user cut, or a noted twin would take an answered one's comment and step 7 would refuse the write.
    #[test]
    fn same_word_comments_keep_their_own_answers() {
        use_test_waiting_list();
        let saves = request_json(1, Some("SaveAnswer"), "// Saves it.\n", 3);
        let first_twin = |id| request_json(id, None, "\t// Holds the lock.\n", 5);
        let second_twin = |id| request_json(id, None, "\t// Holds the lock.\n", 10);
        let more_code = "\tlock()\n".repeat(6);
        let cases = [
            (
                "kept-twin-deleted",
                json!([saves, first_twin(2), second_twin(3)]),
                json!({"1": {"note": "Say what it saves."}, "2": {"answer": "keep"}, "3": {"answer": "cut"}}),
                TWIN_COMMENTS.replacen("\t// Holds the lock.\n", "", 1),
                0,
            ),
            (
                "noted-twin-stays-near",
                json!([first_twin(1), second_twin(2)]),
                json!({"1": {"note": "Say what it holds."}, "2": {"answer": "keep"}}),
                TWIN_COMMENTS.replacen("\tlock()\n", &format!("\tlock()\n{more_code}"), 1),
                2,
            ),
        ];

        for (case, requests, feedback, agent_source, twins_left) in cases {
            let repo = make_repo();
            let dir = repo.path();
            fs::write(dir.join(STORE), TWIN_COMMENTS).unwrap();
            commit_all(dir);
            save_review(dir, "user_notes_waiting", &[requests], feedback);
            // The agent answers the User Note, deleting or moving a twin.
            fs::write(dir.join(STORE), agent_source).unwrap();
            let review_session = send_after_user_notes(dir);
            // The user keeps each resent request, so every Feedback Request has an answer.
            for resent in &review_session.lists.last().unwrap().requests {
                save_user_feedback(dir, resent, UserFeedback::Answer(UserAnswer::Keep)).unwrap();
            }

            write_user_feedback(dir).unwrap();

            let written = fs::read_to_string(dir.join(STORE)).unwrap();
            assert_eq!(
                written.matches("// Holds the lock.").count(),
                twins_left,
                "{case}"
            );
        }
    }

    /// `send_agent_suggestions` must send an empty list after User Notes, keeping the answers that still fit and waiting for User Feedback.
    /// Otherwise a review whose noted comments were all deleted would stay with the agent, and the user's other answers would never be written.
    #[test]
    fn empty_resent_list_waits_for_user_feedback() {
        use_test_waiting_list();
        let repo = make_repo();
        let dir = repo.path();
        let source = "package review\n\n// Saves it.\nfunc SaveAnswer() {}\n\n// Loads it.\nfunc LoadAnswers() {}\n";
        fs::write(dir.join(STORE), source).unwrap();
        commit_all(dir);
        // The user noted one comment and kept the other.
        let saves = request_json(1, Some("SaveAnswer"), "// Saves it.\n", 3);
        let loads = request_json(2, Some("LoadAnswers"), "// Loads it.\n", 6);
        let feedback = json!({"1": {"note": "Cut it."}, "2": {"answer": "keep"}});
        save_review(
            dir,
            "user_notes_waiting",
            &[json!([saves, loads])],
            feedback,
        );
        // The agent deletes the noted comment, so step 3 builds an empty list.
        fs::write(dir.join(STORE), source.replace("// Saves it.\n", "")).unwrap();
        let list = build_list(dir, &[], json!("keep"));

        let output = send_agent_suggestions(dir, list).unwrap();

        assert_eq!(output.result.list_number, 2);
        let review_session = load(dir).unwrap().unwrap();
        assert_eq!(review_session.status, ReviewStatus::WaitingForUserFeedback);
        let feedback = serde_json::to_value(&review_session.feedback).unwrap();
        assert_eq!(feedback, json!({"2": {"answer": "keep"}}));
    }
}
