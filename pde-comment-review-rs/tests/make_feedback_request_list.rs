mod common;

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use common::{
    APPROVED, REWORDED, STORE, add_decision, commit_all, make_repo, request_json, run_git,
    save_review, store_source, write_store,
};
use pde_comment_review::shared::ErrorCode;
use pde_comment_review::shared::git::resolve_commit;
use pde_comment_review::shared::review_session::{FeedbackRequest, RequestId};
use pde_comment_review::steps::make_feedback_request_list::{
    ChangeRequest, make_feedback_request_list,
};
use serde_json::json;

/// Returns the saved Review Session's modified time and bytes, or `None` when it is missing.
fn read_review_file(dir: &Path) -> Option<(SystemTime, Vec<u8>)> {
    let path = dir.join(".git/pde/review.json");
    let bytes = fs::read(&path).ok()?;
    Some((fs::metadata(&path).unwrap().modified().unwrap(), bytes))
}

/// Describes a request by its file, name and reason, for short assertions.
fn describe(request: &FeedbackRequest) -> String {
    let name = request.name.as_deref().unwrap_or("(unnamed)");
    let reason = serde_json::to_value(request.reason).unwrap();
    let reason = reason.as_str().unwrap();
    format!("{} {name} {reason}", request.file.display())
}

/// Describes a request by its ID and line, then as `describe` does.
fn describe_with_id(request: &FeedbackRequest) -> String {
    format!(
        "{} {} {}",
        json!(request.id),
        request.line,
        describe(request)
    )
}

/// Returns the Feedback Requests for a change request on line 3 and a new comment on line 6.
fn list_change_request_and_new_comment() -> Vec<FeedbackRequest> {
    let repo = make_repo();
    let dir = repo.path();
    add_decision(dir, STORE, "SaveAnswer", APPROVED);
    let source = format!(
        "{}\n// Counts the answers.\nfunc CountAnswers() {{}}\n",
        store_source(APPROVED, "")
    );
    fs::write(dir.join(STORE), source).unwrap();
    // The change request is found after the new comment, so only sorting puts it first.
    let change_request = ChangeRequest {
        file: PathBuf::from(STORE),
        name: "SaveAnswer".to_owned(),
    };

    make_feedback_request_list(dir, None, &[change_request])
        .unwrap()
        .result
        .list
        .requests
}

/// `make_feedback_request_list` must refuse while `check_comments` still fails.
/// Otherwise an agent could put its reword of approved text on the Feedback Request List, and keeping it would approve the reword.
#[test]
fn refuses_while_check_comments_fails() {
    let repo = make_repo();
    let dir = repo.path();
    add_decision(dir, STORE, "SaveAnswer", APPROVED);
    write_store(dir, REWORDED, "");
    let change_request = ChangeRequest {
        file: PathBuf::from(STORE),
        name: "SaveAnswer".to_owned(),
    };

    // The change request would refuse with another code if `check_comments` did not refuse first.
    let error = make_feedback_request_list(dir, None, &[change_request]).unwrap_err();

    assert_eq!(error.code, ErrorCode::CheckFailed);
}

