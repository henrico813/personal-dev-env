//! Lets an agent drive the board the user watches. `hub board --listen`
//! takes steps from `board.sock` in the run folder, and
//! `hub board --send` sends one and prints the screen it leads to. Read
//! `StepListener::start` and `send_step` first.
//!
//! The socket is a Unix socket, so both flags work on Linux and macOS.
//! On Windows they stop with an error.

use std::sync::mpsc::Sender;

use crate::commands::Outcome;
use crate::shared::HubError;
use crate::shared::run_folder::RunFolder;

#[cfg(unix)]
use std::fs;
#[cfg(unix)]
use std::io::{BufRead, BufReader, ErrorKind, Write};
#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};
#[cfg(unix)]
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::sync::mpsc::{self, Receiver};
#[cfg(unix)]
use std::thread;
#[cfg(unix)]
use std::time::Duration;

/// The socket's file name in the run folder.
#[cfg(unix)]
const SOCKET: &str = "board.sock";

/// How long the listener waits for an agent's step line. An agent that
/// connects and sends nothing would hold up every agent after it.
#[cfg(unix)]
const STEP_WAIT: Duration = Duration::from_secs(5);

/// One step an agent sent, such as `press esc`, and where the board
/// sends its reply line.
pub(super) struct SentStep {
    pub text: String,
    pub reply: Sender<String>,
}

/// Owns `board.sock` while the board listens. Dropping it removes the
/// file.
pub(super) struct StepListener {
    #[cfg(unix)]
    path: PathBuf,
}

#[cfg(unix)]
impl StepListener {
    /// Makes `board.sock` in `run`, and returns the listener with the
    /// steps agents send. A thread takes one agent at a time and waits
    /// for the board's reply before it takes the next.
    ///
    /// # Errors
    ///
    /// `HubError::Usage` when a board already listens in `run`, and
    /// `HubError::File` when the socket cannot be made.
    pub(super) fn start(run: &RunFolder) -> Result<(StepListener, Receiver<SentStep>), HubError> {
        let path = run.path().join(SOCKET);
        if UnixStream::connect(&path).is_ok() {
            return Err(HubError::Usage(format!(
                "a board already listens in {}",
                run.path().display()
            )));
        }
        // A board that was killed leaves its socket file behind, and
        // binding fails while the file is there.
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(source) => return Err(socket_error(&path, source)),
        }
        let listener = UnixListener::bind(&path).map_err(|source| socket_error(&path, source))?;
        let (sender, steps) = mpsc::channel();
        // Like the reader thread, it is never joined. It waits for the
        // next agent until the process ends.
        thread::spawn(move || answer_agents(&listener, &sender));
        Ok((StepListener { path }, steps))
    }
}

#[cfg(not(unix))]
impl StepListener {
    /// Stops with an error, since Windows has no Unix sockets.
    ///
    /// # Errors
    ///
    /// Always `HubError::Usage`.
    pub(super) fn start(
        _run: &RunFolder,
    ) -> Result<(StepListener, std::sync::mpsc::Receiver<SentStep>), HubError> {
        Err(needs_unix())
    }
}

#[cfg(unix)]
impl Drop for StepListener {
    fn drop(&mut self) {
        // A file left behind is removed by the next board that listens.
        let _ = fs::remove_file(&self.path);
    }
}

/// Sends `step` to the board listening in `run`, and returns the screen
/// it leads to as one JSON line.
///
/// # Errors
///
/// `HubError::NotDone` when no board listens in `run` or the board
/// quits before it takes the step. `HubError::Usage` when the board
/// cannot follow the step, with its reason. `HubError::File` when the
/// socket fails after it connects.
#[cfg(unix)]
pub fn send_step(run: &RunFolder, step: &str) -> Result<Outcome, HubError> {
    let path = run.path().join(SOCKET);
    let stream = UnixStream::connect(&path).map_err(|_| {
        HubError::NotDone(format!(
            "no board listens in {}; start hub board --listen",
            run.path().display()
        ))
    })?;
    writeln!(&stream, "{}", step.trim()).map_err(|source| socket_error(&path, source))?;
    let mut reply = String::new();
    BufReader::new(&stream)
        .read_line(&mut reply)
        .map_err(|source| socket_error(&path, source))?;
    if reply.is_empty() {
        return Err(HubError::NotDone(
            "the board quit before it took the step".to_owned(),
        ));
    }
    let screen: serde_json::Value = serde_json::from_str(&reply)
        .map_err(|error| socket_error(&path, std::io::Error::from(error)))?;
    if let Some(reason) = screen.get("error").and_then(serde_json::Value::as_str) {
        return Err(HubError::Usage(reason.to_owned()));
    }
    Ok(Outcome {
        lines: vec![reply.trim_end().to_owned()],
    })
}

