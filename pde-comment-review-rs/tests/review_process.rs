mod common;

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use common::TempFolder;
use pde_comment_review::shared::ErrorCode;
use pde_comment_review::shared::approvals::{self, Approval};
use pde_comment_review::shared::comment_rules::{Finding, ReportNote, Rule};
use pde_comment_review::shared::git::resolve_commit;
use pde_comment_review::shared::review_session::FeedbackRequest;
use pde_comment_review::steps::NextStep;
use pde_comment_review::steps::check_comments::check_comments;
use pde_comment_review::steps::make_feedback_request_list::{
    ChangeRequest, make_feedback_request_list,
};
use pde_comment_review::steps::show_status::{Waiting, show_status};
use serde_json::{Value, json};

const STORE: &str = "review_store.go";
const APPROVED: &str = "// Saves the answer under the lock.\n";
const REWORDED: &str = "// Writes the answer while holding the lock.\n";
const BROKEN_GO: &str = "package review\n\nfunc SaveAnswer( {}\n";
const WAITING_REVIEW: &str = r#"{"status": "waiting_for_user_feedback", "origin": {"repo": "personal-dev-env", "branch": "review-comments", "agent_session": "ses_123"}, "lists": [], "feedback": {}}"#;

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

/// Makes main hold one Go file with one comment, then starts a branch from it.
fn make_repo() -> TempFolder {
    let folder = TempFolder::new();
    let dir = folder.path();
    run_git(dir, &["init", "-b", "main"]);
    write_store(dir, APPROVED, "");
    commit_all(dir);
    run_git(dir, &["checkout", "-b", "review-comments"]);
    folder
}

fn store_source(comment: &str, body: &str) -> String {
    format!("package review\n\n{comment}func SaveAnswer() {{{body}}}\n")
}

fn write_store(dir: &Path, comment: &str, body: &str) {
    fs::write(dir.join(STORE), store_source(comment, body)).unwrap();
}

fn commit_all(dir: &Path) {
    run_git(dir, &["add", "--all"]);
    run_git(dir, &["commit", "--allow-empty", "-m", "Change the store"]);
}

fn add_decision(dir: &Path, file: &str, name: &str, text: &str) {
    let decision = Approval {
        file: PathBuf::from(file),
        name: name.to_owned(),
        text: text.to_owned(),
    };
    approvals::add(dir, &[decision]).unwrap();
}

/// Commits `before` in a file, adds one decision on `name`, writes `after`,
/// and returns step 2's failures.
fn check_removal(
    file: &str,
    name: &str,
    decision: &str,
    before: &str,
    after: &str,
) -> Vec<Finding> {
    let repo = make_repo();
    let dir = repo.path();
    fs::write(dir.join(file), before).unwrap();
    commit_all(dir);
    let base = resolve_commit(dir, "HEAD").unwrap();
    add_decision(dir, file, name, decision);
    fs::write(dir.join(file), after).unwrap();
    check_comments(dir, Some(base)).unwrap().result.failures
}

/// Saves a review in `status` that sent each of `lists` from HEAD, with `feedback` by request ID.
fn save_review(dir: &Path, status: &str, lists: &[Value], feedback: Value) {
    let base = resolve_commit(dir, "HEAD").unwrap();
    let lists: Vec<Value> = lists
        .iter()
        .map(|requests| json!({"base": base.as_str(), "requests": requests}))
        .collect();
    let review = json!({
        "status": status,
        "origin": {"repo": "personal-dev-env", "branch": "review-comments", "agent_session": "ses_123"},
        "lists": lists,
        "feedback": feedback,
    });
    fs::create_dir_all(dir.join(".git/pde")).unwrap();
    fs::write(dir.join(".git/pde/review.json"), review.to_string()).unwrap();
}

/// Returns the saved review's modified time and bytes, or `None` when it is missing.
fn read_review_file(dir: &Path) -> Option<(SystemTime, Vec<u8>)> {
    let path = dir.join(".git/pde/review.json");
    let bytes = fs::read(&path).ok()?;
    Some((fs::metadata(&path).unwrap().modified().unwrap(), bytes))
}

fn request_json(id: u32, name: Option<&str>, text: &str, line: usize) -> Value {
    json!({
        "id": id,
        "file": STORE,
        "name": name,
        "reason": "new",
        "text": text,
        "can_reword": true,
        "line": line,
        "suggestion": {"kind": "keep", "reasoning": "Says what it does.", "also_at": []},
    })
}

/// Describes a request by its file, name and reason, for short assertions.
fn describe(request: &FeedbackRequest) -> String {
    let name = request.name.as_deref().unwrap_or("(unnamed)");
    let reason = serde_json::to_value(request.reason).unwrap();
    let reason = reason.as_str().unwrap();
    format!("{} {name} {reason}", request.file.display())
}

