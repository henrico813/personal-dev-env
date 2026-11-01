//! This module picks a language's comment rules from a file's path, and runs
//! gofmt, rustfmt and python3 for those rules with a time limit. Only
//! `shared/comments.rs` calls it. Read the `LanguageRules` trait and the
//! `find_rules_for(file)`, `format_if_old_is_formatted(command, folder, old, new)`,
//! and `run_with_input(command, folder, input)` functions to understand the
//! module.

mod docstrings;
mod go;
mod line_comments;
mod python;
mod rust;

use std::io::{self, Read, Write};
use std::iter::successors;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use tree_sitter::{Language, Node};

use crate::shared::comments::{CommentStyle, Definition};
use crate::shared::{Error, ErrorCode};

/// Stops a formatter or check that runs longer, because step 7 holds the Review
/// Session's lock while one runs.
const TOOL_TIME_LIMIT: Duration = Duration::from_secs(10);

/// Limits the wait for a killed tool's output to close, which only a process
/// that left the tool's group can keep open.
const KILLED_TOOL_GRACE: Duration = Duration::from_secs(1);

/// Tells which syntax tree nodes are comments in one language.
pub trait LanguageRules {
    /// Returns the tree-sitter grammar.
    fn grammar(&self) -> Language;
    /// Returns a comment node's style, or `None` for other nodes and
    /// tool lines.
    fn classify_node(&self, node: Node, source: &str) -> Option<CommentStyle>;
    /// Returns the name of the code a comment describes, or `None` when
    /// it describes no one definition. `code_below` is the first node on
    /// the next line, or `None` when that line is blank.
    fn name_comment(&self, comment: Node, code_below: Option<Node>, source: &str)
    -> Option<String>;
    /// Returns the name a comment on the line above `code` would get, or
    /// `None` when that comment would describe no one definition.
    fn name_code_below(&self, code: Node, source: &str) -> Option<String>;
    /// Returns the name a docstring inside `definition` would get, in a
    /// language that has docstrings.
    fn name_docstring_owner(&self, _definition: Node, _source: &str) -> Option<String> {
        None
    }
    /// Returns the comment's words without its markers or quotes. Text that
    /// is not all marked lines, or one whole docstring, comes back as it is.
    fn to_plain_words(&self, text: &str) -> String;
    /// Returns the comment's paragraphs of prose, each on one line, leaving
    /// out examples, or none when the comment has no line markers or quotes.
    fn to_paragraphs(&self, text: &str) -> Vec<String>;
    /// Returns every line of a comment, examples included, joined into one
    /// line, for a language whose length and colon rules read them all.
    fn join_unfiltered_lines(&self, _text: &str) -> Option<String> {
        None
    }
    /// Returns the functions, methods and classes whose names and docstrings
    /// the style rules check, in file order.
    fn list_definitions(&self, _file: &Path, _root: Node, _source: &str) -> Vec<Definition> {
        Vec::new()
    }
    /// Returns true for a file with the marker its generator writes.
    fn is_generated(&self, _source: &str) -> bool {
        false
    }
    /// Returns `words` laid out like `old_text`, with its markers and
    /// indentation, or an empty text when `words` has none.
    fn lay_out_plain_words(&self, words: &str, old_text: &str) -> String;
    /// Returns the new source as the language's tools leave it, or refuses
    /// it. `folder` holds the file, so a formatter finds its settings.
    fn finish_rewrite(&self, folder: &Path, old: &str, new: &str) -> Result<String, Error>;
    /// Returns true for comment text a build or test tool reads, in any case
    /// or spacing, so a cut or reword would change what the tool does.
    fn is_read_by_tools(&self, file: &Path, text: &str) -> bool;
    /// Returns true for a comment node that a cut could turn into different
    /// code, such as one inside a macro call.
    fn is_keep_only(&self, comment: Node, source: &str) -> bool;
}

