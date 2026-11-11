mod common;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use common::TempFolder;
use glimmer::shared::ErrorCode;
use glimmer::shared::git::{
    find_branch_start, find_shared_git_folder, find_worktree_git_folder, list_changed_files,
    list_changed_lines, read_current_branch, read_file_at_commit, resolve_commit,
};

/// Runs Git without the test machine's own settings and returns its trimmed output.
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

/// Makes a new Git repository on `main` with one empty commit.
fn make_repo() -> TempFolder {
    let folder = TempFolder::new();
    run_git(folder.path(), &["init", "-b", "main"]);
    run_git(
        folder.path(),
        &["commit", "--allow-empty", "-m", "Start review"],
    );
    folder
}

/// Commits every change in the checkout.
fn commit_files(dir: &Path) {
    run_git(dir, &["add", "--all"]);
    run_git(dir, &["commit", "-m", "Change review comments"]);
}

/// `find_branch_start` must return the commit where the branch split from main, even after main moves on or exists only as `origin/main`.
/// Otherwise new main commits would ask the user about comments the agent never touched.
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

/// `list_changed_files` must list committed, staged, unstaged, deleted, renamed and oddly named files, but not ignored ones.
/// Otherwise such a file could hide an edit to an approved comment.
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

/// `list_changed_files` must list changed files across the whole checkout when run from a subfolder.
/// Otherwise a review started in a subfolder would miss new files elsewhere.
#[test]
fn changed_files_work_from_a_subfolder() {
    let repo = make_repo();
    let dir = repo.path();
    let base = resolve_commit(dir, "HEAD").unwrap();
    fs::write(dir.join("review.rs"), "// Changed comment.\n").unwrap();
    fs::create_dir(dir.join("nested")).unwrap();

    let files = list_changed_files(&dir.join("nested"), &base).unwrap();

    assert_eq!(files, vec![PathBuf::from("review.rs")]);
}

/// `list_changed_files` and `list_changed_lines` must leave the bytes of Git's index as they were.
/// Otherwise Git could rewrite the index during a review and make a commit at the same time fail.
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

/// `list_changed_lines` must return the added and replaced lines by their numbers in the current file.
/// Otherwise style checks would flag old sentences or skip new ones.
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
            &lines,
            &case.expected.iter().copied().collect(),
            "{}",
            case.name
        );
    }
}

/// `list_changed_lines` must include both staged edits and edits made after staging.
/// Otherwise half the changes would skip the style checks.
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

    assert_eq!(&lines, &BTreeSet::from([1, 3]));
}

/// `read_file_at_commit` must return a file's text at the base byte for byte, and `None` for a file missing there.
/// Otherwise every comment in the file could look changed.
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

/// `resolve_commit`, `read_file_at_commit` and `find_worktree_git_folder` must refuse a missing base, a path outside the checkout and a folder outside Git.
/// Otherwise a mistyped base would make every comment look new instead of reporting an error.
#[test]
fn bad_bases_paths_and_checkouts_return_codes() {
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

/// `find_worktree_git_folder` must give a linked worktree its own Git folder, and `find_shared_git_folder` one folder for the whole clone.
/// Otherwise worktrees would overwrite each other's reviews or miss approvals made in another worktree.
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
