//! Tells the user's phone when `pde-installer update` did not install
//! the newest main. A timer runs the update with no one watching, and
//! the update saves each result in `update.json` in the state folder.

use std::fs;
use std::io;
use std::path::Path;

use jiff::Timestamp;
use serde::{Deserialize, Serialize};

use super::phone::PhoneBackoff;
use crate::shared::HubError;
use crate::shared::run_folder;

/// The file `pde-installer update` writes in the state folder.
const UPDATE_FILE: &str = "update.json";

/// The last result the watch passed on, in the watch's folder.
const SEEN_FILE: &str = "update-seen.json";

/// One result of `pde-installer update`. It leaves out `checked_at`, so
/// a timer that meets the same failure each hour sends one notice.
#[derive(Serialize, Deserialize, PartialEq)]
struct UpdateResult {
    result: String,
    commit: String,
    reason: String,
}

/// Sends one phone notification for a result in `update.json` that is
/// new and is neither `installed` nor `current`, then remembers the
/// result in `watch_folder`. A refused or held-back notification is not
/// remembered, so a later check sends it once `backoff` allows. Does
/// nothing when no update ran.
///
/// # Errors
///
/// `HubError::File` when a file cannot be read, parsed or written, and
/// `HubError::NotDone` when the notifier fails.
pub(crate) fn pass_on_update_result(
    state_folder: &Path,
    watch_folder: &Path,
    notifier: &Path,
    backoff: &mut PhoneBackoff,
    now: Timestamp,
) -> Result<(), HubError> {
    let Some(update) = read_update_result(&state_folder.join(UPDATE_FILE))? else {
        return Ok(());
    };
    let seen_path = watch_folder.join(SEEN_FILE);
    if read_update_result(&seen_path)?.as_ref() == Some(&update) {
        return Ok(());
    }
    if !matches!(update.result.as_str(), "installed" | "current") {
        let title = format!("pde-installer update {}", update.result);
        let sent = backoff
            .send(notifier, &title, &update.reason, now)
            .map_err(HubError::NotDone)?;
        if !sent {
            return Ok(());
        }
    }
    let text = serde_json::to_string(&update).map_err(|error| HubError::File {
        path: seen_path.clone(),
        source: error.into(),
    })?;
    run_folder::write_file_whole(&seen_path, &text)
}

/// Returns the result saved at `path`, or `None` when the file does not
/// exist.
fn read_update_result(path: &Path) -> Result<Option<UpdateResult>, HubError> {
    let wrap_file_error = |source| HubError::File {
        path: path.to_owned(),
        source,
    };
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(wrap_file_error(source)),
    };
    serde_json::from_str(&text).map_err(|error| wrap_file_error(error.into()))
}
