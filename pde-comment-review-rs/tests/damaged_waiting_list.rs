//! Kept apart from the other tests, since every test in one process shares
//! one waiting list and a damaged list would break the others.

mod common;

use std::fs;

use common::{TempFolder, run_git, use_test_waiting_list};
use pde_comment_review::shared::ErrorCode;
use pde_comment_review::shared::review_session::add_to_waiting_list;

/// `add_to_waiting_list` must refuse a damaged waiting list and leave its bytes unchanged.
/// Otherwise one bad hand edit would empty the list and hide every review that waits.
#[test]
fn add_to_waiting_list_refuses_damaged_list() {
    let list_folder = use_test_waiting_list();
    fs::create_dir_all(&list_folder).unwrap();
    let list_file = list_folder.join("waiting-list.json");
    let damaged = b"{\"worktrees\": [\"/home/user/project\"";
    fs::write(&list_file, damaged).unwrap();
    let folder = TempFolder::new();
    run_git(folder.path(), &["init", "--quiet"]);

    let error = add_to_waiting_list(folder.path()).unwrap_err();
    let saved_after = fs::read(&list_file).unwrap();
    // A later test process with this process ID would otherwise find the damaged list.
    fs::remove_file(&list_file).unwrap();

    assert_eq!(error.code, ErrorCode::SavedFileUnreadable);
    assert_eq!(saved_after, damaged);
}
