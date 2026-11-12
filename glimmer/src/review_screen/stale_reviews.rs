//! This module takes stale reviews off Ctrl+b e's list before the list shows.
//! A waiting review is stale once its pull request is merged or closed, or
//! once its worktree has another branch checked out. Read the
//! `drop_stale_reviews(worktrees, gh)` function to understand the module.

use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use serde::Deserialize;

use super::picker::{Waiting, WaitingReview, find_waiting_reviews};
use crate::language_rules::run_for_at_most;
use crate::shared::git::{self, CommitId};
use crate::shared::review_session;

/// Holds the command for GitHub's command-line tool, which finds the
/// repository from the worktree's remote.
pub(super) const GH_COMMAND: &[&str] = &["gh"];

/// Gives the longest a `gh` call may run, so a slow network holds up Ctrl+b e
/// for at most this long.
const GH_TIME_LIMIT: Duration = Duration::from_secs(3);

/// Returns the reviews in `worktrees` that need the user, newest first, as
/// `find_waiting_reviews` does, once stale ones are dropped. Dismisses each
/// waiting review whose worktree has another branch checked out, or whose
/// pull request `gh` finds merged or closed. Takes each worktree with no
/// Review Session, or a done one, off the waiting list. When `gh` is not
/// given, is missing or fails, such as offline, a review's pull request
/// counts as open.
pub(super) fn drop_stale_reviews(
    worktrees: Vec<PathBuf>,
    gh: Option<&[&str]>,
) -> Vec<WaitingReview> {
    let (leftovers, worktrees): (Vec<PathBuf>, Vec<PathBuf>) = worktrees
        .into_iter()
        .partition(|worktree| review_session::is_leftover(worktree));
    if !leftovers.is_empty() {
        // The picker skips leftovers too, so a failed save hides nothing.
        let _ = review_session::remove_leftovers_from_waiting_list(&leftovers);
    }
    let reviews = find_waiting_reviews(worktrees);
    // Checks run at once, so Ctrl+b e waits about as long as one `gh` call.
    let stale: Vec<bool> = thread::scope(|scope| {
        let checks: Vec<_> = reviews
            .iter()
            .map(|review| scope.spawn(move || is_stale(review, gh)))
            .collect();
        checks
            .into_iter()
            .map(|check| check.join().expect("a staleness check does not panic"))
            .collect()
    });
    let mut kept = Vec::new();
    for (review, stale) in reviews.into_iter().zip(stale) {
        // A review that cannot be dismissed, such as one the user answered in the
        // meantime, stays for the picker.
        if stale && review_session::dismiss(&review.root).is_ok() {
            continue;
        }
        kept.push(review);
    }
    kept
}

/// Returns true when the review waits for User Feedback and its worktree has
/// another branch checked out, or its pull request is merged or closed. A
/// review whose last write did not finish, or that cannot be read, is never
/// stale, since only the user can check it.
fn is_stale(review: &WaitingReview, gh: Option<&[&str]>) -> bool {
    let Waiting::Review { origin, .. } = &review.waiting else {
        return false;
    };
    let Ok(current) = git::read_current_branch(&review.root) else {
        return false;
    };
    // Git names a detached HEAD "HEAD", as during a rebase, and that worktree
    // keeps its review.
    if current != origin.branch && current != "HEAD" {
        return true;
    }
    gh.is_some_and(|gh| is_pull_request_finished(gh, &review.root, &origin.branch))
}

