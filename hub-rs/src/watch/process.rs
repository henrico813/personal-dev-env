//! Tells whether a process still runs, for `hub wait --pid`.

use std::process::{Command, Stdio};

/// Tells whether the process with this ID still runs. `kill -0` sends
/// no signal and works on Linux and macOS. When `kill` cannot run, or
/// the process belongs to another user, it counts as ended, so a
/// waiting worker is woken too early rather than never.
pub(crate) fn is_process_running(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}
