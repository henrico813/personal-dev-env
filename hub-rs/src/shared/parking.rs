//! Reads the parked records, one JSON file per parked session. A
//! parked session's tab is closed, and its record is what lets it
//! reopen. Nothing parks a session yet, so this file only reads.

use std::fs;
use std::io;
use std::path::PathBuf;

use jiff::Timestamp;
use serde::Deserialize;

use super::HubError;
use crate::outside::cards::CardKey;

/// What `hub status` reads from a parked record. The file holds more,
/// such as the session ID and folder that reopening needs.
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct ParkedRecord {
    pub name: String,
    pub card: CardKey,
    pub parked_at: Timestamp,
}

/// The folder of parked records, `~/.local/state/pde/hub/parked/`,
/// shared by every hub on the machine.
pub struct ParkedFolder {
    folder: PathBuf,
}

impl ParkedFolder {
    pub fn new(folder: PathBuf) -> ParkedFolder {
        ParkedFolder { folder }
    }

    /// Returns every record, sorted by name, skipping files that do not
    /// parse. A missing folder is no records.
    pub(crate) fn list_parked_records(&self) -> Result<Vec<ParkedRecord>, HubError> {
        let file_error = |source: io::Error| HubError::File {
            path: self.folder.clone(),
            source,
        };
        let entries = match fs::read_dir(&self.folder) {
            Ok(entries) => entries,
            Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(source) => return Err(file_error(source)),
        };
        let mut records = Vec::new();
        for entry in entries {
            let path = entry.map_err(file_error)?.path();
            if path.extension().is_none_or(|extension| extension != "json") {
                continue;
            }
            let record = fs::read(&path)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<ParkedRecord>(&bytes).ok());
            records.extend(record);
        }
        records.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(records)
    }
}