/// Returns the rules for the file's language, if supported.
pub fn find_rules_for(file: &Path) -> Option<&'static dyn LanguageRules> {
    match file.extension()?.to_str()? {
        "go" => Some(&go::GoRules),
        "py" => Some(&python::PythonRules),
        "rs" => Some(&rust::RustRules),
        _ => None,
    }
}

/// Formats `new` only when the formatter leaves `old` as it is, so a review
/// never reformats code the user wrote.
fn format_if_old_is_formatted(
    command: &[&str],
    folder: &Path,
    old: &str,
    new: &str,
) -> Result<String, Error> {
    match run_with_input(command, folder, old) {
        Ok(formatted) if formatted == old => {}
        // A missing formatter, or one that would change the old file, leaves the edits unformatted.
        _ => return Ok(new.to_owned()),
    }
    run_with_input(command, folder, new).map_err(|reason| Error {
        code: ErrorCode::WouldNotWrite,
        message: format!("{} cannot format the new text. {reason}", command[0]),
    })
}

/// Returns what `command` prints for `input`, or why it failed.
fn run_with_input(command: &[&str], folder: &Path, input: &str) -> Result<String, String> {
    run_for_at_most(command, folder, input, TOOL_TIME_LIMIT)
}

/// Returns true when `node` sits inside a node of one of `kinds`.
fn is_inside(node: Node, kinds: &[&str]) -> bool {
    successors(node.parent(), Node::parent).any(|outer| kinds.contains(&outer.kind()))
}

/// Returns `node` and each parent that starts where it does, innermost
/// first.
fn nodes_starting_at(node: Node) -> impl Iterator<Item = Node> {
    let start = node.start_byte();
    successors(Some(node), move |child| {
        child.parent().filter(|parent| parent.start_byte() == start)
    })
}

/// Runs `command` as `run_with_input` does, and stops it and everything it
/// started once `time_limit` passes, even while they hold its output open.
fn run_for_at_most(
    command: &[&str],
    folder: &Path,
    input: &str,
    time_limit: Duration,
) -> Result<String, String> {
    let deadline = Instant::now() + time_limit;
    let cannot_run = |error: io::Error| format!("Cannot run {}. {error}", command[0]);
    let mut child = Command::new(command[0])
        .args(&command[1..])
        .current_dir(folder)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // Its own process group lets the time limit stop anything the tool starts.
        .process_group(0)
        .spawn()
        .map_err(cannot_run)?;
    let mut stdin = child.stdin.take().expect("stdin is piped");
    let mut stdout = child.stdout.take().expect("stdout is piped");
    let mut stderr = child.stderr.take().expect("stderr is piped");
    let input = input.to_owned();
    // Other threads feed and read the tool, and each says on `ended` when it is done.
    let (ended_sender, ended) = mpsc::channel();
    let writer = run_pipe_work(&ended_sender, move || stdin.write_all(input.as_bytes()));
    let printed = run_pipe_work(&ended_sender, move || read_all(&mut stdout));
    let errors = run_pipe_work(&ended_sender, move || read_all(&mut stderr));
    let mut pipes_open = 3;
    while pipes_open > 0 && ended.recv_timeout(time_left(deadline)).is_ok() {
        pipes_open -= 1;
    }
    // Leaving the tool unreaped until its pipes close keeps its group ID for the kill.
    let status = if pipes_open == 0 {
        wait_until(&mut child, deadline)
    } else {
        None
    };
    let Some(status) = status else {
        let stopped = stop_process_group(&mut child);
        while pipes_open > 0 && ended.recv_timeout(KILLED_TOOL_GRACE).is_ok() {
            pipes_open -= 1;
        }
        if pipes_open == 0 {
            let _ = (writer.join(), printed.join(), errors.join());
        }
        let not_stopped = match stopped {
            Ok(()) => String::new(),
            Err(error) => format!(" It could not be stopped. {error}"),
        };
        return Err(format!(
            "{} ran longer than {time_limit:?}.{not_stopped}",
            command[0]
        ));
    };
    let written = writer.join().expect("writing to a pipe does not panic");
    let printed = printed.join().expect("reading a pipe does not panic");
    let errors = errors.join().expect("reading a pipe does not panic");
    if !status.map_err(cannot_run)?.success() {
        // The last line says why, as in a Python traceback.
        let errors = errors.unwrap_or_default();
        let errors = String::from_utf8_lossy(&errors);
        return Err(errors.trim().lines().last().unwrap_or_default().to_owned());
    }
    written.map_err(|error| format!("Cannot send {} the file. {error}", command[0]))?;
    String::from_utf8(printed.map_err(cannot_run)?).map_err(|error| error.to_string())
}

