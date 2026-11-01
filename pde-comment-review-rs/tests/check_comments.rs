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
use pde_comment_review::shared::comment_rules::{Finding, ReportNote, Rule};
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

/// Returns the findings `check_comments` gives a new file holding `source`, failures first.
fn check_new_file(file: &str, source: &str) -> Vec<(Rule, Option<ReportNote>, String)> {
    let repo = make_repo();
    let dir = repo.path();
    add_avoided_word(dir);
    fs::write(dir.join(file), source).unwrap();
    let output = check_comments(dir, None).unwrap();
    list_rules(output.result.failures, output.result.reported)
}

/// Writes an approvals file with no decisions and one word on the avoid list.
fn add_avoided_word(dir: &Path) {
    fs::create_dir_all(dir.join(".git/pde")).unwrap();
    let approvals = r#"{"approved": [], "avoid": {"blob": "say file"}}"#;
    fs::write(dir.join(".git/pde/code-comments.json"), approvals).unwrap();
}

/// Returns each finding's rule, note and detail, failures first.
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
            note: None,
            detail: None,
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
            note: None,
            detail: None,
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

/// `check_comments` must find a break of each style rule and name what broke it.
/// Otherwise filler, long comments or unclear names an agent writes would reach the user unflagged.
#[test]
fn finds_each_style_rule_break() {
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
            "comment-colon-inside-function",
            "answer.go",
            go("func SaveAnswer() {\n\t// Saves it: now.\n}\n"),
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
            "docstring-on-init",
            "answer.py",
            "class Store:\n    \"\"\"Holds answers.\"\"\"\n\n    def __init__(self):\n        pass\n"
                .to_owned(),
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

/// `check_comments` must only report a style break in a comment that matches the user's approval.
/// Otherwise an agent would be stuck on text it may not change.
#[test]
fn only_reports_approved_text_style_breaks() {
    let filler = "// This facilitates saving.\n";
    let repo = make_repo();
    let dir = repo.path();
    add_decision(dir, STORE, "SaveAnswer", filler);
    write_store(dir, filler, "");

    let output = check_comments(dir, None).unwrap();

    assert_eq!(output.result.failures, []);
    let reported = list_rules(Vec::new(), output.result.reported);
    let expected = (
        Rule::Filler,
        Some(ReportNote::Approved),
        "\"facilitates\"".to_owned(),
    );
    assert_eq!(reported, [expected]);
}

/// `check_comments` must only report a style break in a sentence already there at `base` or in a license header.
/// Otherwise an agent would be stuck on an earlier author's text or a license it may not change.
#[test]
fn only_reports_old_or_license_style_breaks() {
    let cases = [
        (
            "sentence-at-base",
            "// This facilitates saving.\n",
            store_source("// This facilitates saving. It holds the lock.\n", ""),
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
            (
                Rule::CommentLength,
                Some(ReportNote::Header),
                "3 sentences".to_owned(),
            ),
        ),
    ];

    for (case, base_comment, source, expected) in cases {
        let repo = make_repo();
        let dir = repo.path();
        write_store(dir, base_comment, "");
        commit_all(dir);
        let base = resolve_commit(dir, "HEAD").unwrap();
        fs::write(dir.join(STORE), source).unwrap();

        let output = check_comments(dir, Some(base)).unwrap();

        assert_eq!(output.result.failures, [], "{case}");
        let reported = list_rules(Vec::new(), output.result.reported);
        assert_eq!(reported, [expected], "{case}");
    }
}

/// `check_comments` must pass a branch, and name `make_feedback_request_list` next, when every finding is only reported.
/// Otherwise an agent with only reported findings could never ask the user about its comments.
#[test]
fn passes_with_only_reported_findings() {
    let repo = make_repo();
    let dir = repo.path();
    write_store(dir, "// Writes the answer, as SaveAnswer says.\n", "");

    let output = check_comments(dir, None).unwrap();

    assert_eq!(output.result.failures, []);
    let reported = list_rules(Vec::new(), output.result.reported);
    let expected = (
        Rule::RestatedName,
        Some(ReportNote::ReportOnly),
        String::new(),
    );
    assert_eq!(reported, [expected]);
    assert_eq!(output.next, NextStep::MakeFeedbackRequestList);
}

/// `check_comments` must fail an old comment that new sentences make too long.
/// Otherwise an agent could add sentences to an old comment until it is too long, and `check_comments` would pass it.
#[test]
fn fails_old_comment_made_too_long() {
    let repo = make_repo();
    let dir = repo.path();
    write_store(dir, "// Saves it.\n", "");
    commit_all(dir);
    let base = resolve_commit(dir, "HEAD").unwrap();
    write_store(dir, "// Saves it. Locks it. Logs it.\n", "");

    let output = check_comments(dir, Some(base)).unwrap();

    let failures = list_rules(output.result.failures, Vec::new());
    assert_eq!(
        failures,
        [(Rule::CommentLength, None, "3 sentences".to_owned())]
    );
}

/// `check_comments` must pass old Go comments and names in a changed or moved file when only the code around them changed.
/// Otherwise touching or moving an old file would fail its untouched comments and names.
#[test]
fn passes_old_or_moved_comments_and_names() {
    let old_comment = "// This facilitates saving.\n";
    let long_name = "package review\n\n// This facilitates saving.\nfunc SaveTheAnswerToTheStoreRightNow() {}\n";
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
    ];

    for (case, old_file, before, file, after) in cases {
        let repo = make_repo();
        let dir = repo.path();
        fs::write(dir.join(old_file), before).unwrap();
        commit_all(dir);
        fs::remove_file(dir.join(old_file)).unwrap();
        let base = resolve_commit(dir, "HEAD").unwrap();
        fs::write(dir.join(file), after).unwrap();

        let output = check_comments(dir, Some(base)).unwrap();

        assert_eq!(output.result.failures, [], "{case}");
    }
}

