//! Reads the pde-gh-write request files, read-only. Only pde-gh-write
//! writes them, and only the user approves a request.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use jiff::Timestamp;
use serde_json::Value;

/// The folder of request files, `pde/pr-write/` in the state folder
/// `XDG_STATE_HOME` names, else in `~/.local/state`.
pub struct RequestFolder {
    folder: PathBuf,
}

/// The states of a request pde-gh-write has not finished. An
/// `approved` request waits for `gh` to start.
const OPEN_STATES: [&str; 3] = ["pending", "approved", "running"];

/// How long a request keeps its session after its last change, in any
/// state. A worker often files its next request soon after one runs.
const RECENT_REQUEST_AGE: Duration = Duration::from_secs(60 * 60);

/// One request, a pull request change that waits on the user or on
/// `gh`, or that ran lately. Each field is `None` when the request
/// came from outside Herdr or OpenCode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrRequest {
    pub pane_id: Option<String>,
    pub session_id: Option<String>,
}

/// The open requests, the others changed in the last hour, and how
/// many files did not parse.
#[derive(Debug, Default)]
pub struct OpenRequests {
    pub requests: Vec<PrRequest>,
    /// Requests that ran, failed, were declined or expired, changed in
    /// the last hour.
    pub recent: Vec<PrRequest>,
    pub bad_files: usize,
}

impl RequestFolder {
    pub fn new(folder: PathBuf) -> RequestFolder {
        RequestFolder { folder }
    }

    /// The folder, for the error when it cannot be read.
    pub(crate) fn folder(&self) -> &Path {
        &self.folder
    }

    /// Returns requests still pending, approved or running, and the
    /// others changed in the last hour by the system clock. A missing
    /// folder is no requests.
    pub fn list_open_requests(&self) -> io::Result<OpenRequests> {
        let entries = match fs::read_dir(&self.folder) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(OpenRequests::default());
            }
            Err(error) => return Err(error),
        };
        let now = SystemTime::now();
        let mut open = OpenRequests::default();
        for entry in entries {
            let path = entry?.path();
            if path.extension().is_none_or(|extension| extension != "json") {
                continue;
            }
            let raw: Option<Value> = fs::read(&path)
                .ok()
                .and_then(|bytes| serde_json::from_slice(&bytes).ok());
            let is_recent = || {
                raw.as_ref()
                    .is_some_and(|raw| is_changed_recently(raw, &path, now))
            };
            match raw.as_ref().and_then(parse_request) {
                Some((state, request)) if OPEN_STATES.contains(&state.as_str()) => {
                    open.requests.push(request);
                }
                Some((_, request)) if is_recent() => {
                    open.recent.push(request);
                }
                Some(_) => {}
                None => open.bad_files += 1,
            }
        }
        Ok(open)
    }
}

/// Returns one request file's state and request, or `None` when it has
/// no `state`.
fn parse_request(raw: &Value) -> Option<(String, PrRequest)> {
    let state = raw["state"].as_str()?.to_owned();
    let read_text = |field: &str| raw[field].as_str().map(str::to_owned);
    let request = PrRequest {
        pane_id: read_text("pane_id"),
        session_id: read_text("session_id"),
    };
    Some((state, request))
}

/// Tells whether a request changed in the last hour, by its `updated`
/// time, else its `created` time, else its file time. pde-gh-write
/// sets `updated` on each save, so a request the user approved long
/// after it was filed still counts once it runs. A request with no
/// readable time, or one changed after `now`, counts as recent, so it
/// keeps its session.
fn is_changed_recently(raw: &Value, path: &Path, now: SystemTime) -> bool {
    let saved = raw["updated"]
        .as_str()
        .or_else(|| raw["created"].as_str())
        .and_then(|text| text.parse::<Timestamp>().ok())
        .map(SystemTime::from);
    let changed = saved.or_else(|| fs::metadata(path).and_then(|data| data.modified()).ok());
    changed
        .is_none_or(|changed| now.duration_since(changed).unwrap_or_default() < RECENT_REQUEST_AGE)
}
