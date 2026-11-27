//! Counts the open pull requests in the `prs` tool's cache, and those
//! with a failed check, for the In review heading. Read
//! `read_pr_count` first.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// The open pull requests in the cache, and how many of them have a
/// failed check.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub(crate) struct PrCount {
    pub open: usize,
    pub failing: usize,
}

/// Returns the count from the `prs` list at `path`, which holds only
/// open pull requests, or `None` when the file is missing or does not
/// parse, as before the first `prs` run.
pub(crate) fn read_pr_count(path: &Path) -> Option<PrCount> {
    let list: PrList = serde_json::from_str(&fs::read_to_string(path).ok()?).ok()?;
    let failing = list
        .pull_requests
        .iter()
        .filter(|pull_request| {
            let mut checks = pull_request.status_check_rollup.iter().flatten();
            checks.any(Check::is_failing)
        })
        .count();
    Some(PrCount {
        open: list.pull_requests.len(),
        failing,
    })
}

/// The part of `list.json` the count reads.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PrList {
    pull_requests: Vec<PullRequest>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PullRequest {
    /// `null` until GitHub has run a check on the pull request.
    status_check_rollup: Option<Vec<Check>>,
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
        let outcome = if self.kind.as_deref() == Some("StatusContext") {
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
}
