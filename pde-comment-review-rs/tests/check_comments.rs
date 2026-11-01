mod common;

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use common::{
    APPROVED, REWORDED, STORE, TempFolder, add_decision, commit_all, make_repo, run_git,
    store_source, write_store,
};
use pde_comment_review::shared::ErrorCode;
use pde_comment_review::shared::comment_rules::{Finding, Rule};
use pde_comment_review::shared::git::resolve_commit;
use pde_comment_review::steps::NextStep;
use pde_comment_review::steps::check_comments::check_comments;

/// Holds Go source that does not parse.
const BROKEN_GO: &str = "package review\n\nfunc SaveAnswer( {}\n";
/// Holds a Review Session waiting for User Feedback.
const WAITING_REVIEW_SESSION: &str = r#"{"status": "waiting_for_user_feedback", "origin": {"repo": "personal-dev-env", "branch": "review-comments", "agent_session": "ses_123"}, "lists": [], "feedback": {}}"#;

/// Commits `before` in a file, adds each of `decisions` on `name`, writes
/// `after`, and returns the failures `check_comments` finds.
fn check_removal(
    file: &str,
    name: &str,
    decisions: &[&str],
    before: &str,
    after: &str,
) -> Vec<Finding> {
    let repo = make_repo();
    let dir = repo.path();
    fs::write(dir.join(file), before).unwrap();
    commit_all(dir);
    let base = resolve_commit(dir, "HEAD").unwrap();
    for decision in decisions {
        add_decision(dir, file, name, decision);
    }
    fs::write(dir.join(file), after).unwrap();
    check_comments(dir, Some(base)).unwrap().result.failures
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

/// Makes a clone with an approved comment, a reword staged then undone on disk,
/// a new Markdown file, and `review_session` saved as its Review Session.
fn make_repo_with_review_session(review_session: &str) -> TempFolder {
    let repo = make_repo();
    let dir = repo.path();
    add_decision(dir, STORE, "SaveAnswer", APPROVED);
    // A reword staged and then undone on disk makes `check_comments` read the index too.
    write_store(dir, REWORDED, "");
    run_git(dir, &["add", STORE]);
    write_store(dir, APPROVED, "");
    fs::write(dir.join("README.md"), "Notes on the store.\n").unwrap();
    fs::write(dir.join(".git/pde/review.json"), review_session).unwrap();
    repo
}

/// `check_comments` must fail a reworded approved comment from any folder, whether the reword is committed, staged, on disk, hidden from Git or behind a restored copy.
/// Otherwise an agent could change text the user approved.
#[test]
fn fails_reworded_approved_comment() {
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
        };
        assert_eq!(output.result.failures, [failure], "{case}");
        assert_eq!(output.next, NextStep::CheckComments, "{case}");
    }
}

/// `check_comments` must pass a comment the branch did not change, even one approved with other text, and a changed comment on a name the user cut.
/// Otherwise the agent would be stuck on comments that `make_feedback_request_list` asks the user about.
#[test]
fn passes_unchanged_or_cut_comments() {
    // Each branch changes the file, so `check_comments` reads it.
    let cases = [
        (
            "approved-elsewhere-unchanged",
            REWORDED,
            store_source(APPROVED, "return"),
        ),
        ("cut-then-reworded", "", store_source(REWORDED, "")),
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

/// `check_comments` must read an approved comment in a file with `\r\n` line ends as unchanged.
/// Otherwise every approved comment in a checkout with `\r\n` line ends would fail.
#[test]
fn passes_approved_comment_with_crlf_line_ends() {
    let repo = make_repo();
    let dir = repo.path();
    add_decision(dir, STORE, "SaveAnswer", APPROVED);
    let source = store_source(APPROVED, "").replace('\n', "\r\n");
    fs::write(dir.join(STORE), source).unwrap();

    let output = check_comments(dir, None).unwrap();

    assert_eq!(output.result.failures, []);
    assert_eq!(output.next, NextStep::MakeFeedbackRequestList);
}

/// `check_comments` must fail an approved comment, docstring or module comment that the branch removed while the code it describes stays.
/// Otherwise an agent could delete text the user approved.
#[test]
fn fails_removed_approved_comment_while_code_stays() {
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
            &[APPROVED],
            store_source(APPROVED, ""),
            store_source("", ""),
            3,
        ),
        (
            "python-docstring-removed",
            "review.py",
            "Review.save",
            &[docstring],
            python_before,
            python_after.to_owned(),
            2,
        ),
        (
            "rust-module-comment-removed",
            "lib.rs",
            "<module>",
            &[module_comment],
            rust_before,
            "pub mod shared;\n".to_owned(),
            1,
        ),
    ];

    for (case, file, name, decisions, before, after, line) in cases {
        let failures = check_removal(file, name, decisions, &before, &after);

        let failure = Finding {
            file: PathBuf::from(file),
            name: Some(name.to_owned()),
            line,
            rule: Rule::ApprovedRemoved,
        };
        assert_eq!(failures, [failure], "{case}");
    }
}

