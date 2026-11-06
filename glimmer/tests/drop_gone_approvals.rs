mod common;

use std::fs;
use std::path::Path;

use common::{STORE, TempFolder, add_decision, make_repo, run_git};
use glimmer::commands::drop_gone_approvals::drop_gone_approvals;
use glimmer::shared::approvals::ApprovalsBackup;
use serde_json::Value;

/// Returns the file and name of each saved approval or cut, oldest first.
fn list_decisions(dir: &Path) -> Vec<(String, String)> {
    let path = dir.join(".git/pde/code-comments.json");
    let saved: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    saved["approved"]
        .as_array()
        .unwrap()
        .iter()
        .map(|decision| {
            let file = decision["file"].as_str().unwrap().to_owned();
            let name = decision["name"].as_str().unwrap().to_owned();
            (file, name)
        })
        .collect()
}

/// `drop_gone_approvals` must keep an approval or cut while any worktree has its file or function, and drop it once none does.
/// Otherwise the user loses approvals for a branch not merged yet, or the file keeps approvals and cuts for deleted code.
#[test]
fn drop_gone_approvals_keeps_code_worktrees_have() {
    // The main checkout's Go file has only SaveAnswer, and a second worktree adds draft.go.
    let repo = make_repo();
    let second = TempFolder::new();
    run_git(
        repo.path(),
        &[
            "worktree",
            "add",
            "-b",
            "second",
            second.path().to_str().unwrap(),
        ],
    );
    fs::write(
        second.path().join("draft.go"),
        "package review\n\nfunc Draft() {}\n",
    )
    .unwrap();
    for (file, name) in [
        (STORE, "SaveAnswer"),
        (STORE, "LoadAnswer"),
        ("deleted.go", "Old"),
        ("draft.go", "Draft"),
        (STORE, "<module>"),
    ] {
        add_decision(repo.path(), file, name, "");
    }

    let report = drop_gone_approvals(repo.path()).unwrap();

    // Only the approvals and cuts whose name or file is gone everywhere are dropped.
    let dropped: Vec<(String, String)> = report
        .first_dropped
        .iter()
        .map(|approval| (approval.file.display().to_string(), approval.name.clone()))
        .collect();
    let pair = |file: &str, name: &str| (file.to_owned(), name.to_owned());
    assert_eq!(
        dropped,
        [pair(STORE, "LoadAnswer"), pair("deleted.go", "Old")]
    );
    assert_eq!(
        list_decisions(repo.path()),
        [
            pair(STORE, "SaveAnswer"),
            pair("draft.go", "Draft"),
            pair(STORE, "<module>")
        ]
    );
}

/// `drop_gone_approvals` must copy the approvals file as it was before it drops anything, and name that copy.
/// Otherwise a wrong drop would lose the user's approvals with no way back.
#[test]
fn drop_gone_approvals_backs_up_file_first() {
    let repo = make_repo();
    add_decision(repo.path(), "deleted.go", "Old", "");
    let before = fs::read(repo.path().join(".git/pde/code-comments.json")).unwrap();

    let report = drop_gone_approvals(repo.path()).unwrap();

    let ApprovalsBackup::SavedAt(backup) = report.backup else {
        panic!("no backup: {:?}", report.backup);
    };
    assert_eq!(fs::read(backup).unwrap(), before);
}

/// `drop_gone_approvals` must count every dropped approval or cut, though it lists only the first five.
/// Otherwise a user who drops hundreds of approvals is told only five went.
#[test]
fn drop_gone_approvals_counts_all_lists_five() {
    let repo = make_repo();
    for name in ["Old1", "Old2", "Old3", "Old4", "Old5", "Old6"] {
        add_decision(repo.path(), "deleted.go", name, "");
    }

    let report = drop_gone_approvals(repo.path()).unwrap();

    assert_eq!(report.dropped, 6);
    assert_eq!(report.first_dropped.len(), 5);
}