/// `make_feedback_request_list` must list only changed comments and comments that go against a decision, such as a cut comment that is back.
/// Otherwise the user would miss a decision made in another worktree, or answer comments nobody changed.
#[test]
fn lists_only_changed_or_conflicting_comments() {
    let repo = make_repo();
    let dir = repo.path();
    let store_before = "package review\n\n// Saves the answer under the lock.\nfunc SaveAnswer() {}\n\n// Loads every answer.\nfunc LoadAnswers() {}\n\nfunc CountAnswers() {}\n";
    let store_after = "package review\n\n// Saves the answer under the lock.\nfunc SaveAnswer() {}\n\n// Loads each saved answer.\nfunc LoadAnswers() {}\n\n// Counts the answers.\nfunc CountAnswers() {}\n\n// Deletes an answer.\nfunc DeleteAnswer() {}\n";
    let file = "package review\n\n// Opens the review file.\nfunc OpenReview() {}\n\n// Closes the review file.\nfunc CloseReview() {}\n";
    fs::write(dir.join(STORE), store_before).unwrap();
    fs::write(dir.join("review_file.go"), file).unwrap();
    commit_all(dir);
    let base = resolve_commit(dir, "HEAD").unwrap();
    fs::write(dir.join(STORE), store_after).unwrap();
    add_decision(dir, STORE, "LoadAnswers", "");
    add_decision(dir, STORE, "DeleteAnswer", "// Deletes an answer.\n");
    add_decision(dir, "review_file.go", "OpenReview", "");
    add_decision(
        dir,
        "review_file.go",
        "CloseReview",
        "// Closes the file.\n",
    );

    let output = make_feedback_request_list(dir, Some(base), &[]).unwrap();

    let listed: Vec<String> = output.result.list.requests.iter().map(describe).collect();
    assert_eq!(
        listed,
        [
            "review_file.go OpenReview was_cut",
            "review_file.go CloseReview differs_from_approved",
            "review_store.go LoadAnswers was_cut",
            "review_store.go CountAnswers new",
        ]
    );
}

/// `make_feedback_request_list` must give each Feedback Request its own ID.
/// Otherwise two would share one, and one's User Feedback would be lost.
#[test]
fn gives_each_feedback_request_its_own_id() {
    let requests = list_change_request_and_new_comment();

    let ids: BTreeSet<RequestId> = requests.iter().map(|request| request.id).collect();
    assert_eq!(ids.len(), requests.len());
}

/// `make_feedback_request_list` must number Feedback Requests in the order they appear in the file.
/// Otherwise the user would answer them out of order.
#[test]
fn numbers_feedback_requests_in_file_order() {
    let requests = list_change_request_and_new_comment();

    let listed: Vec<String> = requests.iter().map(describe_with_id).collect();
    assert_eq!(
        listed,
        [
            "1 3 review_store.go SaveAnswer agent_wants_change",
            "2 6 review_store.go CountAnswers new",
        ]
    );
}

/// `make_feedback_request_list` must refuse while a Feedback Request List waits for the user or a write did not finish.
/// Otherwise a new Feedback Request List could replace one the user is still answering, or run over an unfinished write.
#[test]
fn refuses_while_review_session_waits_or_writes() {
    let cases = [
        (
            "waiting_for_user_feedback",
            ErrorCode::WaitingForUserFeedback,
        ),
        ("writing_user_feedback", ErrorCode::WriteNotFinished),
    ];

    for (status, code) in cases {
        let repo = make_repo();
        let dir = repo.path();
        let requests = json!([request_json(1, Some("SaveAnswer"), APPROVED, 3)]);
        save_review(dir, status, &[requests], json!({"1": {"answer": "cut"}}));

        let error = make_feedback_request_list(dir, None, &[]).unwrap_err();

        assert_eq!(error.code, code, "{status}");
    }
}

/// `make_feedback_request_list` must refuse a base other than the saved one while User Notes wait.
/// Otherwise a later base could hide a comment committed after the Feedback Request List was sent.
#[test]
fn refuses_other_base_while_user_notes_wait() {
    let repo = make_repo();
    let dir = repo.path();
    let requests = json!([request_json(1, Some("SaveAnswer"), APPROVED, 3)]);
    save_review(
        dir,
        "user_notes_waiting",
        &[requests],
        json!({"1": {"note": "Say which lock."}}),
    );
    write_store(dir, REWORDED, "");
    commit_all(dir);
    let later = resolve_commit(dir, "HEAD").unwrap();

    let error = make_feedback_request_list(dir, Some(later), &[]).unwrap_err();

    assert_eq!(error.code, ErrorCode::BadInput);
}