/// Returns true when the newest pull request from `branch` is merged or
/// closed and its last commit is in the worktree's HEAD. A branch with no
/// pull request, or a `gh` that fails, gives false.
fn is_pull_request_finished(gh: &[&str], worktree: &Path, branch: &str) -> bool {
    /// Holds the fields asked of `gh`, such as `MERGED` and a commit hash.
    #[derive(Deserialize)]
    struct PullRequest {
        state: String,
        #[serde(rename = "headRefOid")]
        last_commit: CommitId,
    }

    let mut command: Vec<&str> = gh.to_vec();
    command.extend([
        "pr",
        "list",
        "--head",
        branch,
        "--state",
        "all",
        "--json",
        "state,headRefOid",
        "--limit",
        "1",
    ]);
    let Ok(printed) = run_for_at_most(&command, worktree, "", GH_TIME_LIMIT) else {
        return false;
    };
    let pull_requests: Vec<PullRequest> = serde_json::from_str(&printed).unwrap_or_default();
    // `gh` matches the branch by name only, so an old pull request from a
    // reused name or a fork counts only when its last commit is in HEAD.
    pull_requests.first().is_some_and(|newest| {
        matches!(newest.state.as_str(), "MERGED" | "CLOSED")
            && git::is_ancestor_of_head(worktree, &newest.last_commit).unwrap_or(false)
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::json;

    use super::*;
    use crate::review_screen::test_repo::MadeUpRepo;
    use crate::shared::review_session::{add_to_waiting_list, read_waiting_list};

    /// Fakes `gh pr list` for the test branch. It prints the worktree's
    /// `pr-list` file, and fails as `gh` does offline without one or on other
    /// arguments.
    const FAKE_GH: &[&str] = &[
        "sh",
        "-c",
        r#"[ "$*" = "pr list --head feat/screen --state all --json state,headRefOid --limit 1" ] || exit 2; cat pr-list"#,
        "gh",
    ];

    /// Names the status of a review that waits for User Feedback.
    const WAITING: &str = "waiting_for_user_feedback";

    /// `drop_stale_reviews` must dismiss a waiting review once its pull
    /// request is merged or closed, and keep it while the pull request is
    /// open, missing, from an older branch of the same name or a commit the
    /// clone lacks, or `gh` cannot answer. Otherwise Ctrl+b e offers reviews
    /// nobody needs, or hides one the user still has to answer.
    #[test]
    fn drop_stale_reviews_follows_pull_request_state() {
        let cases = [
            (
                "merged",
                Some(r#"[{"state":"MERGED","headRefOid":"NEW"}]"#),
                "dismissed",
            ),
            (
                "closed",
                Some(r#"[{"state":"CLOSED","headRefOid":"NEW"}]"#),
                "dismissed",
            ),
            (
                "open",
                Some(r#"[{"state":"OPEN","headRefOid":"NEW"}]"#),
                WAITING,
            ),
            ("no pull request", Some("[]"), WAITING),
            (
                "older branch merged",
                Some(r#"[{"state":"MERGED","headRefOid":"OLD"}]"#),
                WAITING,
            ),
            (
                "commit not in clone",
                Some(
                    r#"[{"state":"MERGED","headRefOid":"1111111111111111111111111111111111111111"}]"#,
                ),
                WAITING,
            ),
            ("gh fails", None, WAITING),
        ];

        for (case, pr_list, status) in cases {
            let repo = MadeUpRepo::new("feat/screen", &[]);
            repo.save_review(WAITING, vec![], json!({}));
            // A commit that left the branch stands for an older branch's last commit.
            repo.run_git(&["commit", "--quiet", "--allow-empty", "-m", "Old work"]);
            let old = git::resolve_commit(repo.path(), "HEAD").unwrap();
            repo.run_git(&["reset", "--quiet", "--hard", "HEAD~1"]);
            let head = git::resolve_commit(repo.path(), "HEAD").unwrap();
            if let Some(pr_list) = pr_list {
                let pr_list = pr_list
                    .replace("\"NEW\"", &format!("\"{}\"", head.as_str()))
                    .replace("\"OLD\"", &format!("\"{}\"", old.as_str()));
                fs::write(repo.path().join("pr-list"), pr_list).unwrap();
            }

            let kept = drop_stale_reviews(vec![repo.path().to_owned()], Some(FAKE_GH));

            assert_eq!(repo.read_review()["status"], status, "{case}");
            assert_eq!(kept.is_empty(), status == "dismissed", "{case}");
        }
    }

    /// `drop_stale_reviews` must dismiss a waiting review once its worktree
    /// has another branch checked out, but keep it while HEAD is detached, as
    /// during a rebase. Otherwise Ctrl+b e keeps offering a review of a branch
    /// the worktree left, or a rebase hides a review the user still has to
    /// answer.
    #[test]
    fn drop_stale_reviews_follows_checked_out_branch() {
        let cases: [(&str, &[&str], &str); 2] = [
            (
                "other branch",
                &["checkout", "--quiet", "-b", "main"],
                "dismissed",
            ),
            ("detached", &["checkout", "--quiet", "--detach"], WAITING),
        ];

        for (case, checkout, status) in cases {
            let repo = MadeUpRepo::new("feat/screen", &[]);
            repo.save_review(WAITING, vec![], json!({}));
            repo.run_git(checkout);

            drop_stale_reviews(vec![repo.path().to_owned()], None);

            assert_eq!(repo.read_review()["status"], status, "{case}");
        }
    }

    /// `drop_stale_reviews` must take a worktree with no Review Session, or a
    /// done one, off the waiting list, and keep a dismissed review listed.
    /// Otherwise leftover entries pile up in the list, or
    /// `glimmer restore NAME` can no longer find a dismissed review.
    #[test]
    fn drop_stale_reviews_unlists_only_leftover_worktrees() {
        let empty = MadeUpRepo::new("main", &[]);
        let done = MadeUpRepo::new("feat/screen", &[]);
        done.save_review("done", vec![], json!({}));
        let dismissed = MadeUpRepo::new("feat/screen", &[]);
        dismissed.save_review("dismissed", vec![], json!({}));
        let worktrees = vec![
            empty.path().to_owned(),
            done.path().to_owned(),
            dismissed.path().to_owned(),
        ];
        for worktree in &worktrees {
            add_to_waiting_list(worktree).unwrap();
        }

        drop_stale_reviews(worktrees.clone(), None);

        let listed: Vec<PathBuf> = read_waiting_list()
            .unwrap()
            .into_iter()
            .filter(|listed| worktrees.contains(listed))
            .collect();
        assert_eq!(listed, [dismissed.path().to_owned()]);
    }
}