/// Stops with an error, since Windows has no Unix sockets.
///
/// # Errors
///
/// Always `HubError::Usage`.
#[cfg(not(unix))]
pub fn send_step(_run: &RunFolder, _step: &str) -> Result<Outcome, HubError> {
    Err(needs_unix())
}

/// Reads one step from each agent that connects, hands it to the board
/// and writes the board's reply back. It stops once the board quits.
#[cfg(unix)]
fn answer_agents(listener: &UnixListener, steps: &Sender<SentStep>) {
    for stream in listener.incoming().flatten() {
        let mut text = String::new();
        let read = stream
            .set_read_timeout(Some(STEP_WAIT))
            .and_then(|()| BufReader::new(&stream).read_line(&mut text));
        // A board that checks whether another one listens connects and
        // sends nothing.
        if read.is_err() || text.trim().is_empty() {
            continue;
        }
        let (reply, replies) = mpsc::channel();
        let sent = SentStep {
            text: text.trim().to_owned(),
            reply,
        };
        if steps.send(sent).is_err() {
            return;
        }
        // No reply comes when the board quits before it takes the step,
        // and an agent that hung up misses its reply.
        if let Ok(line) = replies.recv() {
            let _ = writeln!(&stream, "{line}");
        }
    }
}

/// Wraps a failure to make or use `board.sock` for `main` to print.
#[cfg(unix)]
fn socket_error(path: &Path, source: std::io::Error) -> HubError {
    HubError::File {
        path: path.to_owned(),
        source,
    }
}

/// The error `--listen` and `--send` stop with on Windows.
#[cfg(not(unix))]
fn needs_unix() -> HubError {
    HubError::Usage("--listen and --send need Linux or macOS".to_owned())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    use crate::board::control::answer_sent_step;
    use crate::board::view::Step;
    use crate::board::view::tests::sample_board;

    /// A run folder of its own under the system's temporary folder,
    /// removed on drop.
    struct TempRun {
        run: RunFolder,
    }

    impl TempRun {
        /// Makes an empty run folder named after `test`.
        fn new(test: &str) -> TempRun {
            let path = std::env::temp_dir().join(format!("hub-{test}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            TempRun {
                run: RunFolder::open(&path).unwrap(),
            }
        }
    }

    impl Drop for TempRun {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(self.run.path());
        }
    }

    /// An agent drives the board the user watches with `--send`. A step
    /// must reach the board through the socket and come back as the
    /// screen it led to, or the agent reports a screen the user never
    /// saw.
    #[test]
    fn send_step_reaches_board_and_returns_screen() {
        let temp = TempRun::new("send-step");
        let (_listener, steps) = StepListener::start(&temp.run).unwrap();
        let mut board = sample_board();

        // The agent sends a tap from another thread, as `--send` does
        // from another terminal.
        let run = RunFolder::open(temp.run.path()).unwrap();
        let agent = thread::spawn(move || send_step(&run, "tap card:DEMO-KB-001"));

        // The board takes the step as the live loop does.
        let sent = steps.recv_timeout(Duration::from_secs(5)).unwrap();
        let (going, reply) =
            answer_sent_step(&mut board, &sent.text, |_, step| Ok(step != Step::Quit)).unwrap();
        sent.reply.send(reply).unwrap();

        // The agent prints the details page the tap opened.
        let outcome = agent.join().unwrap().unwrap();
        let screen: serde_json::Value = serde_json::from_str(&outcome.lines[0]).unwrap();
        assert!(going);
        assert_eq!(screen["step"], "tap card:DEMO-KB-001");
        assert_eq!(screen["page"], "details");
    }

    /// An agent that sends a step with no board listening must learn
    /// that at once, with the way to start one. Waiting, or a bare
    /// socket error, leaves it guessing why nothing happened.
    #[test]
    fn send_step_without_board_names_the_fix() {
        let temp = TempRun::new("send-no-board");

        let error = send_step(&temp.run, "press j").unwrap_err();

        assert_eq!(error.exit_code(), 3);
        assert!(error.to_string().ends_with("start hub board --listen"));
    }
}
