//! Reads the pde-gh-write request files, read-only. Only pde-gh-write
//! writes them, and only the user approves a request.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::Value;

/// The folder of request files, `pde/pr-write/` in the state folder
/// `XDG_STATE_HOME` names, else in `~/.local/state`.
pub struct RequestFolder {
    folder: PathBuf,
}

/// The states of a request pde-gh-write has not finished. An
/// `approved` request waits for `gh` to start.
const OPEN_STATES: [&str; 3] = ["pending", "approved", "running"];

/// One open request, a pull request change that waits on the user or
/// on `gh`. Each field is `None` when the request came from outside
/// Herdr or OpenCode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrRequest {
    pub pane_id: Option<String>,
    pub session_id: Option<String>,
}

/// The open requests, and how many files did not parse.
#[derive(Debug, Default)]
pub struct OpenRequests {
    pub requests: Vec<PrRequest>,
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

    /// Returns requests still pending, approved or running. A missing
    /// folder is no requests.
    pub fn list_open_requests(&self) -> io::Result<OpenRequests> {
        let entries = match fs::read_dir(&self.folder) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(OpenRequests::default());
            }
            Err(error) => return Err(error),
        };
        let mut open = OpenRequests::default();
        for entry in entries {
            let path = entry?.path();
            if path.extension().is_none_or(|extension| extension != "json") {
                continue;
            }
            let raw: Option<Value> = fs::read(&path)
                .ok()
                .and_then(|bytes| serde_json::from_slice(&bytes).ok());
            match raw.as_ref().and_then(parse_request) {
                Some((state, request)) if OPEN_STATES.contains(&state.as_str()) => {
                    open.requests.push(request);
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
