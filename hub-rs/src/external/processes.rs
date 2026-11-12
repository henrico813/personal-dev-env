//! Reads `/proc` to tell whether a shell runs a job, such as an editor
//! stopped with Ctrl-Z. Herdr shows only the process in front.

use std::fs;
use std::io;

/// Tells whether a shell owns a job. A shell runs each job in a process
/// group of its own, while helpers it starts without job control stay
/// in the shell's group.
///
/// # Errors
///
/// When `/proc` has no readable entry for the shell.
pub fn has_shell_jobs(shell_pid: u32) -> io::Result<bool> {
    let shell_group = read_process_group(shell_pid)?;
    for task in fs::read_dir(format!("/proc/{shell_pid}/task"))? {
        let children = fs::read_to_string(task?.path().join("children"))?;
        for child in children.split_whitespace() {
            let Ok(child) = child.parse() else {
                continue;
            };
            // A child that ended after the shell listed it holds no job.
            let Ok(group) = read_process_group(child) else {
                continue;
            };
            if group != shell_group {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Returns a process's group ID from `/proc/PID/stat`.
fn read_process_group(pid: u32) -> io::Result<u32> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat"))?;
    // The name before the last `)` may hold spaces, and the group is the
    // third field after it.
    stat.rsplit_once(')')
        .and_then(|(_, rest)| rest.split_whitespace().nth(2))
        .and_then(|group| group.parse().ok())
        .ok_or_else(|| io::Error::other(format!("/proc/{pid}/stat has no process group")))
}
