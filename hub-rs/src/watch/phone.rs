//! Sends a phone notification with Moshi's `moshi-hook notify`, which
//! pde-installer installs on full machines.

use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// How long `moshi-hook notify` may run. It makes a network call, and
/// the watch checks no worker while it waits.
const NOTIFY_TIMEOUT: Duration = Duration::from_secs(30);

/// The wait between checks on whether the program ended.
const EXIT_POLL: Duration = Duration::from_millis(100);

/// Runs `program notify --source hub --title TITLE MESSAGE` and stops
/// it after 30 seconds. `main` gives `moshi-hook` as `program`, and
/// tests give a script. The program's error output goes to the watch's
/// log. `moshi-hook notify` exits 0 even when the phone was not
/// reached, so a success means only that the program ran.
///
/// # Errors
///
/// Why the program did not start, ran too long or exited with a
/// failure.
pub(crate) fn send_phone_notification(
    program: &Path,
    title: &str,
    message: &str,
) -> Result<(), String> {
    let mut child = Command::new(program)
        .args(["notify", "--source", "hub", "--title", title, message])
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
