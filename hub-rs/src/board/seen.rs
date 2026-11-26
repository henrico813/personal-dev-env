//! Keeps the time of the user's latest key or tap on the board, so AWAY
//! shows what happened since. The time lives in the state folder, not
//! in a run folder, since AWAY reads the run folders of every hub.
//! Read `Visits` first.

use std::fs;
use std::path::{Path, PathBuf};

use jiff::{SignedDuration, Timestamp};

use crate::shared::HubError;
use crate::shared::run_folder::write_file_whole;

/// The file in the state folder, beside `board.sock`.
const SEEN_FILE: &str = "hub/board-seen";

/// How long the user makes no input before the next one starts a new
/// visit.
const AWAY_GAP: SignedDuration = SignedDuration::from_mins(10);

/// The shortest time between two writes of the file while keys and taps
/// come.
const SAVE_EVERY: SignedDuration = SignedDuration::from_mins(1);

/// The user's visits to one board. A visit starts when the board
/// starts, or at the first key or tap after `AWAY_GAP` with none. AWAY
/// shows what came after the visit's `since`, so it stays put while the
/// user reads, and fills up again while they are away from an open
/// board.
pub(super) struct Visits {
    path: PathBuf,
    /// The saved time as it stood when this visit started, or `None`
    /// when no board ever saved one.
    since: Option<Timestamp>,
    /// When this visit started.
    started: Timestamp,
    last_input: Option<Timestamp>,
    /// The time last written, and when it was written.
    saved: Option<(Timestamp, Timestamp)>,
}

impl Visits {
    /// Starts the first visit at `now`, from the time saved in
    /// `state_folder`.
    pub(super) fn start(state_folder: &Path, now: Timestamp) -> Visits {
        let path = state_folder.join(SEEN_FILE);
        Visits {
            since: read_saved_time(&path),
            path,
            started: now,
            last_input: None,
            saved: None,
        }
    }

    /// Notes a key or tap at `now`, and returns true when it starts a
    /// new visit.
    pub(super) fn note_input(&mut self, now: Timestamp) -> bool {
        let previous = self.last_input.replace(now);
        let quiet_since = previous.unwrap_or(self.started);
        if now.duration_since(quiet_since) < AWAY_GAP {
            return false;
        }
        // A board on another screen may have saved a later time, and
        // this board's last input may not be written yet.
        self.since = read_saved_time(&self.path).max(previous);
        self.started = now;
        true
    }

    /// Writes the latest input's time when the last write was
    /// `SAVE_EVERY` or more before `now`, or there was none.
    ///
    /// # Errors
    ///
    /// `HubError::File` when the file cannot be written.
    pub(super) fn save_if_due(&mut self, now: Timestamp) -> Result<(), HubError> {
        let due = self
            .saved
            .is_none_or(|(_, written)| now.duration_since(written) >= SAVE_EVERY);
        if due {
            self.save_last_input(now)
        } else {
            Ok(())
        }
    }

    /// Writes the latest input's time, unless it is already saved. The
    /// board calls this as it quits.
    ///
    /// # Errors
    ///
    /// `HubError::File` when the file cannot be written.
    pub(super) fn save_last_input(&mut self, now: Timestamp) -> Result<(), HubError> {
        let Some(last) = self.last_input else {
            return Ok(());
        };
        if self.saved.is_some_and(|(time, _)| time >= last) {
            return Ok(());
        }
        if let Some(folder) = self.path.parent() {
            fs::create_dir_all(folder).map_err(|source| HubError::File {
                path: folder.to_owned(),
                source,
            })?;
        }
        write_file_whole(&self.path, &format!("{last}\n"))?;
        self.saved = Some((last, now));
        Ok(())
    }

    /// Returns the time AWAY shows events after, or `None` with no
    /// saved time.
    pub(super) fn since(&self) -> Option<Timestamp> {
        self.since
    }

    /// Returns how long the user was away before this visit, or `None`
    /// with no saved time.
    pub(super) fn measure_time_away(&self) -> Option<SignedDuration> {
        self.since.map(|since| self.started.duration_since(since))
    }
}

/// Returns true when a board has saved a time in `state_folder`.
pub(super) fn has_saved_time(state_folder: &Path) -> bool {
    read_saved_time(&state_folder.join(SEEN_FILE)).is_some()
}

/// Returns the time in the file at `path`, or `None` when it is
/// missing or does not parse.
fn read_saved_time(path: &Path) -> Option<Timestamp> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    /// A state folder of its own under the system's temporary folder,
    /// removed on drop.
    pub(crate) struct TempState {
        pub(crate) path: PathBuf,
    }

    impl TempState {
        /// Makes an empty state folder named after `test`.
        pub(crate) fn new(test: &str) -> TempState {
            let path = std::env::temp_dir().join(format!("hub-{test}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            TempState { path }
        }
    }

    impl Drop for TempState {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    /// A time on 2026-11-15, such as `09:00`, in UTC.
    fn make_time(time: &str) -> Timestamp {
        format!("2026-11-15T{time}:00Z").parse().unwrap()
    }

    /// The time saved as a board quits is where the next board's AWAY
    /// starts, even a board started after a hub handoff. If it were lost,
    /// AWAY would show only the newest events, not what the user missed.
    #[test]
    fn visits_saved_time_starts_next_board() {
        let state = TempState::new("seen-next-board");
        let mut first = Visits::start(&state.path, make_time("09:00"));
        first.note_input(make_time("09:05"));

        first.save_last_input(make_time("09:06")).unwrap();
        let next = Visits::start(&state.path, make_time("10:35"));

        assert_eq!(next.since(), Some(make_time("09:05")));
        assert_eq!(
            next.measure_time_away(),
            Some(SignedDuration::from_mins(90))
        );
    }

    /// While the user reads, their taps must not move the visit's start,
    /// or the events they are reading would leave AWAY under their
    /// finger. After 10 quiet minutes, the next tap starts a new visit
    /// from the later of the last tap before the gap and the time a
    /// board on another screen saved. AWAY then shows only what the user
    /// has not seen on either screen.
    #[test]
    fn visits_note_input_restarts_after_quiet_gap() {
        let state = TempState::new("seen-quiet-gap");
        fs::create_dir_all(state.path.join("hub")).unwrap();
        fs::write(state.path.join(SEEN_FILE), "2026-11-15T08:00:00Z\n").unwrap();
        let mut visits = Visits::start(&state.path, make_time("09:00"));

        let reading = [make_time("09:01"), make_time("09:09"), make_time("09:18")]
            .map(|now| visits.note_input(now));
        let since_while_reading = visits.since();
        let back = visits.note_input(make_time("09:40"));
        let since_back = visits.since();
        // Another board saves a later time during the next gap.
        fs::write(state.path.join(SEEN_FILE), "2026-11-15T10:05:00Z\n").unwrap();
        visits.note_input(make_time("10:20"));

        assert_eq!(reading, [false, false, false]);
        assert_eq!(since_while_reading, Some(make_time("08:00")));
        assert!(back);
        assert_eq!(since_back, Some(make_time("09:18")));
        assert_eq!(visits.since(), Some(make_time("10:05")));
        assert_eq!(
            visits.measure_time_away(),
            Some(SignedDuration::from_mins(15))
        );
    }
}