/// `make_feedback_request_list` must refuse, naming the file, when a commit or the index holds a comment it would ask about that the disk copy does not.
/// Otherwise that comment could ship without User Feedback, even from a file with no decision.
#[test]
fn refuses_unasked_committed_or_staged_comment() {
    /// Puts the cut comment only in the index.
    fn stage_only(dir: &Path) {
        write_store(dir, "", "");
        commit_all(dir);
        write_store(dir, APPROVED, "return");
        run_git(dir, &["add", STORE]);
        run_git(
            dir,
            &["restore", "--source=HEAD", "--worktree", "--", STORE],
        );
    }
    /// Puts the cut comment only in HEAD.
    fn commit_only(dir: &Path) {
        write_store(dir, APPROVED, "return");
        commit_all(dir);
        write_store(dir, "", "return");
        run_git(dir, &["add", STORE]);
    }
    /// Commits a new unnamed comment, then rewords it on disk only.
    fn reword_unnamed_on_disk(dir: &Path) {
        write_store(dir, "", "\n\t// Holds the lock.\n\treturn\n");
        commit_all(dir);
        write_store(dir, "", "\n\t// Holds the store lock.\n\treturn\n");
    }
    /// Stages a new file with a comment, then deletes it from disk.
    fn stage_then_delete_new_file(dir: &Path) {
        let source = "package review\n\n// Counts the answers.\nfunc CountAnswers() {}\n";
        fs::write(dir.join("other.go"), source).unwrap();
        run_git(dir, &["add", "other.go"]);
        fs::remove_file(dir.join("other.go")).unwrap();
    }
    /// Commits a new file with a comment, then removes it from the index and disk.
    fn commit_then_remove_new_file(dir: &Path) {
        let source = "package review\n\n// Counts the answers.\nfunc CountAnswers() {}\n";
        fs::write(dir.join("other.go"), source).unwrap();
        commit_all(dir);
        run_git(dir, &["rm", "--quiet", "other.go"]);
    }
    let cases = [
        ("only-in-the-index", stage_only as fn(&Path), STORE),
        ("only-in-head", commit_only, STORE),
        (
            "unnamed-older-wording-in-head",
            reword_unnamed_on_disk,
            STORE,
        ),
        (
            "file-with-no-decision-only-in-the-index",
            stage_then_delete_new_file,
            "other.go",
        ),
        (
            "file-with-no-decision-only-in-head",
            commit_then_remove_new_file,
            "other.go",
        ),
    ];

    for (case, change_copies, named_file) in cases {
        let repo = make_repo();
        let dir = repo.path();
        add_decision(dir, STORE, "SaveAnswer", "");
        change_copies(dir);

        let error = make_feedback_request_list(dir, None, &[]).unwrap_err();

        assert_eq!(error.code, ErrorCode::BadInput, "{case}");
        assert!(
            error.message.contains(named_file),
            "{case}: {}",
            error.message
        );
    }
}

/// `make_feedback_request_list` must refuse a file with a decision that does not parse, naming the file, even when the branch never changed it.
/// Otherwise a cut decided in another worktree could hide in it, and the Feedback Request List would come back empty.
#[test]
fn refuses_unparsable_file_with_a_decision() {
    let repo = make_repo();
    let dir = repo.path();
    let broken = format!("package review\n\n{APPROVED}func SaveAnswer( {{}}\n");
    fs::write(dir.join("old_broken.go"), broken).unwrap();
    commit_all(dir);
    add_decision(dir, "old_broken.go", "SaveAnswer", "");
    let base = resolve_commit(dir, "HEAD").unwrap();

    let error = make_feedback_request_list(dir, Some(base), &[]).unwrap_err();

    assert_eq!(error.code, ErrorCode::FileUnreadable);
    assert!(error.message.contains("old_broken.go"), "{}", error.message);
}

/// `make_feedback_request_list` must leave a saved Review Session's bytes and modified time as they were, in every status.
/// Otherwise building or refusing a Feedback Request List could rewrite the Review Session and lose User Feedback.
#[test]
fn never_changes_review_session() {
    let cases = [
        "waiting_for_user_feedback",
        "writing_user_feedback",
        "user_notes_waiting",
        "done",
    ];

    for status in cases {
        let repo = make_repo();
        let dir = repo.path();
        write_store(dir, REWORDED, "");
        let requests = json!([request_json(1, Some("SaveAnswer"), REWORDED, 3)]);
        save_review(
            dir,
            status,
            &[requests],
            json!({"1": {"note": "Say which lock."}}),
        );
        let review_file = File::options()
            .write(true)
            .open(dir.join(".git/pde/review.json"));
        review_file.unwrap().set_modified(UNIX_EPOCH).unwrap();
        let before = read_review_file(dir);

        let _ = make_feedback_request_list(dir, None, &[]);

        assert_eq!(read_review_file(dir), before, "{status}");
    }
}

