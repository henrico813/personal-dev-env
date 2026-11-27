//! Reads the open pull requests in the `prs` tool's cache, with each
//! one's check result, for page 5 and the IN REVIEW heading. Read
//! `read_pull_requests` first.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// One open pull request from the cache, such as `app #301`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct PullRequestLine {
    /// The repository as `owner/repo`, which a card's `prs:` link must
    /// name.
    pub name_with_owner: String,
    pub number: u64,
    pub title: String,
    pub check: CheckResult,
}

/// The combined result of a pull request's checks. The order is the
/// order page 5 lists pull requests in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) enum CheckResult {
    Fails,
    Running,
    Pass,
    /// GitHub has run no check yet.
    None,
}

impl CheckResult {
    /// Returns the word page 5 shows, empty with no checks.
    pub(crate) fn as_word(self) -> &'static str {
        match self {
            CheckResult::Fails => "fails",
            CheckResult::Running => "running",
            CheckResult::Pass => "pass",
            CheckResult::None => "",
        }
    }
}

impl PullRequestLine {
    /// Returns the repository name after the owner, such as `app`.
    pub(crate) fn find_repo_name(&self) -> &str {
        let (_, repo) = self
            .name_with_owner
            .split_once('/')
            .unwrap_or(("", &self.name_with_owner));
        repo
    }

    /// Tells whether a card's `prs:` link, such as
    /// `https://github.com/demo/app/pull/301`, names this pull request.
    pub(crate) fn is_named_by(&self, link: &str) -> bool {
        parse_pr_link(link).is_some_and(|(name_with_owner, number)| {
            number == self.number && name_with_owner == self.name_with_owner
        })
    }

    /// Tells whether a card's `prs:` link names a pull request of this
    /// one's repository, whatever its number.
    pub(crate) fn shares_repo_with(&self, link: &str) -> bool {
        parse_pr_link(link)
            .is_some_and(|(name_with_owner, _)| name_with_owner == self.name_with_owner)
    }
}

/// Returns the repository and number a pull request link names, such
/// as `demo/app` and 301 for `https://github.com/demo/app/pull/301`.
fn parse_pr_link(link: &str) -> Option<(String, u64)> {
    let mut parts = link.trim_end_matches('/').rsplit('/');
    let (Some(number), Some("pull"), Some(repo), Some(owner)) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return None;
    };
    Some((format!("{owner}/{repo}"), number.parse().ok()?))
}

/// Returns each pull request in the `prs` list at `path`, which holds
/// only open ones, or `None` when the file is missing or does not
/// parse, as before the first `prs` run.
pub(crate) fn read_pull_requests(path: &Path) -> Option<Vec<PullRequestLine>> {
    let list: PrList = serde_json::from_str(&fs::read_to_string(path).ok()?).ok()?;
    let lines = list.pull_requests.into_iter().map(|pull_request| {
        let checks = pull_request.status_check_rollup.unwrap_or_default();
        PullRequestLine {
            name_with_owner: pull_request.repository.name_with_owner,
            number: pull_request.number,
            title: pull_request.title,
            check: decide_check_result(&checks),
        }
    });
    Some(lines.collect())
}

/// Returns `Fails` when any check failed, else `Running` while any has
/// not finished, else `Pass`.
fn decide_check_result(checks: &[Check]) -> CheckResult {
    if checks.is_empty() {
        CheckResult::None
    } else if checks.iter().any(Check::is_failing) {
        CheckResult::Fails
    } else if checks.iter().any(Check::is_running) {
        CheckResult::Running
    } else {
        CheckResult::Pass
    }
}

/// The part of `list.json` the board reads.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PrList {
    pull_requests: Vec<PullRequest>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PullRequest {
    number: u64,
    title: String,
    repository: Repository,
    /// `null` until GitHub has run a check on the pull request.
    status_check_rollup: Option<Vec<Check>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Repository {
    name_with_owner: String,
}

/// A check run, or a commit status when `kind` is `StatusContext`.
#[derive(Deserialize)]
struct Check {
    #[serde(rename = "__typename")]
    kind: Option<String>,
    status: Option<String>,
    conclusion: Option<String>,
    state: Option<String>,
}

impl Check {
    /// Tells whether the check failed, by the rule `prs` uses, so both
    /// tools count the same pull requests as failing. A check run that
    /// has not completed is pending, not failed.
    fn is_failing(&self) -> bool {
        let outcome = if self.is_status() {
            self.state.as_deref()
        } else if self.status.as_deref() == Some("COMPLETED") {
            self.conclusion.as_deref()
        } else {
            return false;
        };
        !matches!(
            outcome,
            Some("SUCCESS" | "NEUTRAL" | "SKIPPED" | "PENDING" | "EXPECTED")
        )
    }

    /// Tells whether the check has not finished yet.
    fn is_running(&self) -> bool {
        if self.is_status() {
            matches!(self.state.as_deref(), Some("PENDING" | "EXPECTED"))
        } else {
            self.status.as_deref() != Some("COMPLETED")
        }
    }

    fn is_status(&self) -> bool {
        self.kind.as_deref() == Some("StatusContext")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A card's link must name a pull request only when owner, repo and
    /// number all match, or page 5 puts another repository's PR #301
    /// under the card.
    #[test]
    fn is_named_by_needs_repo_and_number() {
        let pull_request = PullRequestLine {
            name_with_owner: "demo/app".to_owned(),
            number: 301,
            title: "Retry uploads".to_owned(),
            check: CheckResult::Fails,
        };

        assert!(pull_request.is_named_by("https://github.com/demo/app/pull/301"));
        assert!(!pull_request.is_named_by("https://github.com/other/app/pull/301"));
        assert!(!pull_request.is_named_by("https://github.com/demo/app/pull/302"));
    }
}
