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

/// Names the Go file most tests change.
const STORE: &str = "review_store.go";
/// Holds the comment text the user approved.
const APPROVED: &str = "// Saves the answer under the lock.\n";
/// Holds an agent's new wording of the approved comment.
const REWORDED: &str = "// Writes the answer while holding the lock.\n";
/// Holds Go source that does not parse.
const BROKEN_GO: &str = "package review\n\nfunc SaveAnswer( {}\n";
/// Holds a Review Session waiting for User Feedback.
const WAITING_REVIEW_SESSION: &str = r#"{"status": "waiting_for_user_feedback", "origin": {"repo": "personal-dev-env", "branch": "review-comments", "agent_session": "ses_123"}, "lists": [], "feedback": {}}"#;

/// Runs Git without the test machine's own settings, so they cannot change the result.
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

/// Returns Go source with `comment` above a `SaveAnswer` function whose body is `body`.
fn store_source(comment: &str, body: &str) -> String {
    format!("package review\n\n{comment}func SaveAnswer() {{{body}}}\n")
}

/// Writes `store_source(comment, body)` to the store file on disk.
fn write_store(dir: &Path, comment: &str, body: &str) {
    fs::write(dir.join(STORE), store_source(comment, body)).unwrap();
}

/// Commits every change, or makes an empty commit when nothing changed.
fn commit_all(dir: &Path) {
    run_git(dir, &["add", "--all"]);
    run_git(dir, &["commit", "--allow-empty", "-m", "Change the store"]);
}

/// Saves the user's approval of `text` for `name`, or a cut when `text` is empty.
fn add_decision(dir: &Path, file: &str, name: &str, text: &str) {
    let decision = Approval {
        file: PathBuf::from(file),
        name: name.to_owned(),
        text: text.to_owned(),
    };
    approvals::add(dir, &[decision]).unwrap();
}

