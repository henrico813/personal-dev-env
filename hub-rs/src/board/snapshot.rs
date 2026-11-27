//! Saves each read of the live board, so the next board draws the last
//! one at once while its own first read runs.

use std::fs;
use std::path::Path;

use super::queue::Queue;
use crate::shared::HubError;
use crate::shared::run_folder::write_file_whole;

/// The file in the state folder, beside `board-seen`.
const SNAPSHOT_FILE: &str = "hub/board-snapshot.json";

/// Saves `queue` in `state_folder`, whole, so a board starting at the
/// same moment never reads half of it.
///
/// # Errors
///
/// `HubError::File` when the folder or the file cannot be written.
pub(super) fn save_snapshot(state_folder: &Path, queue: &Queue) -> Result<(), HubError> {
    let path = state_folder.join(SNAPSHOT_FILE);
    if let Some(folder) = path.parent() {
        fs::create_dir_all(folder).map_err(|source| HubError::File {
            path: folder.to_owned(),
            source,
        })?;
    }
    let text = serde_json::to_string(queue).map_err(|error| HubError::File {
        path: path.clone(),
        source: error.into(),
    })?;
    write_file_whole(&path, &text)
}

/// Returns the read saved in `state_folder`, or `None` when there is
/// none or it no longer parses, as after an update that changed `Queue`.
pub(super) fn load_snapshot(state_folder: &Path) -> Option<Queue> {
    let text = fs::read_to_string(state_folder.join(SNAPSHOT_FILE)).ok()?;
    serde_json::from_str(&text).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::board::seen::tests::TempState;
    use crate::board::view::Board;
    use crate::board::view::tests::make_sample_queue;

    /// A new board's first read can take half a minute. The board must
    /// draw the saved read at once and say when it was saved, or the
    /// user waits on an empty screen, or takes old rows for new ones.
    #[test]
    fn load_snapshot_gives_board_saved_read() {
        let state = TempState::new("snapshot-saved-read");
        save_snapshot(&state.path, &make_sample_queue()).unwrap();

        let mut board = Board::new(false);
        board.show_saved_reading(load_snapshot(&state.path).unwrap());
        let lines = board.render_lines(40, 16);

        let screen = lines.join("\n");
        assert!(lines[0].starts_with("saved 10:42 · reading"), "{screen}");
        assert!(screen.contains("DEMO-KB-001"), "{screen}");
    }
}