/// `check_comments` must not ask for a docstring on a function moved from a deleted file, or on an old class when only one of its methods changed.
/// Otherwise moving a file or editing one method would ask for docstrings on code the branch did not write.
#[test]
fn skips_docstring_on_moved_or_untouched_definitions() {
    let python_without_docstring = "def save_answer():\n    return 1\n";
    let python_class =
        "class Store:\n    def save(self):\n        \"\"\"Saves it.\"\"\"\n        return 1\n";
    let cases = [
        (
            "old-class-method-body-changed",
            "answer.py",
            python_class.to_owned(),
            "answer.py",
            python_class.replace("return 1", "return 2"),
        ),
        (
            "moved-python-without-docstring",
            "old_answer.py",
            python_without_docstring.to_owned(),
            "answer.py",
            python_without_docstring.to_owned(),
        ),
    ];

    for (case, old_file, before, file, after) in cases {
        let repo = make_repo();
        let dir = repo.path();
        fs::write(dir.join(old_file), before).unwrap();
        commit_all(dir);
        fs::remove_file(dir.join(old_file)).unwrap();
        let base = resolve_commit(dir, "HEAD").unwrap();
        fs::write(dir.join(file), after).unwrap();

        let output = check_comments(dir, Some(base)).unwrap();

        assert_eq!(output.result.failures, [], "{case}");
    }
}

/// `check_comments` must fail a missing docstring on an old function the branch edits, even when a deleted file held the same name.
/// Otherwise an agent could change an old function and never give it the docstring the skill asks for.
#[test]
fn fails_edited_old_function_without_docstring() {
    let repo = make_repo();
    let dir = repo.path();
    let old_function = "def save():\n    return 1\n";
    fs::write(dir.join("answer.py"), old_function).unwrap();
    fs::write(dir.join("old_answer.py"), old_function).unwrap();
    commit_all(dir);
    let base = resolve_commit(dir, "HEAD").unwrap();
    fs::remove_file(dir.join("old_answer.py")).unwrap();
    fs::write(dir.join("answer.py"), "def save():\n    return 2\n").unwrap();

    let output = check_comments(dir, Some(base)).unwrap();

    let failures = list_rules(output.result.failures, Vec::new());
    assert_eq!(failures, [(Rule::Docstring, None, String::new())]);
}

/// `check_comments` must fail a missing docstring on a new function that shares its name with a function in a file that stays.
/// Otherwise a new function with a common name, such as `save`, would never need a docstring.
#[test]
fn fails_new_same_named_function_without_docstring() {
    let repo = make_repo();
    let dir = repo.path();
    fs::write(
        dir.join("answer.py"),
        "def save():\n    \"\"\"Saves it.\"\"\"\n    return 1\n",
    )
    .unwrap();
    commit_all(dir);
    let base = resolve_commit(dir, "HEAD").unwrap();
    fs::write(
        dir.join("answer.py"),
        "def save():\n    \"\"\"Saves it.\"\"\"\n    return 2\n",
    )
    .unwrap();
    fs::write(dir.join("store.py"), "def save():\n    return 1\n").unwrap();

    let output = check_comments(dir, Some(base)).unwrap();

    let failures = list_rules(output.result.failures, Vec::new());
    assert_eq!(failures, [(Rule::Docstring, None, String::new())]);
}

/// `check_comments` must pass a file a generator wrote, whatever its comments say.
/// Otherwise an agent would be told to fix comments that the next run of the generator writes back.
#[test]
fn passes_generated_file() {
    let generated = "// Code generated by protoc. DO NOT EDIT.\n\npackage review\n\n// This facilitates saving.\nfunc SaveAnswer() {}\n";

    assert_eq!(check_new_file("answer.go", generated), []);
}

/// `check_comments` must read a Python comment without the `# noqa` mark after it.
/// Otherwise an agent would be told to fix the colon in a mark that a linter reads.
#[test]
fn ignores_linter_mark_after_python_comment() {
    let source = "LIMIT = 1  # Counts it.  # noqa: E501\n";

    assert_eq!(check_new_file("answer.py", source), []);
}

/// `check_comments` must not run the length and colon rules on Rust comments, which this version checks only in Go and Python.
/// Otherwise a Rust comment of three sentences or with a colon would fail `check_comments`.
#[test]
fn skips_length_and_colon_rules_in_rust() {
    let source = "// Saves it: now. Locks it. Logs it.\npub fn save_answer() {}\n";

    assert_eq!(check_new_file("lib.rs", source), []);
}

/// `check_comments` must not ask for a verb at the start of a test, getter, constructor or framework method name.
/// Otherwise an agent would be told to put a verb on a name that must not have one.
#[test]
fn skips_verb_rule_on_conventional_names() {
    let go = |code: &str| format!("package review\n\n{code}");
    let python = r#"import pytest
from typing import override


class Store:
    """Holds answers."""

    def __init__(self):
        """Starts empty."""

    def setUp(self):
        """Prepares it."""

    def tearDown(self):
        """Cleans up."""

    @property
    def total(self):
        """Counts answers."""

    @override
    def ledger(self):
        """Lists answers."""


@pytest.fixture
def database():
    """Opens an empty one."""
"#;
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