/// Commits `before` in a file, adds the decision on `name` if one is given,
/// writes `after`, and returns step 2's failures.
fn check_removal(
    file: &str,
    name: &str,
    decision: Option<&str>,
    before: &str,
    after: &str,
) -> Vec<Finding> {
    let repo = make_repo();
    let dir = repo.path();
    fs::write(dir.join(file), before).unwrap();
    commit_all(dir);
    let base = resolve_commit(dir, "HEAD").unwrap();
    if let Some(decision) = decision {
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
    // A reword staged and then undone on disk makes step 2 read the index too.
    write_store(dir, REWORDED, "");
    run_git(dir, &["add", STORE]);
    write_store(dir, APPROVED, "");
    fs::write(dir.join("README.md"), "Notes on the store.\n").unwrap();
    fs::write(dir.join(".git/pde/review.json"), review_session).unwrap();
    repo
}

/// Saves a Review Session in `status` that sent each of `lists` from HEAD, with `feedback` by request ID.
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

/// Returns the saved Review Session's modified time and bytes, or `None` when it is missing.
fn read_review_file(dir: &Path) -> Option<(SystemTime, Vec<u8>)> {
    let path = dir.join(".git/pde/review.json");
    let bytes = fs::read(&path).ok()?;
    Some((fs::metadata(&path).unwrap().modified().unwrap(), bytes))
}

/// Returns a sent Feedback Request for the store file, with reason `new` and a keep Agent Suggestion.
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

/// Describes a request by its ID and line, then as `describe` does.
fn describe_with_id(request: &FeedbackRequest) -> String {
    format!(
        "{} {} {}",
        json!(request.id),
        request.line,
        describe(request)
    )
}

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

/// `check_comments` must fail a reworded approved comment from any folder, whether the reword is committed, staged, on disk, hidden from Git or behind a restored copy.
/// Otherwise an agent could change text the user approved.
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

/// `check_comments` must pass a comment the branch did not change, even one approved with other text, and a changed comment on a name the user cut.
/// Otherwise the agent would be stuck on comments that step 3 asks the user about.
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
fn crlf_checkout_passes_unchanged_approved_comment() {
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
            Some(APPROVED),
            store_source(APPROVED, ""),
            store_source("", ""),
            3,
        ),
        (
            "python-docstring-removed",
            "review.py",
            "Review.save",
            Some(docstring),
            python_before,
            python_after.to_owned(),
            2,
        ),
        (
            "rust-module-comment-removed",
            "lib.rs",
            "<module>",
            Some(module_comment),
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

/// `check_comments` must pass a removed comment whose code went too, or that the user cut or never decided on.
/// Otherwise an agent would be stuck after deleting a function, or a comment the user did not approve.
#[test]
fn deleted_code_or_unapproved_comment_passes_check() {
    let cases = [
        (
            "code-removed-too",
            Some(APPROVED),
            "package review\n".to_owned(),
        ),
        ("cut-comment-removed", Some(""), store_source("", "")),
        ("undecided-comment-removed", None, store_source("", "")),
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

/// `check_comments` must check only the changes since the `base` it is given.
/// Otherwise a branch made from another would fail on its parent's changes, which its agent must not undo.
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

/// `check_comments` must refuse a `base` that HEAD does not build on.
/// Otherwise a base from a sibling branch that already holds the branch's reword would hide it.
#[test]
fn non_ancestor_base_refuses_check() {
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
fn unsupported_changed_file_is_listed_unchecked() {
    let repo = make_repo();
    let dir = repo.path();
    fs::write(dir.join("README.md"), "Notes on the store.\n").unwrap();

    let output = check_comments(dir, None).unwrap();

    assert_eq!(output.result.unsupported, [PathBuf::from("README.md")]);
}

/// `check_comments` must refuse a changed file that does not parse, naming the file.
/// Otherwise a broken file could hide a changed approved comment.
#[test]
fn unparsable_changed_file_refuses_check() {
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
fn unchanged_broken_approved_file_passes_check() {
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

/// `check_comments` must leave every file in the clone as it was, beside a waiting or damaged Review Session.
/// Otherwise a check could change the user's decisions, the Review Session or the code it reads.
#[test]
fn check_comments_leaves_repository_files_unchanged() {
    let cases = [
        ("waiting-review-session", WAITING_REVIEW_SESSION),
        ("damaged-review-session", "{\"status\": "),
    ];

    for (case, review_session) in cases {
        let repo = make_repo_with_review_session(review_session);
        let dir = repo.path();
        let before = read_every_file(dir);

        let result = check_comments(dir, None);

        assert!(result.is_ok(), "{case}");
        assert_eq!(read_every_file(dir), before, "{case}");
    }
}

/// `check_comments` must leave every file in the clone as it was when it refuses a file that does not parse.
/// Otherwise a refused check could move a damaged Review Session aside.
#[test]
fn refused_check_leaves_repository_files_unchanged() {
    let repo = make_repo_with_review_session("{\"status\": ");
    let dir = repo.path();
    fs::write(dir.join("broken.go"), BROKEN_GO).unwrap();
    let before = read_every_file(dir);

    let result = check_comments(dir, None);

    assert!(result.is_err());
    assert_eq!(read_every_file(dir), before);
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

/// `make_feedback_request_list` must list a cut comment that is back, a comment that differs from its approval, and a new comment, and nothing else.
/// Otherwise the user would miss a decision made in another worktree, or answer comments nobody changed.
#[test]
fn list_holds_only_new_or_conflicting_comments() {
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

/// `make_feedback_request_list` must refuse while step 2 still fails.
/// Otherwise an agent could put its reword of approved text on the list, and keeping it would approve the reword.
#[test]
fn failed_check_refuses_list() {
    let repo = make_repo();
    let dir = repo.path();
    add_decision(dir, STORE, "SaveAnswer", APPROVED);
    write_store(dir, REWORDED, "");
    let change_request = ChangeRequest {
        file: PathBuf::from(STORE),
        name: "SaveAnswer".to_owned(),
    };

    // The change request would refuse with another code if step 2 did not refuse first.
    let error = make_feedback_request_list(dir, None, &[change_request]).unwrap_err();

    assert_eq!(error.code, ErrorCode::CheckFailed);
}

/// `make_feedback_request_list` must list a change request once, with the approved text from disk.
/// Otherwise the user could see text other than what they approved, or two pieces of User Feedback would be written into one comment.
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

/// `make_feedback_request_list` must number a new list's requests from 1 in file and line order.
/// Otherwise two requests could share an ID and one's User Feedback would be lost.
#[test]
fn new_list_numbers_requests_from_one() {
    let repo = make_repo();
    let dir = repo.path();
    add_decision(dir, STORE, "SaveAnswer", APPROVED);
    let source = format!(
        "{}\n// Counts the answers.\nfunc CountAnswers() {{}}\n",
        store_source(APPROVED, "")
    );
    fs::write(dir.join(STORE), source).unwrap();
    let change_request = ChangeRequest {
        file: PathBuf::from(STORE),
        name: "SaveAnswer".to_owned(),
    };

    let output = make_feedback_request_list(dir, None, &[change_request]).unwrap();

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
            "1 3 review_store.go SaveAnswer agent_wants_change",
            "2 6 review_store.go CountAnswers new",
        ]
    );
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

/// `make_feedback_request_list` must refuse while a Feedback Request List waits for the user or a write did not finish.
/// Otherwise a new list could replace one the user is still answering, or run over an unfinished write.
#[test]
fn waiting_or_writing_review_session_refuses_list() {
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

/// `make_feedback_request_list` must leave the saved Review Session's bytes and modified time as they were, and a missing one missing.
/// Otherwise building or refusing a list could rewrite or start the Review Session and lose User Feedback.
#[test]
fn make_feedback_request_list_leaves_review_session() {
    let cases = [
        None,
        Some("waiting_for_user_feedback"),
        Some("writing_user_feedback"),
        Some("user_notes_waiting"),
        Some("done"),
    ];

    for status in cases {
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

        let _ = make_feedback_request_list(dir, None, &[]);

        assert_eq!(read_review_file(dir), before, "{status:?}");
    }
}

/// `make_feedback_request_list` must resend each noted request whose comment is still there once, under its ID with the comment's current text.
/// Otherwise the user would answer a User Note's request with old text, under a new ID, or twice.
#[test]
fn noted_requests_keep_ids_with_current_text() {
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

/// `make_feedback_request_list` must list each comment written while User Notes wait under a new ID after the highest one.
/// Otherwise a comment the agent wrote while answering User Notes would skip the user, or take an ID already used.
#[test]
fn comments_written_during_user_notes_get_ids() {
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
fn only_reworded_answered_comments_are_asked_again() {
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

/// `make_feedback_request_list` must keep a resent list in file and line order, with noted and new requests mixed.
/// Otherwise the user would get the comments out of file order.
#[test]
fn resent_list_keeps_file_and_line_order() {
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

/// `make_feedback_request_list` must use the Review Session's saved base when User Notes wait and no base is given.
/// Otherwise a stacked branch would be checked from the branch start and list its parent's comments.
#[test]
fn no_base_given_uses_saved_review_base() {
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

/// `make_feedback_request_list` must refuse a base other than the saved one while User Notes wait.
/// Otherwise a later base could hide a comment committed after the list was sent.
#[test]
fn other_base_during_user_notes_refuses_list() {
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
fn unasked_committed_or_staged_comment_refuses_list() {
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

/// `make_feedback_request_list` must list a named comment the disk copy asks about even when HEAD holds an older wording.
/// Otherwise an agent would have to commit before each list whenever it rewords a committed comment.
#[test]
fn older_committed_wording_still_lists_named_comment() {
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

/// `make_feedback_request_list` must refuse a file with a decision that does not parse, naming the file, even when the branch never changed it.
/// Otherwise a cut decided in another worktree could hide in it, and the list would come back empty.
#[test]
fn unparsable_approved_file_refuses_list() {
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

/// `show_status` must name the next step and the requests or User Notes the Review Session waits on.
/// Otherwise an agent could call the wrong step next, or miss what the user waits for.
#[test]
fn show_status_tells_agent_what_comes_next() {
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
