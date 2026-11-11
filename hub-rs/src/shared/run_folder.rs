//! Names the files in the hub's run folder, and reads and writes
//! reports, notes and the watcher files. Read `RunFolder` first. Every
//! whole-file write goes through `write_file_whole`.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::{Duration, SystemTime};

use jiff::{Timestamp, Zoned};
use regex::Regex;
use serde::{Deserialize, Serialize};

use super::{AgentName, HubError};

/// The model in `rules.md`, written as `--model '<provider>/<model>#<effort>'`.
static RULES_MODEL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"--model '([^']+)'").unwrap());

/// The file that keeps the last minute pass's mark.
const MINUTE_MARK_FILE: &str = ".minute-pass.json";

/// One unread `report-NAME.md`.
#[derive(Clone, Debug)]
pub(crate) struct ReportFile {
    pub name: String,
    /// Its modification time, which tells one version from the next.
    pub mtime_ns: u64,
    pub age: Duration,
}

/// The names with an unread report, and every name that ever
/// reported, read or not.
#[derive(Debug, Default)]
pub(crate) struct ReportedNames {
    pub unread: BTreeSet<String>,
    pub any: BTreeSet<String>,
}

/// When the last minute pass ran, and what it saw, saved in
/// `.minute-pass.json` so a restarted `hub next` keeps the limit.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct MinuteMark {
    pub at: Timestamp,
    /// Each agent's Herdr state change count, by name.
    pub state_seqs: BTreeMap<String, u64>,
    /// How many `read` events the log held.
    pub read_count: usize,
}

/// Holds `.watcher.lock` while a `hub next` runs. Dropping it, or the
/// process ending, lets the next watcher in.
pub(crate) struct WatcherLock {
    _file: File,
}

/// The hub's run folder, checked to exist.
pub struct RunFolder {
    /// Absolute, so a new hub's pane gets a `HUB_RUN` that works from
    /// any folder.
    path: PathBuf,
}

impl RunFolder {
    /// Returns the folder, or `HubError::Usage` when it does not exist.
    pub fn open(path: &Path) -> Result<RunFolder, HubError> {
        match std::path::absolute(path) {
            Ok(absolute) if absolute.is_dir() => Ok(RunFolder { path: absolute }),
            _ => Err(HubError::Usage(format!(
                "no run folder at {}",
                path.display()
            ))),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Lists unread reports with their modification time and age, by
    /// name.
    pub(crate) fn list_reports(&self, now: &Zoned) -> Result<Vec<ReportFile>, HubError> {
        let entries = fs::read_dir(&self.path).map_err(|source| self.make_file_error(source))?;
        let now = SystemTime::from(now.timestamp());
        let mut reports = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|source| self.make_file_error(source))?;
            let file_name = entry.file_name().to_string_lossy().into_owned();
            let Some(name) = file_name
                .strip_prefix("report-")
                .and_then(|rest| rest.strip_suffix(".md"))
            else {
                continue;
            };
            let modified = entry
                .metadata()
                .and_then(|metadata| metadata.modified())
                .map_err(|source| HubError::File {
                    path: entry.path(),
                    source,
                })?;
            let since_epoch = modified
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default();
            reports.push(ReportFile {
                name: name.to_owned(),
                mtime_ns: u64::try_from(since_epoch.as_nanos()).unwrap_or(u64::MAX),
                age: now.duration_since(modified).unwrap_or_default(),
            });
        }
        reports.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(reports)
    }

    /// Returns the names with a `report-NAME.md`, and the names with any
    /// `report-NAME.*` file, archives included.
    pub(crate) fn list_reported_names(&self) -> Result<ReportedNames, HubError> {
        let mut names = ReportedNames::default();
        for file_name in self.list_file_names()? {
            let Some(rest) = file_name.strip_prefix("report-") else {
                continue;
            };
            // Names hold no dot, so the name ends at the first one.
            let Some((name, suffix)) = rest.split_once('.') else {
                continue;
            };
            if suffix == "md" {
                names.unread.insert(name.to_owned());
            }
            names.any.insert(name.to_owned());
        }
        Ok(names)
    }

