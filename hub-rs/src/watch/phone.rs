//! Sends a phone notification with Moshi's `moshi-hook notify`, which
//! pde-installer installs on full machines.

use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use jiff::{SignedDuration, Timestamp};

/// How long `moshi-hook notify` may run. It makes a network call, and
/// the watch checks no worker while it waits.
const NOTIFY_TIMEOUT: Duration = Duration::from_secs(30);

/// The wait between checks on whether the program ended.
const EXIT_POLL: Duration = Duration::from_millis(100);

/// The wait after the first refused notification. Each refusal in a
/// row doubles it, up to `MAX_BACKOFF`.
const FIRST_BACKOFF: SignedDuration = SignedDuration::from_mins(1);
const MAX_BACKOFF: SignedDuration = SignedDuration::from_hours(1);

/// Holds every phone notification back for a while after a refused
/// one. Moshi's rate limit covers the whole account, so asking again
/// at each poll only meets more refusals, and one may then get
/// through as a repeat. `run_watch` keeps one for the watch's life.
#[derive(Default)]
pub struct PhoneBackoff {
    refusals: u32,
    next_try: Option<Timestamp>,
}

impl PhoneBackoff {
    /// Sends one notification as `send_phone_notification` does, unless
    /// a wait from an earlier refusal runs at `now`. Returns `false`
    /// when held back, so the caller leaves it unrecorded and tries
    /// again at a later check.
    ///
    /// # Errors
    ///
    /// The program's failure, which starts or grows the wait.
    pub(crate) fn send(
        &mut self,
        program: &Path,
        title: &str,
        message: &str,
        now: Timestamp,
    ) -> Result<bool, String> {
        if self.next_try.is_some_and(|next_try| now < next_try) {
            return Ok(false);
        }
        if let Err(error) = send_phone_notification(program, title, message) {
            // 2^6 minutes is past the hour, so more doublings change nothing.
            let doublings = self.refusals.min(6);
            let wait = (FIRST_BACKOFF * 2_i32.pow(doublings)).min(MAX_BACKOFF);
            self.refusals += 1;
            self.next_try = Some(now + wait);
            return Err(error);
        }
        *self = Self::default();
        Ok(true)
    }
}

/// Runs `program notify --strict --source hub --title TITLE MESSAGE`
/// and stops it after 30 seconds. `main` gives `moshi-hook` as
/// `program`, and tests give a script. The program's error output goes
/// to the watch's log. Without `--strict`, `moshi-hook notify` exits 0
/// even when Moshi refused the push, such as at its rate limit, so the
/// caller could not tell to try again.
///
/// # Errors
///
/// Why the program did not start, ran too long or exited with a
/// failure.
fn send_phone_notification(program: &Path, title: &str, message: &str) -> Result<(), String> {
    let mut child = Command::new(program)
        .args([
            "notify", "--strict", "--source", "hub", "--title", title, message,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .map_err(|error| format!("cannot run {}: {error}", program.display()))?;
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < NOTIFY_TIMEOUT => thread::sleep(EXIT_POLL),
            Ok(None) => {
                // A program that ended just now cannot be killed, which
                // is no failure.
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "{} ran past {} seconds and was stopped",
                    program.display(),
                    NOTIFY_TIMEOUT.as_secs()
                ));
            }
            Err(error) => {
                return Err(format!("cannot wait for {}: {error}", program.display()));
            }
        }
    };
    if status.success() {
        return Ok(());
    }
    Err(format!("{} ended with {status}", program.display()))
}
