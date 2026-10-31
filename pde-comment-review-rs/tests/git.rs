mod common;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use common::TempFolder;
use pde_comment_review::shared::ErrorCode;
use pde_comment_review::shared::git::{
    find_branch_start, find_shared_git_folder, find_worktree_git_folder, list_changed_files,
    list_changed_lines, read_current_branch, read_file_at_commit, resolve_commit,
};

fn run_git(dir: &Path, args: &[&str]) -> String {
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
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn make_repo() -> TempFolder {
    let folder = TempFolder::new();
    run_git(folder.path(), &["init", "-b", "main"]);
    run_git(
        folder.path(),
        &["commit", "--allow-empty", "-m", "Start review"],
    );
    folder
}

fn commit_files(dir: &Path) {
    run_git(dir, &["add", "--all"]);
    run_git(dir, &["commit", "-m", "Change review comments"]);
}

/// Prevents later main commits from shifting the review's task start.
#[test]
fn branch_start_uses_the_shared_ancestor() {
    let repo = make_repo();
    let dir = repo.path();
    let split = resolve_commit(dir, "HEAD").unwrap();
    run_git(dir, &["checkout", "-b", "review-comments"]);
    run_git(dir, &["commit", "--allow-empty", "-m", "Review branch"]);
    run_git(dir, &["checkout", "main"]);
    run_git(dir, &["commit", "--allow-empty", "-m", "Main advances"]);
    run_git(dir, &["checkout", "review-comments"]);

    assert_eq!(find_branch_start(dir).unwrap(), split);

    run_git(dir, &["update-ref", "refs/remotes/origin/main", "main"]);
    run_git(dir, &["branch", "-D", "main"]);

    assert_eq!(find_branch_start(dir).unwrap(), split);
}

/// Keeps staged, new and removed comments from disappearing before review.
#[test]
fn changed_files_include_every_working_change() {
    let repo = make_repo();
    let dir = repo.path();
    for file in [
        "committed.rs",
        "staged.rs",
        "working.rs",
        "removed.rs",
        "old.rs",
    ] {
        fs::write(dir.join(file), "// Original comment.\n").unwrap();
    }
    fs::write(dir.join(".gitignore"), "ignored.rs\n").unwrap();
    commit_files(dir);
    let base = resolve_commit(dir, "HEAD").unwrap();
    fs::write(dir.join("committed.rs"), "// Committed wording.\n").unwrap();
    commit_files(dir);
    fs::write(dir.join("staged.rs"), "// Staged wording.\n").unwrap();
    run_git(dir, &["add", "staged.rs"]);
    fs::write(dir.join("working.rs"), "// Working wording.\n").unwrap();
    fs::remove_file(dir.join("removed.rs")).unwrap();
    run_git(dir, &["mv", "old.rs", "new.rs"]);
    let unusual = ":review [draft]\t\n.rs";
    fs::write(dir.join(unusual), "// New comment.\n").unwrap();
    fs::write(dir.join("ignored.rs"), "// Generated comment.\n").unwrap();

    let files = list_changed_files(dir, &base).unwrap();

    let expected: BTreeSet<_> = [
        "committed.rs",
        "staged.rs",
        "working.rs",
        "removed.rs",
        "old.rs",
        "new.rs",
        unusual,
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect();
    assert_eq!(files.len(), expected.len());
    assert_eq!(files.into_iter().collect::<BTreeSet<_>>(), expected);
}

/// Keeps review calls from depending on the caller's current subfolder.
#[test]
fn changed_files_work_from_a_subfolder() {
    let repo = make_repo();
    let dir = repo.path();
    fs::write(dir.join("review.rs"), "// Original comment.\n").unwrap();
    commit_files(dir);
    let base = resolve_commit(dir, "HEAD").unwrap();
    fs::write(dir.join("review.rs"), "// Changed comment.\n").unwrap();
    fs::create_dir(dir.join("nested")).unwrap();

    let files = list_changed_files(&dir.join("nested"), &base).unwrap();

    assert_eq!(files, vec![PathBuf::from("review.rs")]);
}

/// Keeps read-only review calls from competing with commits for the index lock.
#[test]
fn git_reads_preserve_index_bytes() {
    let repo = make_repo();
    let dir = repo.path();
    let path = dir.join("review.rs");
    fs::write(&path, "// Unchanged comment.\n").unwrap();
    commit_files(dir);
    run_git(dir, &["config", "diff.autoRefreshIndex", "true"]);
    let base = resolve_commit(dir, "HEAD").unwrap();
    let index = find_worktree_git_folder(dir).unwrap().join("index");
    let before = fs::read(&index).unwrap();
    fs::File::open(&path)
        .unwrap()
        .set_modified(SystemTime::UNIX_EPOCH)
        .unwrap();

    list_changed_files(dir, &base).unwrap();

    assert_eq!(fs::read(&index).unwrap(), before);

    list_changed_lines(dir, &base, Path::new("review.rs")).unwrap();

    assert_eq!(fs::read(&index).unwrap(), before);
}

/// Keeps style checks on present changed lines instead of unchanged neighbors.
#[test]
fn changed_lines_follow_current_line_numbers() {
    struct Case {
        name: &'static str,
        before: Option<&'static str>,
        after: &'static str,
        expected: &'static [usize],
    }
    let cases = [
        Case {
            name: "replacement",
            before: Some("// First.\n// Old.\n// Last.\n"),
            after: "// First.\n// New.\n// Last.\n",
            expected: &[2],
        },
        Case {
            name: "insertion",
            before: Some("// First.\n// Last.\n"),
            after: "// First.\n// New.\n// More.\n// Last.\n",
            expected: &[2, 3],
        },
        Case {
            name: "deletion",
            before: Some("// First.\n// Old.\n// Last.\n"),
            after: "// First.\n// Last.\n",
            expected: &[],
        },
        Case {
            name: "new without final newline",
            before: None,
            after: "// First.\n// Last.",
            expected: &[1, 2],
        },
        Case {
            name: "new empty file",
            before: None,
            after: "",
            expected: &[],
        },
    ];
    for case in cases {
        let repo = make_repo();
        let dir = repo.path();
        let file = Path::new(":review [draft].rs");
        if let Some(before) = case.before {
            fs::write(dir.join(file), before).unwrap();
            commit_files(dir);
        }
        let base = resolve_commit(dir, "HEAD").unwrap();
        fs::write(dir.join(file), case.after).unwrap();

        let lines = list_changed_lines(dir, &base, file).unwrap();

        assert_eq!(
            lines.lines(),
            &case.expected.iter().copied().collect(),
            "{}",
            case.name
        );
    }
}

/// Protects comments edited again after the agent staged an earlier wording.
#[test]
fn changed_lines_include_unstaged_and_staged_edits() {
    let repo = make_repo();
    let dir = repo.path();
    let file = Path::new("review.rs");
    fs::write(dir.join(file), "// First.\n// Middle.\n// Last.\n").unwrap();
    commit_files(dir);
    let base = resolve_commit(dir, "HEAD").unwrap();
    fs::write(dir.join(file), "// Staged.\n// Middle.\n// Last.\n").unwrap();
    run_git(dir, &["add", "review.rs"]);
    fs::write(dir.join(file), "// Staged.\n// Middle.\n// Working.\n").unwrap();

    let lines = list_changed_lines(dir, &base, file).unwrap();

    assert_eq!(lines.lines(), &BTreeSet::from([1, 3]));
}

/// Keeps local Git settings from marking unchanged lines as changed.
#[test]
fn changed_lines_ignore_local_diff_settings() {
    let repo = make_repo();
    let dir = repo.path();
    let file = Path::new("review.rs");
    fs::write(dir.join(file), "// First.\n// Middle.\n// Last.\n").unwrap();
    commit_files(dir);
    let base = resolve_commit(dir, "HEAD").unwrap();
    run_git(dir, &["config", "diff.interHunkContext", "10"]);
    fs::write(dir.join(file), "// New first.\n// Middle.\n// New last.\n").unwrap();

    let lines = list_changed_lines(dir, &base, file).unwrap();

    assert_eq!(lines.lines(), &BTreeSet::from([1, 3]));
}

/// Keeps original wording available even after the working file changes.
#[test]
fn historical_reads_preserve_text_and_absence() {
    let repo = make_repo();
    let dir = repo.path();
    let file = Path::new(":review [draft]\t\n.rs");
    let original = "// Keep these bytes.\r\n// No final newline.";
    fs::write(dir.join(file), original).unwrap();
    commit_files(dir);
    let base = resolve_commit(dir, "HEAD").unwrap();
    fs::write(dir.join(file), "// Different wording.\n").unwrap();

    assert_eq!(
        read_file_at_commit(dir, &base, file).unwrap().as_deref(),
        Some(original)
    );
    assert_eq!(
        read_file_at_commit(dir, &base, Path::new("missing.rs")).unwrap(),
        None
    );
}

/// Keeps bad paths and revisions from masquerading as absent comments.
#[test]
fn invalid_inputs_return_structured_failures() {
    let repo = make_repo();
    let dir = repo.path();
    let base = resolve_commit(dir, "HEAD").unwrap();

    assert_eq!(
        resolve_commit(dir, "missing-review-base").unwrap_err().code,
        ErrorCode::BadInput
    );
    assert_eq!(
        read_file_at_commit(dir, &base, Path::new("../review.rs"))
            .unwrap_err()
            .code,
        ErrorCode::BadInput
    );
    let outside = TempFolder::new();
    assert_eq!(
        find_worktree_git_folder(outside.path()).unwrap_err().code,
        ErrorCode::NotAGitCheckout
    );
}

/// Separates worktree reviews while keeping their clone's approvals together.
#[test]
fn worktree_identity_separates_branches_and_git_folders() {
    let repo = make_repo();
    let sibling = TempFolder::new();
    let linked = sibling.path().join("linked");
    run_git(
        repo.path(),
        &[
            "worktree",
            "add",
            "-b",
            "review-comments",
            linked.to_str().unwrap(),
        ],
    );

    let own = find_worktree_git_folder(&linked).unwrap();
    let shared = find_shared_git_folder(&linked).unwrap();

    assert!(own.is_absolute());
    assert!(shared.is_absolute());
    assert_ne!(own, shared);
    assert_eq!(shared, find_shared_git_folder(repo.path()).unwrap());
    assert_eq!(shared, find_worktree_git_folder(repo.path()).unwrap());
    assert_eq!(read_current_branch(&linked).unwrap(), "review-comments");
    run_git(&linked, &["checkout", "--detach"]);
    assert_eq!(read_current_branch(&linked).unwrap(), "HEAD");
}