/// Returns every folder and file below `folder`, with each one's modified time and each file's bytes.
fn read_every_file(folder: &Path) -> BTreeMap<PathBuf, (SystemTime, Option<Vec<u8>>)> {
    let mut found = BTreeMap::new();
    let mut folders = vec![folder.to_path_buf()];
    while let Some(folder) = folders.pop() {
        for entry in fs::read_dir(&folder).unwrap() {
            let path = entry.unwrap().path();
            let modified = fs::metadata(&path).unwrap().modified().unwrap();
            if path.is_dir() {
                folders.push(path.clone());
                found.insert(path, (modified, None));
            } else {
                let bytes = fs::read(&path).unwrap();
                found.insert(path, (modified, Some(bytes)));
            }
        }
    }
    found
}

/// Without it, an agent could reword text the user approved, committed, staged or not, from any folder, in a file Git does not list, or behind a restored copy.
#[test]
fn changed_approved_text_fails_check() {
    let commit: &[&str] = &["commit", "--all", "-m", "Reword the store"];
    let restore_disk: &[&str] = &["restore", "--source=main", "--worktree", "--", STORE];
    let restore_disk_and_index: &[&str] = &[
        "restore",
        "--source=main",
        "--staged",
        "--worktree",
        "--",
        STORE,
    ];
    let cases: [(&str, &[&[&str]], &str); 6] = [
        ("committed", &[commit], ""),
        ("uncommitted", &[], ""),
        ("uncommitted-from-subfolder", &[], "docs"),
        (
            "hidden-from-git-diff",
            &[&["update-index", "--skip-worktree", STORE]],
            "",
        ),
        (
            "committed-then-restored-on-disk-and-staged",
            &[commit, restore_disk_and_index],
            "",
        ),
        (
            "staged-then-restored-on-disk",
            &[&["add", STORE], restore_disk],
            "",
        ),
    ];

    for (case, git_commands, folder) in cases {
        let repo = make_repo();
        let dir = repo.path();
        add_decision(dir, STORE, "SaveAnswer", APPROVED);
        write_store(dir, REWORDED, "");
        for git_args in git_commands {
            run_git(dir, git_args);
        }
        fs::create_dir_all(dir.join(folder)).unwrap();

        let output = check_comments(&dir.join(folder), None).unwrap();

        let failure = Finding {
            file: PathBuf::from(STORE),
            name: Some("SaveAnswer".to_owned()),
            line: 3,
            rule: Rule::Approved,
            note: None,
            detail: None,
        };
        assert_eq!(output.result.failures, [failure], "{case}");
        assert_eq!(output.next, NextStep::CheckComments, "{case}");
    }
}

/// Without it, an agent would be stuck on a decision from another worktree, on a new comment for a name the user cut, or on `\r\n` line ends.
#[test]
fn unchanged_or_cut_name_comments_pass_check() {
    // Each branch changes the file, so step 2 reads it.
    let cases = [
        (
            "approved-elsewhere-unchanged",
            REWORDED,
            store_source(APPROVED, "return"),
        ),
        ("cut-then-reworded", "", store_source(REWORDED, "")),
        (
            "crlf-line-ends",
            APPROVED,
            store_source(APPROVED, "").replace('\n', "\r\n"),
        ),
    ];

    for (case, decision, source) in cases {
        let repo = make_repo();
        let dir = repo.path();
        add_decision(dir, STORE, "SaveAnswer", decision);
        fs::write(dir.join(STORE), source).unwrap();

        let output = check_comments(dir, None).unwrap();

        assert_eq!(output.result.failures, [], "{case}");
        assert_eq!(output.next, NextStep::MakeFeedbackRequestList, "{case}");
    }
}

/// Without it, an agent could delete an approved comment, docstring or module comment while the code it described stays.
#[test]
fn removed_approved_comment_fails_while_code_stays() {
    let docstring = "        \"\"\"Saves the answer.\"\"\"\n";
    let python_before =
        format!("class Review:\n    def save(self):\n{docstring}        return 1\n");
    let python_after = "class Review:\n    def save(self):\n        return 1\n";
    let module_comment = "//! Lists each folder.\n";
    let rust_before = format!("{module_comment}\npub mod shared;\n");
    let cases = [
        (
            "go-comment-removed",
            STORE,
            "SaveAnswer",
            APPROVED,
            store_source(APPROVED, ""),
            store_source("", ""),
            3,
        ),
        (
            "python-docstring-removed",
            "review.py",
            "Review.save",
            docstring,
            python_before,
            python_after.to_owned(),
            2,
        ),
        (
            "rust-module-comment-removed",
            "lib.rs",
            "<module>",
            module_comment,
            rust_before,
            "pub mod shared;\n".to_owned(),
            1,
        ),
    ];

    for (case, file, name, decision, before, after, line) in cases {
        let failures = check_removal(file, name, decision, &before, &after);

        let failure = Finding {
            file: PathBuf::from(file),
            name: Some(name.to_owned()),
            line,
            rule: Rule::ApprovedRemoved,
            note: None,
            detail: None,
        };
        assert_eq!(failures, [failure], "{case}");
    }
}

