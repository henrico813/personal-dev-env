//! Starts a program that keeps running after the command that started
//! it ends, for `hub watch start`, `hub report` and `hub wait`. It works
//! the same on Linux and macOS, and under any harness.

use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};

/// Starts `program` with `args` in a process group of its own and
/// returns without waiting. Its output and errors go to the end of the
/// file at `log`. OpenCode and Claude Code stop a shell command by
/// signalling its process group, so a program left in that group would
/// end with the command. Returns the child so a test can wait for it.
/// Dropping it neither stops nor waits for the program.
///
/// # Errors
///
/// When the log file cannot be opened or the program cannot start.
pub fn start_in_background(program: &Path, args: &[OsString], log: &Path) -> io::Result<Child> {
    let output = OpenOptions::new().create(true).append(true).open(log)?;
    let errors = output.try_clone()?;
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(output)
        .stderr(errors)
        // Group 0 makes the program the leader of a new group.
        .process_group(0)
        .spawn()
}
