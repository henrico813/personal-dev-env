//! This module holds made-up repositories and Review Sessions for the screen's
//! tests, so no test reaches the user's own reviews or waiting list. Read the
//! `MadeUpRepo::new(branch, files)` and
//! `MadeUpRepo::save_review(status, requests, feedback)` functions to
//! understand the module.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, PoisonError};

use serde_json::{Value, json};

use crate::shared::review_session::set_test_waiting_list_folder;

/// Counts the made-up repositories of this test process that exist now, so
/// the last one dropped removes the waiting list's folder.
static LIVE_REPOS: Mutex<usize> = Mutex::new(0);

/// A made-up Git repository in a new temporary folder, deleted when dropped.
pub(super) struct MadeUpRepo(PathBuf);

impl MadeUpRepo {
    /// Makes a repository on `branch` with `files` committed, and keeps this
    /// test process's waiting list in the build folder until the last
    /// repository is dropped.
    pub(super) fn new(branch: &str, files: &[(&str, &str)]) -> Self {
        set_test_waiting_list_folder(&find_test_waiting_list_folder());
        *LIVE_REPOS.lock().unwrap_or_else(PoisonError::into_inner) += 1;
        let output = Command::new("mktemp").arg("-d").output().unwrap();
        let repo = Self(PathBuf::from(
            String::from_utf8(output.stdout).unwrap().trim_end(),
        ));
        for (name, contents) in files {
            fs::write(repo.path().join(name), contents).unwrap();
        }
        repo.run_git(&["init", "--quiet", "-b", branch]);
        repo.run_git(&["add", "--all"]);
        repo.run_git(&["commit", "--quiet", "--allow-empty", "-m", "Add the files"]);
        repo
    }

    /// Returns the repository's top folder.
    pub(super) fn path(&self) -> &Path {
        &self.0
    }

    /// Runs Git without the test machine's own settings.
    pub(super) fn run_git(&self, args: &[&str]) {
        let output = Command::new("git")
            .arg("-C")
            .arg(self.path())
            .args(args)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_AUTHOR_NAME", "Review Test")
            .env("GIT_AUTHOR_EMAIL", "review@example.invalid")
            .env("GIT_COMMITTER_NAME", "Review Test")
            .env("GIT_COMMITTER_EMAIL", "review@example.invalid")
            .output()
            .unwrap();
        assert!(output.status.success(), "{args:?}: {output:?}");
    }

    /// Saves a Review Session in `status` with one list of `requests` and
    /// `feedback` by request ID.
    pub(super) fn save_review(&self, status: &str, requests: Vec<Value>, feedback: Value) {
        let review = json!({
            "status": status,
            "origin": {"repo": "personal-dev-env", "branch": "feat/screen", "agent_session": "ses_123"},
            "lists": [{"base": "4f2a9c1e", "requests": requests}],
            "feedback": feedback,
        });
        let folder = self.path().join(".git/pde");
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("review.json"), review.to_string()).unwrap();
    }

    /// Returns the saved Review Session as JSON.
    pub(super) fn read_review(&self) -> Value {
        let saved = fs::read(self.path().join(".git/pde/review.json")).unwrap();
        serde_json::from_slice(&saved).unwrap()
    }
}

impl Drop for MadeUpRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
        let mut live_repos = LIVE_REPOS.lock().unwrap_or_else(PoisonError::into_inner);
        *live_repos -= 1;
        // A test reaches the waiting list only while it holds a repository, so none is using it now.
        if *live_repos == 0 {
            let _ = fs::remove_dir_all(find_test_waiting_list_folder());
        }
    }
}

/// Returns this test process's waiting-list folder, beside the test program
/// in the build folder.
fn find_test_waiting_list_folder() -> PathBuf {
    let build_folder = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_owned();
    build_folder.join(format!("waiting-list-{}", std::process::id()))
}

/// Holds Go source with a comment above each of two functions.
pub(super) const STORE_SOURCE: &str =
    "package store\n\n// Saves it.\nfunc Save() {}\n\n// Loads it.\nfunc Load() {}\n";

/// Returns a Feedback Request about the comment above function `name` in the
/// sample Go file, with an Agent Suggestion of `kind`.
pub(super) fn store_request(id: u32, name: &str, kind: Value) -> Value {
    let (text, line) = match name {
        "Save" => ("// Saves it.\n", 3),
        _ => ("// Loads it.\n", 6),
    };
    json!({
        "id": id,
        "file": "store.go",
        "name": name,
        "reason": "new",
        "text": text,
        "can_reword": true,
        "line": line,
        "suggestion": {"kind": kind, "reasoning": "Says what it does.", "also_at": []},
    })
}