/// Without it, an agent would be stuck after deleting a function with its approved comment, or a comment the user cut.
#[test]
fn removing_code_or_cut_comment_passes_check() {
    let cases = [
        ("code-removed-too", APPROVED, "package review\n".to_owned()),
        ("cut-comment-removed", "", store_source("", "")),
    ];

    for (case, decision, after) in cases {
        let failures = check_removal(
            STORE,
            "SaveAnswer",
            decision,
            &store_source(APPROVED, ""),
            &after,
        );

        assert_eq!(failures, [], "{case}");
    }
}

/// Without it, a branch made from another would fail on its parent's changes, which its agent must not undo.
#[test]
fn stacked_branch_checks_only_its_own_changes() {
    let repo = make_repo();
    let dir = repo.path();
    add_decision(dir, STORE, "SaveAnswer", APPROVED);
    write_store(dir, REWORDED, "");
    commit_all(dir);
    let parent = resolve_commit(dir, "HEAD").unwrap();
    run_git(dir, &["checkout", "-b", "stacked"]);

    let from_main = check_comments(dir, None).unwrap();
    let from_parent = check_comments(dir, Some(parent)).unwrap();

    assert_eq!(from_main.result.failures.len(), 1);
    assert_eq!(from_parent.result.failures, []);
}

/// Without it, a base from a sibling branch that already holds the branch's reword would hide it.
#[test]
fn refuses_base_that_is_not_an_ancestor() {
    let repo = make_repo();
    let dir = repo.path();
    add_decision(dir, STORE, "SaveAnswer", APPROVED);
    run_git(dir, &["checkout", "-b", "sibling", "main"]);
    write_store(dir, REWORDED, "");
    commit_all(dir);
    let sibling = resolve_commit(dir, "HEAD").unwrap();
    run_git(dir, &["checkout", "review-comments"]);
    write_store(dir, REWORDED, "");

    let error = check_comments(dir, Some(sibling)).unwrap_err();

    assert_eq!(error.code, ErrorCode::BadInput);
}

/// Without it, a changed file that does not parse could hide a changed approved comment, and a Markdown change would block the branch.
#[test]
fn lists_unsupported_and_refuses_changed_unparsable_files() {
    let repo = make_repo();
    let dir = repo.path();
    fs::write(dir.join("README.md"), "Notes on the store.\n").unwrap();

    let output = check_comments(dir, None).unwrap();

    assert_eq!(output.result.unsupported, [PathBuf::from("README.md")]);

    fs::write(dir.join("broken.go"), BROKEN_GO).unwrap();

    let error = check_comments(dir, None).unwrap_err();

    assert_eq!(error.code, ErrorCode::FileUnreadable);
    assert!(error.message.contains("broken.go"), "{}", error.message);
}

/// Without it, an approved file the branch never changed that does not parse would block the branch, once its time stamp or line ends change.
#[test]
fn unchanged_broken_approved_file_passes_check() {
    fn change_time_stamp(dir: &Path) {
        let file = File::options().write(true).open(dir.join("old_broken.go"));
        file.unwrap().set_modified(UNIX_EPOCH).unwrap();
    }
    fn write_crlf_line_ends(dir: &Path) {
        run_git(dir, &["config", "core.autocrlf", "true"]);
        let source = BROKEN_GO.replace('\n', "\r\n");
        fs::write(dir.join("old_broken.go"), source).unwrap();
    }
    let cases = [
        ("time-stamp-changed", change_time_stamp as fn(&Path)),
        ("crlf-on-disk", write_crlf_line_ends),
    ];

    for (case, touch) in cases {
        let repo = make_repo();
        let dir = repo.path();
        fs::write(dir.join("old_broken.go"), BROKEN_GO).unwrap();
        commit_all(dir);
        add_decision(dir, "old_broken.go", "SaveAnswer", APPROVED);
        touch(dir);

        let output = check_comments(dir, Some(resolve_commit(dir, "HEAD").unwrap()));

        let failures = output.map(|output| output.result.failures);
        assert_eq!(failures.map_err(|error| error.code), Ok(vec![]), "{case}");
    }
}