/// `make_feedback_request_list` must leave a missing Review Session missing.
/// Otherwise a Review Session could start before any Feedback Request List is sent.
#[test]
fn never_starts_review_session() {
    let repo = make_repo();
    let dir = repo.path();
    write_store(dir, REWORDED, "");

    let _ = make_feedback_request_list(dir, None, &[]);

    assert_eq!(read_review_file(dir), None);
}

/// `make_feedback_request_list` must resend each noted request whose comment is still there once, under its ID with the comment's current text.
/// Otherwise the user would answer a User Note's request with old text, under a new ID, or twice.
#[test]
fn resent_requests_keep_ids_and_current_text() {
    let repo = make_repo();
    let dir = repo.path();
    let first_list = json!([
        request_json(1, Some("SaveAnswer"), "// Saves the answer.\n", 3),
        request_json(2, None, "\t// Takes the lock.\n", 5),
        request_json(3, None, "\t// Writes the answer.\n", 8),
        request_json(4, None, "\t// Unlocks the store.\n", 10),
        request_json(5, Some("CountAnswers"), "// Counts the answers.\n", 14),
        request_json(6, None, "\t// Retries once.\n", 17),
        request_json(7, None, "\t// Retries once.\n", 19),
    ]);
    // The second list sent one noted request again with new text.
    let second_list = json!([request_json(2, None, "\t// Holds the lock.\n", 5)]);
    let note = json!({"note": "Say which lock."});
    let feedback = json!({
        "1": note, "2": note, "3": note, "4": {"answer": "keep"},
        "5": note, "6": note, "7": note,
    });
    save_review(
        dir,
        "user_notes_waiting",
        &[first_list, second_list],
        feedback,
    );
    let current = concat!(
        "package review\n",
        "\n",
        "// Saves the answer under the review lock.\n",
        "func SaveAnswer() {\n",
        // A new line above moves the noted comment down by one.
        "\tdefer unlock()\n",
        "\t// Holds the lock.\n",
        "\tlock()\n",
        // A noted comment reworded in place to the kept comment's text.
        "\t// Unlocks the store.\n",
        "\twrite()\n",
        "\t// Unlocks the store.\n",
        "\tunlock()\n",
        "}\n",
        "\n",
        "func CountAnswers() {}\n",
        "\n",
        // Two noted comments with equal text, after the agent deleted one copy.
        "func RetryAnswer() {\n",
        "\t// Retries once.\n",
        "\tretry()\n",
        "\tretry()\n",
        "}\n",
    );
    fs::write(dir.join(STORE), current).unwrap();

    let output = make_feedback_request_list(dir, None, &[]).unwrap();

    let requests = output.result.list.requests;
    let listed: Vec<String> = requests.iter().map(describe_with_id).collect();
    assert_eq!(
        listed,
        [
            "1 3 review_store.go SaveAnswer new",
            "2 6 review_store.go (unnamed) new",
            "3 8 review_store.go (unnamed) new",
            "6 17 review_store.go (unnamed) new",
        ]
    );
    assert_eq!(
        requests[0].text,
        "// Saves the answer under the review lock.\n"
    );
    assert_eq!(requests[2].text, "\t// Unlocks the store.\n");
}