/// Runs `work` on its own thread, which sends on `ended` once it is done.
fn run_pipe_work<T: Send + 'static>(
    ended: &Sender<()>,
    work: impl FnOnce() -> T + Send + 'static,
) -> JoinHandle<T> {
    let ended = ended.clone();
    thread::spawn(move || {
        let result = work();
        // A send fails only once the caller stopped waiting.
        let _ = ended.send(());
        result
    })
}

fn time_left(deadline: Instant) -> Duration {
    deadline.saturating_duration_since(Instant::now())
}

fn read_all(pipe: &mut impl Read) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    pipe.read_to_end(&mut bytes).map(|_| bytes)
}

/// Returns the tool's exit status, or `None` while it still runs at the
/// deadline.
fn wait_until(child: &mut Child, deadline: Instant) -> Option<io::Result<ExitStatus>> {
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(Ok(status)),
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            Ok(None) => return None,
            Err(error) => return Some(Err(error)),
        }
    }
}

/// Kills the tool's process group, then collects the tool. The tool must be
/// unreaped, so that the group ID is still its own.
fn stop_process_group(child: &mut Child) -> io::Result<()> {
    let group = libc::pid_t::try_from(child.id()).map_err(io::Error::other)?;
    // SAFETY: killpg touches no memory, and the unreaped tool keeps its group ID from reuse.
    let killed = unsafe { libc::killpg(group, libc::SIGKILL) };
    let kill_error = (killed != 0).then(io::Error::last_os_error);
    if kill_error.is_some() {
        // Killing the tool alone still lets the wait below end.
        let _ = child.kill();
    }
    child.wait()?;
    kill_error.map_or(Ok(()), Err)
}

#[cfg(test)]
mod run_for_at_most {
    use std::fs;
    use std::path::PathBuf;

    use super::*;

    /// `run_for_at_most` must stop the tool, and a child that keeps its output open, once the time limit passes.
    /// Otherwise a hung formatter would keep the Review Session locked, or keep running after the limit.
    #[test]
    fn stops_tool_and_its_children_at_limit() {
        let output = Command::new("mktemp").arg("-d").output().unwrap();
        let folder = PathBuf::from(String::from_utf8(output.stdout).unwrap().trim_end());
        let cases = [
            ("tool sleeps", "echo $$ > pid; exec sleep 5"),
            ("child sleeps", "sleep 5 & echo $! > pid"),
        ];
        for (case, script) in cases {
            let started = Instant::now();

            let result = run_for_at_most(
                &["sh", "-c", script],
                &folder,
                "",
                Duration::from_millis(100),
            );

            let elapsed = started.elapsed();
            let pid = fs::read_to_string(folder.join("pid")).unwrap();
            assert!(result.is_err(), "{case}");
            assert!(elapsed < Duration::from_secs(2), "{case}");
            assert!(is_gone_soon(pid.trim()), "{case}");
        }
        fs::remove_dir_all(&folder).unwrap();
    }

    /// Returns true once the process has left `/proc`, waiting at most two
    /// seconds for whoever collects it.
    fn is_gone_soon(pid: &str) -> bool {
        let started = Instant::now();
        while Path::new("/proc").join(pid).exists() {
            if started.elapsed() > Duration::from_secs(2) {
                return false;
            }
            thread::yield_now();
        }
        true
    }
}