    /// Lists the Python hub's message files, `send-NAME.json`,
    /// `.sending` and `.failed`, which this hub never sends.
    pub(crate) fn find_old_send_files(&self) -> Result<Vec<PathBuf>, HubError> {
        let mut old = Vec::new();
        for file_name in self.list_file_names()? {
            let is_old = file_name.starts_with("send-")
                && [".json", ".sending", ".failed"]
                    .iter()
                    .any(|suffix| file_name.ends_with(suffix));
            if is_old {
                old.push(self.path.join(file_name));
            }
        }
        old.sort();
        Ok(old)
    }

    /// Renames a report to its `HHMM` archive name, or `HHMM-2` and
    /// so on within the same minute. Returns `None` when NAME has no
    /// unread report.
    pub(crate) fn archive_report(
        &self,
        name: &AgentName,
        now: &Zoned,
    ) -> Result<Option<PathBuf>, HubError> {
        let name = name.as_str();
        let report = self.path.join(format!("report-{name}.md"));
        let stamp = now.strftime("%H%M").to_string();
        let mut archive = self.path.join(format!("report-{name}.{stamp}.read"));
        let mut count = 2;
        while archive.exists() {
            archive = self
                .path
                .join(format!("report-{name}.{stamp}-{count}.read"));
            count += 1;
        }
        match fs::rename(&report, &archive) {
            Ok(()) => Ok(Some(archive)),
            Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(source) => Err(HubError::File {
                path: report,
                source,
            }),
        }
    }

    /// Writes `report-NAME.md` whole.
    pub(crate) fn write_report(&self, name: &AgentName, text: &str) -> Result<(), HubError> {
        let report = self.path.join(format!("report-{}.md", name.as_str()));
        write_file_whole(&report, text)
    }