/// `make_feedback_request_list` must resend a Feedback Request List in file and line order, with noted and new requests mixed.
/// Otherwise the user would get the comments out of file order.
#[test]
fn resends_in_file_and_line_order() {
    let repo = make_repo();
    let dir = repo.path();
    let requests = json!([request_json(1, Some("SaveAnswer"), APPROVED, 3)]);
    save_review(
        dir,
        "user_notes_waiting",
        &[requests],
        json!({"1": {"note": "Say which lock."}}),
    );
    let source = format!(
        "package review\n\n// Deletes an answer.\nfunc DeleteAnswer() {{}}\n\n{APPROVED}func SaveAnswer() {{}}\n"
    );
    fs::write(dir.join(STORE), source).unwrap();

    let output = make_feedback_request_list(dir, None, &[]).unwrap();

    let listed: Vec<String> = output
        .result
        .list
        .requests
        .iter()
        .map(describe_with_id)
        .collect();
    assert_eq!(
        listed,
        [
            "2 3 review_store.go DeleteAnswer new",
            "1 6 review_store.go SaveAnswer new",
        ]
    );
}

/// `make_feedback_request_list` must list each comment written while User Notes wait under a new ID after the highest one sent.
/// Otherwise a comment the agent wrote while answering User Notes would skip the user, or take an ID already used.
#[test]
fn gives_later_comments_new_ids_during_user_notes() {
    let repo = make_repo();
    let dir = repo.path();
    let requests = json!([
        request_json(1, Some("SaveAnswer"), APPROVED, 3),
        request_json(2, None, "\t/* Takes it. */", 5),
        request_json(3, None, "\t// Flushes the store.\n", 6),
    ]);
    let keep = json!({"answer": "keep"});
    let feedback = json!({"1": {"note": "Say which lock."}, "2": keep, "3": keep});
    save_review(dir, "user_notes_waiting", &[requests], feedback);
    let current = concat!(
        "package review\n",
        "\n",
        "// Saves the answer under the lock.\n",
        "func SaveAnswer() {\n",
        // A new comment on the same line as a kept one.
        "\t/* Takes it. */ lock() // Never unlocks.\n",
        "\t// Flushes the store.\n",
        "\tflush()\n",
        // A new copy of the kept comment's text.
        "\t// Flushes the store.\n",
        "\tflush()\n",
        "}\n",
        "\n",
        "// Deletes an answer.\n",
        "func DeleteAnswer() {}\n",
    );
    fs::write(dir.join(STORE), current).unwrap();

    let output = make_feedback_request_list(dir, None, &[]).unwrap();

    let listed: Vec<String> = output
        .result
        .list
        .requests
        .iter()
        .map(describe_with_id)
        .collect();
    assert_eq!(
        listed,
        [
            "1 3 review_store.go SaveAnswer new",
            "4 5 review_store.go (unnamed) new",
            "5 8 review_store.go (unnamed) new",
            "6 12 review_store.go DeleteAnswer new",
        ]
    );
}

/// `make_feedback_request_list` must ask again about an answered comment reworded since, and not about one left as it was.
/// Otherwise User Feedback on the old text would be written over the new text, or the user would answer one comment twice.
#[test]
fn asks_again_only_about_reworded_comments() {
    let repo = make_repo();
    let dir = repo.path();
    let requests = json!([
        request_json(1, Some("SaveAnswer"), APPROVED, 3),
        request_json(2, Some("CountAnswers"), "// Counts the answers.\n", 6),
        request_json(3, Some("LoadAnswers"), "// Loads every answer.\n", 9),
        request_json(4, None, "\t// Reads the list.\n", 13),
    ]);
    let keep = json!({"answer": "keep"});
    let feedback = json!({"1": {"note": "Say which lock."}, "2": keep, "3": keep, "4": keep});
    save_review(dir, "user_notes_waiting", &[requests], feedback);
    let current = concat!(
        "package review\n",
        "\n",
        "// Saves the answer under the lock.\n",
        "func SaveAnswer() {}\n",
        "\n",
        "// Counts the answers.\n",
        "func CountAnswers() {}\n",
        "\n",
        // Two kept comments reworded since the list was sent.
        "// Loads every answer and deletes every backup.\n",
        "func LoadAnswers() {}\n",
        "\n",
        "func ReadList() {\n",
        "\t// Reads and clears the list.\n",
        "\tread()\n",
        "}\n",
    );
    fs::write(dir.join(STORE), current).unwrap();

    let output = make_feedback_request_list(dir, None, &[]).unwrap();

    let listed: Vec<String> = output
        .result
        .list
        .requests
        .iter()
        .map(describe_with_id)
        .collect();
    assert_eq!(
        listed,
        [
            "1 3 review_store.go SaveAnswer new",
            "5 9 review_store.go LoadAnswers new",
            "6 13 review_store.go (unnamed) new",
        ]
    );
}