/// Without it, a check could change the user's approvals, a waiting or broken review, or the code it reads.
#[test]
fn check_leaves_repository_files_unchanged() {
    let cases = [
        ("waiting-review", WAITING_REVIEW, false),
        ("broken-review", "{\"status\": ", false),
        ("broken-review-and-refused-check", "{\"status\": ", true),
    ];

    for (case, review, refuses) in cases {
        let repo = make_repo();
        let dir = repo.path();
        add_decision(dir, STORE, "SaveAnswer", APPROVED);
        // A reword staged and then undone on disk makes step 2 read the index too.
        write_store(dir, REWORDED, "");
        run_git(dir, &["add", STORE]);
        write_store(dir, APPROVED, "");
        fs::write(dir.join("README.md"), "Notes on the store.\n").unwrap();
        fs::write(dir.join(".git/pde/review.json"), review).unwrap();
        if refuses {
            fs::write(dir.join("broken.go"), BROKEN_GO).unwrap();
        }
        let before = read_every_file(dir);

        let result = check_comments(dir, None);

        assert_eq!(result.is_err(), refuses, "{case}");
        assert_eq!(read_every_file(dir), before, "{case}");
    }
}

mod make_feedback_request_list {
    use super::*;

    /// Without it, the user would miss a cut comment that came back or a decision made in another worktree, or would answer comments nobody changed.
    #[test]
    fn lists_only_new_and_conflicting_comments() {
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

    /// Without it, an agent could skip step 2 and slip its reword of approved text onto the list, where keeping it approves the reword.
    #[test]
    fn refuses_list_when_check_fails() {
        let repo = make_repo();
        let dir = repo.path();
        add_decision(dir, STORE, "SaveAnswer", APPROVED);
        write_store(dir, REWORDED, "");
        let change_request = ChangeRequest {
            file: PathBuf::from(STORE),
            name: "SaveAnswer".to_owned(),
        };

        let error = make_feedback_request_list(dir, None, &[change_request]).unwrap_err();

        assert_eq!(error.code, ErrorCode::CheckFailed);
    }

    /// Without it, a change request could show the user text other than what they approved, or leave two answers to write into one comment.
    #[test]
    fn change_request_lists_approved_comment_once() {
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

    /// Without it, a misspelled change request would vanish, and the user would never see the change the agent promised.
    #[test]
    fn refuses_change_request_for_unknown_name() {
        let repo = make_repo();
        let dir = repo.path();
        add_decision(dir, STORE, "SaveAnswer", APPROVED);
        let misspelled = ChangeRequest {
            file: PathBuf::from(STORE),
            name: "SaveAnswers".to_owned(),
        };

        let error = make_feedback_request_list(dir, None, &[misspelled]).unwrap_err();

        assert_eq!(error.code, ErrorCode::BadInput);
    }

    /// Without it, a new list could replace one the user is still answering, or run over a write that did not finish.
    #[test]
    fn refuses_list_while_review_waits_or_writes() {
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

    /// Without it, building or refusing a list could rewrite or start the saved review and lose the user's answers.
    #[test]
    fn list_never_writes_saved_review() {
        let cases = [
            (None, false),
            (Some("waiting_for_user_feedback"), true),
            (Some("writing_user_feedback"), true),
            (Some("user_notes_waiting"), false),
            (Some("done"), false),
        ];

        for (status, refuses) in cases {
            let repo = make_repo();
            let dir = repo.path();
            write_store(dir, REWORDED, "");
            if let Some(status) = status {
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
            }
            let before = read_review_file(dir);

            let result = make_feedback_request_list(dir, None, &[]);

            assert_eq!(result.is_err(), refuses, "{status:?}");
            assert_eq!(read_review_file(dir), before, "{status:?}");
        }
    }

    /// Without it, noted comments could come back with old text, new IDs, another comment's place or one comment twice, and new or reworded comments could skip the user or come from a parent branch.
    #[test]
    fn resends_noted_requests_and_changed_comments() {
        let repo = make_repo();
        let dir = repo.path();
        let parent = "package review\n\n// Lists every review.\nfunc ListReviews() {}\n";
        fs::write(dir.join(STORE), parent).unwrap();
        commit_all(dir);
        let first_list = json!([
            request_json(1, Some("SaveAnswer"), "// Saves the answer.\n", 13),
            request_json(2, None, "\t// Takes the lock.\n", 15),
            request_json(3, Some("LoadAnswers"), "// Loads every answer.\n", 24),
            request_json(4, Some("CountAnswers"), "// Counts the answers.\n", 27),
            request_json(5, None, "\t// Writes the answer.\n", 7),
            request_json(6, None, "\t// Unlocks the store.\n", 9),
            request_json(7, None, "\t// Closes the store.\n", 17),
            request_json(8, None, "\t// Flushes the store.\n", 20),
            request_json(9, None, "\t// Retries once.\n", 31),
            request_json(10, None, "\t// Retries once.\n", 33),
            request_json(11, None, "\t/* Takes it. */", 38),
            request_json(12, None, "\t// Reads the list.\n", 41),
        ]);
        let second_list = json!([request_json(2, None, "\t// Holds the lock.\n", 15)]);
        let note = json!({"note": "Say which lock."});
        let keep = json!({"answer": "keep"});
        let feedback = json!({
            "1": note, "2": note, "3": keep, "4": note,
            "5": note, "6": keep, "7": note, "8": keep,
            "9": note, "10": note, "11": keep, "12": keep,
        });
        save_review(
            dir,
            "user_notes_waiting",
            &[first_list, second_list],
            feedback,
        );
        let reworded = "// Saves the answer under the review lock.\n";
        let current = concat!(
            "package review\n",
            "\n",
            "// Lists every review.\n",
            "func ListReviews() {}\n",
            "\n",
            "func WriteAnswer() {\n",
            "\t// Unlocks the store.\n",
            "\twrite()\n",
            "\t// Unlocks the store.\n",
            "\tunlock()\n",
            "}\n",
            "\n",
            "// Saves the answer under the review lock.\n",
            "func SaveAnswer() {\n",
            "\tdefer unlock()\n",
            "\t// Holds the lock.\n",
            "\tlock()\n",
            "\t// Flushes the store.\n",
            "\tclose()\n",
            "\tflush()\n",
            "\t// Flushes the store.\n",
            "\tflush()\n",
            "}\n",
            "\n",
            "// Loads every answer and deletes every backup.\n",
            "func LoadAnswers() {}\n",
            "\n",
            "func CountAnswers() {}\n",
            "\n",
            "// Deletes an answer.\n",
            "func DeleteAnswer() {}\n",
            "\n",
            "func RetryAnswer() {\n",
            "\t// Retries once.\n",
            "\tretry()\n",
            "\tretry()\n",
            "}\n",
            "\n",
            "func HoldLock() {\n",
            "\t/* Takes it. */ lock() // Never unlocks.\n",
            "\t// Reads and clears the list.\n",
            "\tread()\n",
            "}\n",
        );
        fs::write(dir.join(STORE), current).unwrap();

        let output = make_feedback_request_list(dir, None, &[]).unwrap();

        let requests = output.result.list.requests;
        let listed: Vec<String> = requests
            .iter()
            .map(|request| {
                format!(
                    "{} {} {}",
                    json!(request.id),
                    request.line,
                    describe(request)
                )
            })
            .collect();
        assert_eq!(
            listed,
            [
                "5 7 review_store.go (unnamed) new",
                "1 13 review_store.go SaveAnswer new",
                "2 16 review_store.go (unnamed) new",
                "13 18 review_store.go (unnamed) new",
                "14 25 review_store.go LoadAnswers new",
                "15 30 review_store.go DeleteAnswer new",
                "9 34 review_store.go (unnamed) new",
                "16 40 review_store.go (unnamed) new",
                "17 41 review_store.go (unnamed) new",
            ]
        );
        assert_eq!(requests[0].text, "\t// Unlocks the store.\n");
        assert_eq!(requests[1].text, reworded);
    }

    /// Without it, a comment the user must answer could ship unasked from a commit or the index, even in a file with no decision, or an older committed wording would block the list.
    #[test]
    fn refuses_committed_or_staged_comment_disk_skips() {
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
        fn commit_only(dir: &Path) {
            write_store(dir, APPROVED, "return");
            commit_all(dir);
            write_store(dir, "", "return");
            run_git(dir, &["add", STORE]);
        }
        fn reword_on_disk(dir: &Path) {
            write_store(dir, APPROVED, "return");
            commit_all(dir);
            write_store(dir, REWORDED, "return");
        }
        fn reword_unnamed_on_disk(dir: &Path) {
            write_store(dir, "", "\n\t// Holds the lock.\n\treturn\n");
            commit_all(dir);
            write_store(dir, "", "\n\t// Holds the store lock.\n\treturn\n");
        }
        fn stage_then_delete_new_file(dir: &Path) {
            let source = "package review\n\n// Counts the answers.\nfunc CountAnswers() {}\n";
            fs::write(dir.join("other.go"), source).unwrap();
            run_git(dir, &["add", "other.go"]);
            fs::remove_file(dir.join("other.go")).unwrap();
        }
        fn commit_then_remove_new_file(dir: &Path) {
            let source = "package review\n\n// Counts the answers.\nfunc CountAnswers() {}\n";
            fs::write(dir.join("other.go"), source).unwrap();
            commit_all(dir);
            run_git(dir, &["rm", "--quiet", "other.go"]);
        }
        let cases = [
            (
                "only-in-the-index",
                stage_only as fn(&Path),
                STORE,
                Err((ErrorCode::BadInput, true)),
            ),
            (
                "only-in-head",
                commit_only,
                STORE,
                Err((ErrorCode::BadInput, true)),
            ),
            ("older-wording-in-head", reword_on_disk, STORE, Ok(1)),
            (
                "unnamed-older-wording-in-head",
                reword_unnamed_on_disk,
                STORE,
                Err((ErrorCode::BadInput, true)),
            ),
            (
                "file-with-no-decision-only-in-the-index",
                stage_then_delete_new_file,
                "other.go",
                Err((ErrorCode::BadInput, true)),
            ),
            (
                "file-with-no-decision-only-in-head",
                commit_then_remove_new_file,
                "other.go",
                Err((ErrorCode::BadInput, true)),
            ),
        ];

        for (case, change_copies, named_file, expected) in cases {
            let repo = make_repo();
            let dir = repo.path();
            add_decision(dir, STORE, "SaveAnswer", "");
            change_copies(dir);

            let result = make_feedback_request_list(dir, None, &[]);

            let result = result
                .map(|output| output.result.list.requests.len())
                .map_err(|error| (error.code, error.message.contains(named_file)));
            assert_eq!(result, expected, "{case}");
        }
    }

    /// Without it, a cut decided in another worktree could hide in an approved file that no longer parses, and the list would come back empty.
    #[test]
    fn refuses_approved_file_that_does_not_parse() {
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

    /// Without it, a notes round given a later base could skip a comment committed during the round.
    #[test]
    fn refuses_notes_round_with_another_base() {
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
}

mod show_status {
    use super::*;

    /// Without it, an agent could call the wrong step next, or miss the requests and notes the review waits on.
    #[test]
    fn names_next_step_and_what_waits() {
        let requests = || {
            json!([
                request_json(1, Some("SaveAnswer"), APPROVED, 3),
                request_json(2, None, "\t// Holds the lock.\n", 5),
            ])
        };
        let keep = || json!({"answer": "keep"});
        let cases = [
            ("no-review", None, NextStep::CheckComments, json!(null)),
            (
                "waiting",
                Some(("waiting_for_user_feedback", json!({"1": keep()}))),
                NextStep::TellUserAndWait,
                json!([2]),
            ),
            (
                "notes",
                Some((
                    "user_notes_waiting",
                    json!({"1": {"note": "Say which lock."}, "2": keep()}),
                )),
                NextStep::MakeFeedbackRequestList,
                json!({"1": "Say which lock."}),
            ),
            (
                "writing",
                Some(("writing_user_feedback", json!({"1": keep(), "2": keep()}))),
                NextStep::TellUserAndWait,
                json!(null),
            ),
            (
                "done",
                Some(("done", json!({"1": keep(), "2": keep()}))),
                NextStep::CheckComments,
                json!(null),
            ),
        ];

        for (case, review, next, waiting) in cases {
            let repo = make_repo();
            let dir = repo.path();
            if let Some((status, feedback)) = review {
                save_review(dir, status, &[requests()], feedback);
            }

            let output = show_status(dir).unwrap();

            let shown = match output.result.waiting {
                Waiting::Nothing => json!(null),
                Waiting::FeedbackRequests(ids) => json!(ids),
                Waiting::UserNotes(notes) => json!(notes),
            };
            assert_eq!(output.next, next, "{case}");
            assert_eq!(shown, waiting, "{case}");
        }
    }
}

/// Runs step 2's style rules on files a branch adds or changes.
mod style_rules {
    use super::*;

    /// Writes `source` to a file the branch adds, with `blob` on the avoid list.
    /// Returns step 2's failures and reported findings as rules and notes.
    fn check_new_file(file: &str, source: &str) -> Vec<(Rule, Option<ReportNote>, String)> {
        let repo = make_repo();
        let dir = repo.path();
        add_avoided_word(dir);
        fs::write(dir.join(file), source).unwrap();
        let output = check_comments(dir, None).unwrap();
        list_rules(output.result.failures, output.result.reported)
    }

    fn add_avoided_word(dir: &Path) {
        fs::create_dir_all(dir.join(".git/pde")).unwrap();
        let approvals = r#"{"approved": [], "avoid": {"blob": "say file"}}"#;
        fs::write(dir.join(".git/pde/code-comments.json"), approvals).unwrap();
    }

    fn list_rules(
        failures: Vec<Finding>,
        reported: Vec<Finding>,
    ) -> Vec<(Rule, Option<ReportNote>, String)> {
        failures
            .into_iter()
            .chain(reported)
            .map(|finding| {
                (
                    finding.rule,
                    finding.note,
                    finding.detail.unwrap_or_default(),
                )
            })
            .collect()
    }

    /// Without it, filler, long comments or unclear names an agent writes would reach the user unflagged.
    #[test]
    fn each_style_rule_flags_its_bad_sample() {
        let go = |code: &str| format!("package review\n\n{code}");
        let cases = [
            (
                "filler",
                "answer.go",
                go("// This facilitates saving.\nfunc SaveAnswer() {}\n"),
                vec![(Rule::Filler, None, "\"facilitates\"".to_owned())],
            ),
            (
                "avoided-word",
                "answer.go",
                go("// Saves the blob.\nfunc SaveAnswer() {}\n"),
                vec![(Rule::AvoidedWord, None, "\"blob\": say file".to_owned())],
            ),
            (
                "restated-name",
                "answer.go",
                go("// Writes the answer, as SaveAnswer says.\nfunc SaveAnswer() {}\n"),
                vec![(
                    Rule::RestatedName,
                    Some(ReportNote::ReportOnly),
                    String::new(),
                )],
            ),
            (
                "starts-with-name",
                "answer.go",
                go("// SaveAnswer writes it.\nfunc SaveAnswer() {}\n"),
                vec![
                    (Rule::StartsWithName, None, String::new()),
                    (
                        Rule::RestatedName,
                        Some(ReportNote::ReportOnly),
                        String::new(),
                    ),
                ],
            ),
            (
                "comment-length",
                "answer.go",
                go("// Saves it. Locks it. Logs it.\nfunc SaveAnswer() {}\n"),
                vec![(Rule::CommentLength, None, "3 sentences".to_owned())],
            ),
            (
                "comment-colon",
                "answer.go",
                go("// Saves it: now.\nfunc SaveAnswer() {}\n"),
                vec![(Rule::CommentColon, None, String::new())],
            ),
            (
                "seven-words",
                "answer.go",
                go("func SaveTheAnswerToTheStoreRightNow() {}\n"),
                vec![(Rule::SevenWords, None, "8 words".to_owned())],
            ),
            (
                "verb-first",
                "answer.go",
                go("func Total() {}\n"),
                vec![(
                    Rule::VerbFirst,
                    Some(ReportNote::ReportOnly),
                    "\"Total\" is not in the checker's verbs".to_owned(),
                )],
            ),
            (
                "docstring",
                "answer.py",
                "def save_answer():\n    return 1\n".to_owned(),
                vec![(Rule::Docstring, None, String::new())],
            ),
            (
                "summary-line",
                "answer.py",
                "def save_answer():\n    \"\"\"Saves\n    the answer.\"\"\"\n".to_owned(),
                vec![(
                    Rule::SummaryLine,
                    None,
                    "summary runs onto a second line".to_owned(),
                )],
            ),
            (
                "test-why",
                "answer.py",
                "def test_saves_answer():\n    \"\"\"Saves the answer.\"\"\"\n".to_owned(),
                vec![(Rule::TestWhy, None, String::new())],
            ),
            (
                "section-heading",
                "answer.py",
                "def save_answer():\n    \"\"\"Saves the answer.\n\n    Why: it is lost.\n    \"\"\"\n"
                    .to_owned(),
                vec![(Rule::SectionHeading, None, "\"Why:\"".to_owned())],
            ),
            (
                "python-comment-with-example",
                "answer.py",
                "# Saves it. Logs it.\n#\n#   example: 1\n#   example: 2\nLIMIT = 1\n".to_owned(),
                vec![
                    (Rule::CommentLength, None, "3 sentences".to_owned()),
                    (Rule::CommentColon, None, String::new()),
                ],
            ),
            (
                "rust-filler",
                "lib.rs",
                "/// This facilitates saving.\npub fn save_answer() {}\n".to_owned(),
                vec![(Rule::Filler, None, "\"facilitates\"".to_owned())],
            ),
        ];

        for (case, file, source, expected) in cases {
            assert_eq!(check_new_file(file, &source), expected, "{case}");
        }
    }

    /// Without it, an agent would be blocked by text it may not change, the user's, an earlier author's or a license's.
    #[test]
    fn report_notes_mark_approved_old_and_header_text() {
        let filler = "// This facilitates saving.\n";
        let cases = [
            (
                "approved-text",
                APPROVED,
                store_source(filler, ""),
                filler,
                (
                    Rule::Filler,
                    Some(ReportNote::Approved),
                    "\"facilitates\"".to_owned(),
                ),
            ),
            (
                "sentence-at-base",
                filler,
                store_source("// This facilitates saving. It holds the lock.\n", ""),
                "",
                (
                    Rule::Filler,
                    Some(ReportNote::Existing),
                    "\"facilitates\"".to_owned(),
                ),
            ),
            (
                "license-header",
                APPROVED,
                format!("// One. Two. Three.\n\n{}", store_source(APPROVED, "")),
                "",
                (
                    Rule::CommentLength,
                    Some(ReportNote::Header),
                    "3 sentences".to_owned(),
                ),
            ),
        ];

        for (case, base_comment, source, decision, expected) in cases {
            let repo = make_repo();
            let dir = repo.path();
            write_store(dir, base_comment, "");
            commit_all(dir);
            let base = resolve_commit(dir, "HEAD").unwrap();
            if !decision.is_empty() {
                add_decision(dir, STORE, "SaveAnswer", decision);
            }
            fs::write(dir.join(STORE), source).unwrap();

            let output = check_comments(dir, Some(base)).unwrap();

            assert_eq!(output.result.failures, [], "{case}");
            let reported = list_rules(Vec::new(), output.result.reported);
            assert_eq!(reported, [expected], "{case}");
            assert_eq!(output.next, NextStep::MakeFeedbackRequestList, "{case}");
        }
    }

    /// Without it, touching or moving an old file would wrongly flag its untouched comments and names. The same holds for generated code and a `# noqa` mark the agent must keep.
    #[test]
    fn unchanged_moved_generated_or_noqa_text_passes() {
        let old_comment = "// This facilitates saving.\n";
        let long_name = "package review\n\n// This facilitates saving.\nfunc SaveTheAnswerToTheStoreRightNow() {}\n";
        let generated = "// Code generated by protoc. DO NOT EDIT.\n\npackage review\n\n// This facilitates saving.\nfunc SaveAnswer() {}\n";
        let cases = [
            (
                "old-comment-body-changed",
                STORE,
                store_source(old_comment, ""),
                STORE,
                store_source(old_comment, "return"),
            ),
            (
                "old-name-body-changed",
                STORE,
                long_name.to_owned(),
                STORE,
                long_name.replace("{}", "{ return }"),
            ),
            (
                "moved-to-another-file",
                "old_answer.go",
                long_name.to_owned(),
                "answer.go",
                long_name.to_owned(),
            ),
            (
                "generated-file",
                "",
                String::new(),
                "answer.go",
                generated.to_owned(),
            ),
            (
                "python-noqa-after-prose",
                "",
                String::new(),
                "answer.py",
                "LIMIT = 1  # Counts it.  # noqa: E501\n".to_owned(),
            ),
        ];

        for (case, old_file, before, file, after) in cases {
            let repo = make_repo();
            let dir = repo.path();
            if !old_file.is_empty() {
                fs::write(dir.join(old_file), before).unwrap();
                commit_all(dir);
                fs::remove_file(dir.join(old_file)).unwrap();
            }
            let base = resolve_commit(dir, "HEAD").unwrap();
            fs::write(dir.join(file), after).unwrap();

            let output = check_comments(dir, Some(base)).unwrap();

            assert_eq!(output.result.failures, [], "{case}");
        }
    }

    /// Without it, a test, getter, constructor or framework method would be told to start with a verb it must not have.
    #[test]
    fn verb_first_skips_names_a_language_requires() {
        let go = |code: &str| format!("package review\n\n{code}");
        let python = "import pytest\nfrom typing import override\n\n\nclass Store:\n    \"\"\"Holds answers.\"\"\"\n\n    def __init__(self):\n        \"\"\"Starts empty.\"\"\"\n\n    def setUp(self):\n        \"\"\"Prepares it.\"\"\"\n\n    def tearDown(self):\n        \"\"\"Cleans up.\"\"\"\n\n    @property\n    def total(self):\n        \"\"\"Counts answers.\"\"\"\n\n    @override\n    def ledger(self):\n        \"\"\"Lists answers.\"\"\"\n\n\n@pytest.fixture\ndef database():\n    \"\"\"Opens an empty one.\"\"\"\n";
        let not_a_verb = |word: &str| {
            let detail = format!("\"{word}\" is not in the checker's verbs");
            vec![(Rule::VerbFirst, Some(ReportNote::ReportOnly), detail)]
        };
        let cases = [
            (
                "go-constructor",
                "answer.go",
                go("func NewStore() {}\n"),
                vec![],
            ),
            (
                "go-getter",
                "answer.go",
                go("type Store struct{}\n\nfunc (s Store) Total() int { return 0 }\n"),
                vec![],
            ),
            (
                "go-test-and-benchmark",
                "answer_test.go",
                go("func TestSave(t *testing.T) {}\n\nfunc BenchmarkSave(b *testing.B) {}\n"),
                vec![],
            ),
            (
                "go-testify-needs-verb",
                "answer_test.go",
                go("func Testify() {}\n"),
                not_a_verb("Testify"),
            ),
            (
                "python-framework-names",
                "answer.py",
                python.to_owned(),
                vec![],
            ),
        ];

        for (case, file, source, expected) in cases {
            assert_eq!(check_new_file(file, &source), expected, "{case}");
        }
    }
}
