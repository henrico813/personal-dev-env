//! Runs `hub requests`, which reads each pde-gh-write request file
//! fresh, so the hub can tell an approval that waits from one that
//! already ran or expired. `hub status` still says `approval waits`
//! for approved and running requests too. This command can go once
//! the question queue lists live approvals with their state.

use jiff::{SignedDuration, Timestamp};
use serde_json::{Value, json};

use super::Outcome;
use super::status::format_columns;
use crate::external::requests::SavedRequest;
use crate::shared::facts::is_request_from;
use crate::shared::{HubError, Tools, format_span};

/// How far back a finished request of a live session still shows.
const SHOWN_AGE: SignedDuration = SignedDuration::from_secs(24 * 60 * 60);

/// Prints one row per request, newest change first. It shows each live
/// session's requests changed in the last day, and every open request
/// whatever its session. SESSION is `-` for a request no live session
/// filed. AGE counts from the last change.
///
/// # Errors
///
/// `HubError::Herdr` when Herdr cannot list the agents, and
/// `HubError::File` when the request folder cannot be read.
pub fn run_requests(tools: &Tools<'_>, as_json: bool, now: Timestamp) -> Result<Outcome, HubError> {
    let agents = tools.herdr.list_agents()?;
    let saved = tools
        .requests
        .list_saved_requests(now)
        .map_err(|source| HubError::File {
            path: tools.requests.folder().to_owned(),
            source,
        })?;
    let mut rows: Vec<(Option<&str>, SavedRequest)> = saved
        .requests
        .into_iter()
        .filter_map(|request| {
            let session = agents
                .iter()
                .find(|agent| is_request_from(agent, &request.source))
                .map(|agent| agent.name.as_str());
            let is_new = request
                .changed
                .is_none_or(|changed| now.duration_since(changed) < SHOWN_AGE);
            (request.is_open || session.is_some() && is_new).then_some((session, request))
        })
        .collect();
    rows.sort_by(|left, right| right.1.changed.cmp(&left.1.changed));
    let problem =
        (saved.bad_files > 0).then(|| format!("requests: {} files did not parse", saved.bad_files));
    let lines = if as_json {
        vec![format_requests_json(&rows, problem, now)]
    } else {
        format_requests_text(&rows, problem, now)
    };
    Ok(Outcome { lines })
}

/// Writes a table, or one line when no request shows, then the problem.
fn format_requests_text(
    rows: &[(Option<&str>, SavedRequest)],
    problem: Option<String>,
    now: Timestamp,
) -> Vec<String> {
    let mut lines = if rows.is_empty() {
        vec!["No requests from live sessions in the last day.".to_owned()]
    } else {
        let mut table = vec![["REF", "SESSION", "STATE", "AGE", "PULL REQUEST"].map(str::to_owned)];
        table.extend(rows.iter().map(|(session, request)| {
            [
                request.reference(),
                session.unwrap_or("-").to_owned(),
                request.state.clone(),
                format_age(request, now),
                request.pull_request.clone(),
            ]
        }));
        format_columns(&table)
    };
    lines.extend(problem);
    lines
}

/// Writes the same rows as one JSON object, with the full ID.
fn format_requests_json(
    rows: &[(Option<&str>, SavedRequest)],
    problem: Option<String>,
    now: Timestamp,
) -> String {
    let requests: Vec<Value> = rows
        .iter()
        .map(|(session, request)| {
            json!({
                "ref": request.reference(),
                "id": request.id,
                "session": session,
                "state": request.state,
                "age": format_age(request, now),
                "changed": request.changed.map(|changed| changed.to_string()),
                "pull_request": request.pull_request,
            })
        })
        .collect();
    json!({ "requests": requests, "problems": problem.into_iter().collect::<Vec<_>>() }).to_string()
}

/// Returns the time since the last change, or `-` when it is unknown.
fn format_age(request: &SavedRequest, now: Timestamp) -> String {
    request.changed.map_or_else(
        || "-".to_owned(),
        |changed| format_span(now.duration_since(changed)),
    )
}