    /// Appends `- HHMM TEXT` to `notes.md`, after a `## YYYY-MM-DD`
    /// heading when the last heading is another day. Returns `HHMM`.
    pub(crate) fn append_note_line(&self, text: &str, now: &Zoned) -> Result<String, HubError> {
        let existing = self.read_optional_file("notes.md")?;
        let today = now.strftime("%Y-%m-%d").to_string();
        let stamp = now.strftime("%H%M").to_string();
        let last_heading = existing
            .lines()
            .rev()
            .find_map(|line| line.strip_prefix("## "));
        let mut addition = String::new();
        if !existing.is_empty() && !existing.ends_with('\n') {
            addition.push('\n');
        }
        if last_heading.map(str::trim) != Some(today.as_str()) {
            addition.push_str(&format!("## {today}\n"));
        }
        addition.push_str(&format!("- {stamp} {text}\n"));
        let notes = self.path.join("notes.md");
        let appended = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&notes)
            .and_then(|mut file| file.write_all(addition.as_bytes()));
        appended.map_err(|source| HubError::File {
            path: notes,
            source,
        })?;
        Ok(stamp)
    }

    /// Returns the model of the last `--model '...'` line in
    /// `rules.md`, or `None`.
    pub(crate) fn read_rules_model(&self) -> Result<Option<String>, HubError> {
        let rules = self.read_optional_file("rules.md")?;
        let last = RULES_MODEL.captures_iter(&rules).last();
        Ok(last.map(|found| found[1].to_owned()))
    }

    /// Returns the last minute mark, or `None` before the first pass or
    /// when the file does not parse.
    pub(crate) fn read_minute_mark(&self) -> Result<Option<MinuteMark>, HubError> {
        let text = self.read_file_if_exists(MINUTE_MARK_FILE)?;
        Ok(text.and_then(|text| serde_json::from_str(&text).ok()))
    }

    /// Writes the minute mark whole.
    pub(crate) fn write_minute_mark(&self, mark: &MinuteMark) -> Result<(), HubError> {
        let path = self.path.join(MINUTE_MARK_FILE);
        let text = serde_json::to_string(mark).map_err(|error| HubError::File {
            path: path.clone(),
            source: error.into(),
        })?;
        write_file_whole(&path, &text)
    }

    /// Writes this process's ID to `.watcher-pid`, which tells an
    /// older `hub next` to stop.
    pub(crate) fn claim_watcher(&self, process_id: u32) -> Result<(), HubError> {
        write_file_whole(&self.path.join(".watcher-pid"), &format!("{process_id}\n"))
    }

    /// Waits for the lock on `.watcher.lock` and holds it.
    pub(crate) fn take_watcher_lock(&self) -> Result<WatcherLock, HubError> {
        let path = self.path.join(".watcher.lock");
        let locked = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(&path)
            .and_then(|file| file.lock().map(|()| file));
        match locked {
            Ok(file) => Ok(WatcherLock { _file: file }),
            Err(source) => Err(HubError::File { path, source }),
        }
    }

    /// Tells whether a `hub next` holds the watcher lock now, by trying
    /// it.
    pub(crate) fn is_watcher_running(&self) -> Result<bool, HubError> {
        let path = self.path.join(".watcher.lock");
        let file = match File::open(&path) {
            Ok(file) => file,
            Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(source) => return Err(HubError::File { path, source }),
        };
        match file.try_lock() {
            Ok(()) => Ok(false),
            Err(TryLockError::WouldBlock) => Ok(true),
            Err(TryLockError::Error(source)) => Err(HubError::File { path, source }),
        }
    }

    /// Tells whether a newer `hub next` claimed the watcher.
    pub(crate) fn is_replaced(&self, process_id: u32) -> Result<bool, HubError> {
        let claimed = self.read_optional_file(".watcher-pid")?;
        let claimed = claimed.trim();
        Ok(!claimed.is_empty() && claimed != process_id.to_string())
    }

    /// Returns a file of the run folder, or empty text when it does not
    /// exist.
    pub(crate) fn read_optional_file(&self, file_name: &str) -> Result<String, HubError> {
        Ok(self.read_file_if_exists(file_name)?.unwrap_or_default())
    }

    /// Returns a file of the run folder, or `None` when it does not
    /// exist. Use it when a missing file changes what the caller does,
    /// such as an auto run with no `approved.md`.
    pub(crate) fn read_file_if_exists(&self, file_name: &str) -> Result<Option<String>, HubError> {
        let path = self.path.join(file_name);
        match fs::read_to_string(&path) {
            Ok(text) => Ok(Some(text)),
            Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(source) => Err(HubError::File { path, source }),
        }
    }

    /// Returns the name of every file in the folder.
    fn list_file_names(&self) -> Result<Vec<String>, HubError> {
        let entries = fs::read_dir(&self.path).map_err(|source| self.make_file_error(source))?;
        entries
            .map(|entry| {
                entry
                    .map(|entry| entry.file_name().to_string_lossy().into_owned())
                    .map_err(|source| self.make_file_error(source))
            })
            .collect()
    }

    /// Returns the failure to read the folder itself.
    fn make_file_error(&self, source: io::Error) -> HubError {
        HubError::File {
            path: self.path.clone(),
            source,
        }
    }
}

/// Replaces a file through a temporary file and a rename, so a reader
/// never sees half of it.
pub(crate) fn write_file_whole(path: &Path, text: &str) -> Result<(), HubError> {
    let file_name = path.file_name().unwrap_or_default().to_string_lossy();
    // The dot keeps the temporary file out of `list_reports`.
    let temporary = path.with_file_name(format!(".{file_name}.{}.tmp", std::process::id()));
    let written = fs::write(&temporary, text).and_then(|()| fs::rename(&temporary, path));
    if let Err(source) = written {
        let _ = fs::remove_file(&temporary);
        return Err(HubError::File {
            path: path.to_owned(),
            source,
        });
    }
    Ok(())
}