/// `make_feedback_request_list` must use the Review Session's saved base when User Notes wait and no base is given.
/// Otherwise a stacked branch would be checked from the branch start and list its parent's comments.
#[test]
fn uses_saved_base_when_none_is_given() {
    let repo = make_repo();
    let dir = repo.path();
    let parent = format!(
        "{}\n// Lists every review.\nfunc ListReviews() {{}}\n",
        store_source(APPROVED, "")
    );
    fs::write(dir.join(STORE), parent).unwrap();
    commit_all(dir);
    let requests = json!([request_json(1, Some("SaveAnswer"), APPROVED, 3)]);
    save_review(
        dir,
        "user_notes_waiting",
        &[requests],
        json!({"1": {"note": "Say which lock."}}),
    );

    let output = make_feedback_request_list(dir, None, &[]).unwrap();

    let listed: Vec<String> = output
        .result
        .list
        .requests
        .iter()
        .map(describe_with_id)
        .collect();
    assert_eq!(listed, ["1 3 review_store.go SaveAnswer new"]);
}

/// `make_feedback_request_list` must list a named comment the disk copy asks about even when HEAD holds an older wording.
/// Otherwise an agent would have to commit before each Feedback Request List whenever it rewords a committed comment.
#[test]
fn lists_named_comment_with_older_committed_text() {
    let repo = make_repo();
    let dir = repo.path();
    add_decision(dir, STORE, "SaveAnswer", "");
    write_store(dir, APPROVED, "return");
    commit_all(dir);
    write_store(dir, REWORDED, "return");

    let output = make_feedback_request_list(dir, None, &[]).unwrap();

    let listed: Vec<String> = output.result.list.requests.iter().map(describe).collect();
    assert_eq!(listed, ["review_store.go SaveAnswer was_cut"]);
}

/// `make_feedback_request_list` must add one Feedback Request for an approved comment named twice, with the approved text from disk.
/// Otherwise the user could see text other than what they approved, or two pieces of User Feedback would be written into one comment.
#[test]
fn change_request_adds_one_request_per_comment() {
    let repo = make_repo();
    let dir = repo.path();
    add_decision(dir, STORE, "SaveAnswer", APPROVED);
    let change_request = || ChangeRequest {
        file: PathBuf::from(STORE),
        name: "SaveAnswer".to_owned(),
    };

    let output = make_feedback_request_list(dir, None, &[change_request(), change_request()]);

    let requests = output.unwrap().result.list.requests;
    let listed: Vec<String> = requests.iter().map(describe).collect();
    assert_eq!(listed, ["review_store.go SaveAnswer agent_wants_change"]);
    assert_eq!(requests[0].text, APPROVED);
}

/// `make_feedback_request_list` must refuse a change request unless it names an approved comment the user can reword.
/// Otherwise a misspelled request would vanish, or the user would be asked to reword a comment they cut or can only keep or cut.
#[test]
fn change_request_needs_approved_rewordable_comment() {
    let block = "/* Saves the answer. */\n";
    let cases = [
        ("misspelled-name", APPROVED, APPROVED, "SaveAnswers"),
        ("cut-comment", APPROVED, "", "SaveAnswer"),
        ("keep-or-cut-only-comment", block, block, "SaveAnswer"),
    ];

    for (case, comment, decision, name) in cases {
        let repo = make_repo();
        let dir = repo.path();
        write_store(dir, comment, "");
        add_decision(dir, STORE, "SaveAnswer", decision);
        let change_request = ChangeRequest {
            file: PathBuf::from(STORE),
            name: name.to_owned(),
        };

        let error = make_feedback_request_list(dir, None, &[change_request]).unwrap_err();

        assert_eq!(error.code, ErrorCode::BadInput, "{case}");
    }
}
