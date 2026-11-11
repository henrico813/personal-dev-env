//! Names the files in the hub's run folder, and reads and writes
//! reports and notes. Read `RunFolder` first. Every whole-file write
//! goes through `write_file_whole`.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use jiff::Zoned;
use regex::Regex;

use super::{AgentName, HubError};

/// The model in `rules.md`, written as `--model '<provider>/<model>#<effort>'`.
static RULES_MODEL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"--model '([^']+)'").unwrap());

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

    /// Returns the names with an unread report, sorted.
    pub(crate) fn list_reports(&self) -> Result<Vec<String>, HubError> {
        let entries = fs::read_dir(&self.path).map_err(|source| self.make_file_error(source))?;
        let mut names = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|source| self.make_file_error(source))?;
            let file_name = entry.file_name().to_string_lossy().into_owned();
            if let Some(name) = file_name
                .strip_prefix("report-")
                .and_then(|rest| rest.strip_suffix(".md"))
            {
                names.push(name.to_owned());
            }
        }
        names.sort();
        Ok(names)
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

    /// Returns a file of the run folder, or empty text when it does not
    /// exist.
    pub(crate) fn read_optional_file(&self, file_name: &str) -> Result<String, HubError> {
        let path = self.path.join(file_name);
        match fs::read_to_string(&path) {
            Ok(text) => Ok(text),
            Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(String::new()),
            Err(source) => Err(HubError::File { path, source }),
        }
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
fn write_file_whole(path: &Path, text: &str) -> Result<(), HubError> {
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
