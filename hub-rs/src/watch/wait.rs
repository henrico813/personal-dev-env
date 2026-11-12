//! Runs `hub wait`, which a worker runs before it ends its turn to wait
//! on a long job, and keeps each open wait in `waits/NAME.json` in the
//! run folder. Read `run_wait` and `WaitFile::find_end` first. The watch
//! prods the worker once the job ends or the deadline passes. So no
//! wait runs inside the worker's chat, where a killed shell once left a
//! worker waiting for six hours.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use jiff::{SignedDuration, Timestamp, Zoned};
use serde::{Deserialize, Serialize};

use super::process;
use crate::commands::Outcome;
use crate::shared::run_folder::{self, RunFolder};
use crate::shared::{AgentName, HubError};

/// The run folder's folder of open waits.
const WAITS_FOLDER: &str = "waits";

/// What `hub wait` waits on, from its flags. clap lets exactly one of
/// `pid` and `file` through.
pub struct WaitRequest {
    pub name: AgentName,
    pub pid: Option<u32>,
    pub file: Option<PathBuf>,
    /// Minutes until the watch prods the worker anyway, at least 1.
    pub deadline_minutes: u32,
}

/// What ends a worker's wait, besides its deadline.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WaitFor {
    /// The process with this ID ends.
    Pid(u32),
    /// The file at `path` changes. `mtime_ns` is its time when the wait
    /// began, or `None` when it did not exist yet.
    File {
        path: PathBuf,
        mtime_ns: Option<u64>,
    },
}

/// One worker's open wait, kept until the watch prods the worker.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct WaitFile {
    pub name: String,
    pub wait_for: WaitFor,
    /// When the watch prods the worker even if the job still runs.
    pub deadline: Timestamp,
}

impl WaitFile {
    /// Returns why the wait ended by `now`, such as `pid 4242 ended`,
    /// or `None` while it runs.
    pub(crate) fn find_end(&self, now: &Zoned) -> Option<String> {
        let job_end = match &self.wait_for {
            WaitFor::Pid(pid) if !process::is_process_running(*pid) => {
                Some(format!("pid {pid} ended"))
            }
            WaitFor::File { path, mtime_ns } if read_mtime_ns(path) != *mtime_ns => {
                Some(format!("{} changed", path.display()))
            }
            WaitFor::Pid(_) | WaitFor::File { .. } => None,
        };
        job_end.or_else(|| {
            (now.timestamp() >= self.deadline).then(|| "its deadline passed".to_owned())
        })
    }
}

/// Writes NAME's wait to `waits/NAME.json`, replacing an older one, and
/// returns at once. A file wait keeps the file's time now, so any later
/// change, or the file appearing, ends it.
///
/// # Errors
///
/// `HubError::Usage` for neither or both of a PID and a file, or a
/// deadline past the year 9999. `HubError::File` when the file's path
/// cannot be made absolute or the wait cannot be written.
pub fn run_wait(run: &RunFolder, request: &WaitRequest, now: &Zoned) -> Result<Outcome, HubError> {
    let (wait_for, what) = match (request.pid, &request.file) {
        (Some(pid), None) => (WaitFor::Pid(pid), format!("pid {pid} ends")),
        (None, Some(file)) => {
            // The watch runs in another folder, where a relative path
            // names another file.
            let path = std::path::absolute(file).map_err(|source| HubError::File {
                path: file.clone(),
                source,
            })?;
            let what = format!("{} changes", path.display());
            let mtime_ns = read_mtime_ns(&path);
            (WaitFor::File { path, mtime_ns }, what)
        }
        _ => {
            return Err(HubError::Usage("give one of --pid and --file".to_owned()));
        }
    };
    let minutes = SignedDuration::from_mins(i64::from(request.deadline_minutes));
    let deadline = now.checked_add(minutes).map_err(|_| {
        HubError::Usage(format!(
            "--deadline {} is too far",
            request.deadline_minutes
        ))
    })?;
    let name = request.name.as_str();
    let wait = WaitFile {
        name: name.to_owned(),
        wait_for,
        deadline: deadline.timestamp(),
    };
    write_wait(run, &wait)?;
    Ok(Outcome::line(format!(
        "hub watch prods {name} once {what} or at {}; end your turn now",
        deadline.strftime("%H:%M")
    )))
}

/// Writes `waits/NAME.json` whole, replacing NAME's older wait.
fn write_wait(run: &RunFolder, wait: &WaitFile) -> Result<(), HubError> {
    let folder = run.path().join(WAITS_FOLDER);
    fs::create_dir_all(&folder).map_err(|source| HubError::File {
        path: folder.clone(),
        source,
    })?;
    let path = folder.join(format!("{}.json", wait.name));
    let text = serde_json::to_string(wait).map_err(|error| HubError::File {
        path: path.clone(),
        source: error.into(),
    })?;
    run_folder::write_file_whole(&path, &text)
}

/// Lists the run's open waits, by name. A file that does not parse is
/// left out, so it never hides a stall.
pub(crate) fn list_waits(run: &RunFolder) -> Result<Vec<WaitFile>, HubError> {
    let folder = run.path().join(WAITS_FOLDER);
    let file_error = |source| HubError::File {
        path: folder.clone(),
        source,
    };
    let entries = match fs::read_dir(&folder) {
        Ok(entries) => entries,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => return Err(file_error(source)),
    };
    let mut waits = Vec::new();
    for entry in entries {
        let path = entry.map_err(file_error)?.path();
        if path.extension().is_none_or(|extension| extension != "json") {
            continue;
        }
        let parsed = fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str::<WaitFile>(&text).ok());
        waits.extend(parsed);
    }
    waits.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(waits)
}

/// Deletes the worker's wait file while it still holds `wait`. A newer
/// wait the worker wrote meanwhile stays, and a wait already gone is no
/// error.
pub(crate) fn remove_wait_if_unchanged(run: &RunFolder, wait: &WaitFile) -> Result<(), HubError> {
    let path = run
        .path()
        .join(WAITS_FOLDER)
        .join(format!("{}.json", wait.name));
    let saved = fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str::<WaitFile>(&text).ok());
    if saved.as_ref() != Some(wait) {
        return Ok(());
    }
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(HubError::File { path, source }),
    }
}

/// Returns the file's modification time in nanoseconds since 1970, or
/// `None` when it cannot be read, as for a missing file.
fn read_mtime_ns(path: &Path) -> Option<u64> {
    let modified = fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()?;
    let since_epoch = modified.duration_since(SystemTime::UNIX_EPOCH).ok()?;
    Some(u64::try_from(since_epoch.as_nanos()).unwrap_or(u64::MAX))
}
