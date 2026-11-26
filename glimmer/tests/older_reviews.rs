mod common;

use std::fs;
use std::path::Path;

use common::{TempFolder, read_archive_copies, use_test_waiting_list};
use glimmer::shared::older_reviews::archive_older_reviews;
use serde_json::{Value, json};

/// Writes `review` as the review.json in `git_folder`.
fn write_review(git_folder: &Path, review: &Value) {
    let folder = git_folder.join("pde");
    fs::create_dir_all(&folder).unwrap();
    fs::write(folder.join("review.json"), review.to_string()).unwrap();
}

/// Reviews in a main checkout and in a linked worktree must each get one archive copy, and a second run must not copy them again.
/// Otherwise a review saved before glimmer kept copies is lost with its worktree, or every glimmer command walks the home folder.
#[test]
fn archive_older_reviews_copies_each_review_once() {
    let waiting_list = use_test_waiting_list();
    // A test process with a reused process ID finds the marker of an earlier run.
    let _ = fs::remove_file(waiting_list.join("older-reviews-archived.txt"));
    let home = TempFolder::new();
    let repo = home.path().file_name().unwrap().to_str();
    let checkout = json!({"status": "done", "origin": {"repo": repo, "branch": "main"}});
    let linked = json!({"status": "dismissed", "origin": {"repo": repo, "branch": "feat/login"}});
    let git_folder = home.path().join("shop/.git");
    write_review(&git_folder, &checkout);
    write_review(&git_folder.join("worktrees/login"), &linked);

    archive_older_reviews(home.path()).unwrap();
    archive_older_reviews(home.path()).unwrap();

    let copies = read_archive_copies(home.path());
    assert_eq!(copies.len(), 2, "{copies:?}");
    assert!(
        copies.contains(&checkout) && copies.contains(&linked),
        "{copies:?}"
    );
}