/// `check_comments` must pass a removed comment whose code went too, or that the user cut or never decided on.
/// Otherwise an agent would be stuck after deleting a function, or a comment the user did not approve.
#[test]
fn passes_removed_comment_without_code_or_approval() {
    let cases: [(&str, &[&str], String); 3] = [
        (
            "code-removed-too",
            &[APPROVED],
            "package review\n".to_owned(),
        ),
        ("cut-comment-removed", &[""], store_source("", "")),
        ("undecided-comment-removed", &[], store_source("", "")),
    ];

    for (case, decisions, after) in cases {
        let failures = check_removal(
            STORE,
            "SaveAnswer",
            decisions,
            &store_source(APPROVED, ""),
            &after,
        );

        assert_eq!(failures, [], "{case}");
    }
}

/// `check_comments` must check only the changes since the `base` it is given.
/// Otherwise a branch made from another would fail on its parent's changes, which its agent must not undo.
#[test]
fn checks_only_changes_since_given_base() {
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

/// `check_comments` must refuse a `base` that HEAD does not build on.
/// Otherwise a base from a sibling branch that already holds the branch's reword would hide it.
#[test]
fn refuses_base_outside_head_history() {
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

/// `check_comments` must list a changed file that no language rule reads, instead of refusing the check.
/// Otherwise a Markdown change would block the branch, or the agent would not know its comments went unchecked.
#[test]
fn lists_unsupported_changed_file_as_unchecked() {
    let repo = make_repo();
    let dir = repo.path();
    fs::write(dir.join("README.md"), "Notes on the store.\n").unwrap();

    let output = check_comments(dir, None).unwrap();

    assert_eq!(output.result.unsupported, [PathBuf::from("README.md")]);
}

/// `check_comments` must refuse a changed file that does not parse, naming the file.
/// Otherwise a broken file could hide a changed approved comment.
#[test]
fn refuses_unparsable_changed_file() {
    let repo = make_repo();
    let dir = repo.path();
    fs::write(dir.join("broken.go"), BROKEN_GO).unwrap();

    let error = check_comments(dir, None).unwrap_err();

    assert_eq!(error.code, ErrorCode::FileUnreadable);
    assert!(error.message.contains("broken.go"), "{}", error.message);
}

/// `check_comments` must pass an approved file the branch never changed, even one that does not parse, after its time stamp or line ends change.
/// Otherwise an old broken file would block a branch that never touched it.
#[test]
fn passes_unchanged_unparsable_approved_file() {
    /// Sets the broken file's modified time to 1970 without changing its bytes.
    fn change_time_stamp(dir: &Path) {
        let file = File::options().write(true).open(dir.join("old_broken.go"));
        file.unwrap().set_modified(UNIX_EPOCH).unwrap();
    }
    /// Rewrites the broken file with `\r\n` line ends, which Git stores as `\n` under autocrlf.
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

/// `check_comments` must leave every file in the clone as it was, beside a waiting or damaged Review Session, even when it refuses a file that does not parse.
/// Otherwise a check could change the user's decisions, the code it reads or the Review Session, even by moving a damaged one aside.
#[test]
fn writes_no_file_even_when_refusing() {
    let damaged = "{\"status\": ";
    let cases = [
        (
            "passes-beside-waiting-review-session",
            WAITING_REVIEW_SESSION,
            "package review\n",
            Ok(()),
        ),
        (
            "passes-beside-damaged-review-session",
            damaged,
            "package review\n",
            Ok(()),
        ),
        (
            "refuses-beside-damaged-review-session",
            damaged,
            BROKEN_GO,
            Err(ErrorCode::FileUnreadable),
        ),
    ];

    for (case, review_session, new_go_file, expected) in cases {
        let repo = make_repo_with_review_session(review_session);
        let dir = repo.path();
        fs::write(dir.join("new.go"), new_go_file).unwrap();
        let before = read_every_file(dir);

        let result = check_comments(dir, None);

        let result = result.map(|_| ()).map_err(|error| error.code);
        assert_eq!(result, expected, "{case}");
        assert_eq!(read_every_file(dir), before, "{case}");
    }
}
