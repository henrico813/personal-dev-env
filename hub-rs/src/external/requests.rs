//! Reads the pde-gh-write request files, read-only. Only pde-gh-write
//! writes them, and only the user approves a request.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use jiff::{SignedDuration, Timestamp};
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
const RECENT_REQUEST_AGE: SignedDuration = SignedDuration::from_secs(60 * 60);

/// pde-gh-write's `approval.Expiry`. Its runner gives up on a pending
/// or approved request this long after the last change, so `gh` never
/// runs for it.
const REQUEST_EXPIRY: SignedDuration = SignedDuration::from_secs(4 * 60 * 60);

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

/// One request file as pde-gh-write saved it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedRequest {
    pub id: String,
    /// The saved `state`, or `expired` for a pending or approved
    /// request past `REQUEST_EXPIRY`.
    pub state: String,
    /// pde-gh-write may still act on it. Its state is pending, approved or running,
    /// and changed within `REQUEST_EXPIRY`. A run that died stays
    /// `running` in its file, and pde-gh-write never retries it.
    pub is_open: bool,
    /// The last change, from `updated`, else `created`, else the file
    /// time. `None` when none can be read.
    pub changed: Option<Timestamp>,
    /// The repository and `gh pr` action, such as
    /// `personal-dev-env #15 merge`.
    pub pull_request: String,
    pub source: PrRequest,
}

impl SavedRequest {
    /// Returns the short reference the design gives an approval, `A`
    /// and the ID's first six characters, such as `A0016cd`.
    pub fn reference(&self) -> String {
        format!("A{}", self.id.chars().take(6).collect::<String>())
    }

    /// Returns how long a pending request has before pde-gh-write lets
    /// it expire, or `None` when it is not pending or has no time.
    pub fn find_time_left(&self, now: Timestamp) -> Option<SignedDuration> {
        let changed = self.changed.filter(|_| self.state == "pending")?;
        Some(REQUEST_EXPIRY - now.duration_since(changed))
    }
}

/// Every request file, and how many did not parse.
#[derive(Debug, Default)]
pub struct SavedRequests {
    pub requests: Vec<SavedRequest>,
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
        let now = Timestamp::now();
        let saved = self.list_saved_requests(now)?;
        let mut open = OpenRequests {
            bad_files: saved.bad_files,
            ..OpenRequests::default()
        };
        for request in saved.requests {
            if request.is_open {
                open.requests.push(request.source);
            } else if is_changed_recently(request.changed, now) {
                open.recent.push(request.source);
            }
        }
        Ok(open)
    }

    /// Reads every request file, judging expiry at `now`. A missing
    /// folder is no requests.
    pub fn list_saved_requests(&self, now: Timestamp) -> io::Result<SavedRequests> {
        let entries = match fs::read_dir(&self.folder) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(SavedRequests::default());
            }
            Err(error) => return Err(error),
        };
        let mut saved = SavedRequests::default();
        for entry in entries {
            let path = entry?.path();
            if path.extension().is_none_or(|extension| extension != "json") {
                continue;
            }
            let request = fs::read(&path)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
                .and_then(|raw| parse_request(&raw, &path, now));
            match request {
                Some(request) => saved.requests.push(request),
                None => saved.bad_files += 1,
            }
        }
        Ok(saved)
    }
}

/// Returns one request file's request, or `None` when it has no
/// `state`.
fn parse_request(raw: &Value, path: &Path, now: Timestamp) -> Option<SavedRequest> {
    let saved_state = raw["state"].as_str()?;
    let read_text = |field: &str| raw[field].as_str().map(str::to_owned);
    let changed = read_changed_time(raw, path);
    let is_old = changed.is_some_and(|changed| now.duration_since(changed) > REQUEST_EXPIRY);
    let is_expired = is_old && matches!(saved_state, "pending" | "approved");
    let id = read_text("id").unwrap_or_else(|| {
        let stem = path.file_stem().unwrap_or_default();
        stem.to_string_lossy().into_owned()
    });
    Some(SavedRequest {
        id,
        state: if is_expired { "expired" } else { saved_state }.to_owned(),
        is_open: OPEN_STATES.contains(&saved_state) && !is_old,
        changed,
        pull_request: describe_pull_request(raw),
        source: PrRequest {
            pane_id: read_text("pane_id"),
            session_id: read_text("session_id"),
        },
    })
}

/// Returns the request's last change. pde-gh-write sets `updated` on
/// each save, so a request the user approved long after it was filed
/// counts from when it ran.
fn read_changed_time(raw: &Value, path: &Path) -> Option<Timestamp> {
    let saved = raw["updated"]
        .as_str()
        .or_else(|| raw["created"].as_str())
        .and_then(|text| text.parse::<Timestamp>().ok());
    saved.or_else(|| {
        let modified = fs::metadata(path).and_then(|data| data.modified()).ok()?;
        Timestamp::try_from(modified).ok()
    })
}

/// Returns the repository's short name, the pull request number when
/// the action takes one, and the action, from `repo` and `args` such as
/// `["pr", "merge", "15", ...]`.
fn describe_pull_request(raw: &Value) -> String {
    let repo = raw["repo"]
        .as_str()
        .and_then(|repo| repo.rsplit('/').next())
        .unwrap_or("-");
    let args: Vec<&str> = raw["args"]
        .as_array()
        .map(|args| args.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let action = args.get(1).copied().unwrap_or("-");
    match args.get(2) {
        Some(number) if !number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit()) => {
            format!("{repo} #{number} {action}")
        }
        _ => format!("{repo} {action}"),
    }
}

/// Tells whether a request changed in the last hour. A request with no
/// readable time, or one changed after `now`, counts as recent, so it
/// keeps its session.
fn is_changed_recently(changed: Option<Timestamp>, now: Timestamp) -> bool {
    changed.is_none_or(|changed| now.duration_since(changed) < RECENT_REQUEST_AGE)
}
