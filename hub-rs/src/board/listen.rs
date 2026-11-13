//! Lets an agent drive the board the user watches. `hub board --listen`
//! takes steps from `hub/board.sock` in the state folder,
//! `~/.local/state/pde`, and `hub board --send` sends one and prints
//! the screen it leads to. One board per user listens at a time. Read
//! `StepListener::start` and `send_step` first.
//!
//! The socket is a Unix socket, so both flags work on Linux and macOS,
//! not Windows.

use crate::commands::Outcome;
use crate::shared::HubError;

use std::fs;
use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

/// The socket's path in the state folder, beside `hub watch`'s lock.
const SOCKET: &str = "hub/board.sock";

/// How long the listener waits for an agent's step line. An agent that
/// connects and sends nothing would hold up every agent after it.
const STEP_WAIT: Duration = Duration::from_secs(5);

/// How long `send_step` waits for the board's reply. A board stuck on a
/// closed terminal would otherwise hold the agent forever.
const REPLY_WAIT: Duration = Duration::from_secs(15);

/// One step an agent sent, such as `press esc`, with the agent's
/// connection for the board's reply.
pub(super) struct SentStep {
    pub text: String,
    agent: UnixStream,
}

impl SentStep {
    /// Writes the board's reply line to the agent, or drops it when the
    /// agent hung up. The board calls this before it quits, so a
    /// `press q` reply is never lost.
    pub(super) fn answer(&self, reply: &str) {
        let _ = writeln!(&self.agent, "{reply}");
    }
}

/// Owns `board.sock` while the board listens. Dropping it removes the
/// file.
pub(super) struct StepListener {
    path: PathBuf,
}

impl StepListener {
    /// Makes `board.sock` in `state_folder`, and returns the listener
    /// with the steps agents send. A thread reads each agent's step in
    /// turn, and the board answers them in that order.
    ///
    /// # Errors
    ///
    /// `HubError::Usage` when another board listens, and
    /// `HubError::File` when the socket cannot be made.
    pub(super) fn start(
        state_folder: &Path,
    ) -> Result<(StepListener, Receiver<SentStep>), HubError> {
        let path = state_folder.join(SOCKET);
        if UnixStream::connect(&path).is_ok() {
            return Err(HubError::Usage(
                "another board listens; quit it, or start this one without --listen".to_owned(),
            ));
        }
        if let Some(folder) = path.parent() {
            fs::create_dir_all(folder).map_err(|source| socket_error(folder, source))?;
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
        thread::spawn(move || pass_steps_to_board(&listener, &sender));
        Ok((StepListener { path }, steps))
    }

    /// Returns the path of `board.sock`.
    pub(super) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for StepListener {
    fn drop(&mut self) {
        // A file left behind is removed by the next board that listens.
        let _ = fs::remove_file(&self.path);
    }
}

/// Sends `step` to the board that listens on `state_folder`'s socket,
/// and returns the screen it leads to as one JSON line.
///
/// # Errors
///
/// `HubError::NotDone` when no board listens, the board
/// quits before it takes the step, or it does not answer within 15
/// seconds. `HubError::Usage` when `step` is
/// blank or the board cannot follow it, with the reason.
/// `HubError::File` when the socket fails after it connects.
pub fn send_step(state_folder: &Path, step: &str) -> Result<Outcome, HubError> {
    // The board drops a blank line's connection, which would read as
    // the board quitting before it took the step.
    if step.trim().is_empty() {
        return Err(HubError::Usage(
            "the step is blank; send one such as press j".to_owned(),
        ));
    }
    let path = state_folder.join(SOCKET);
    let stream = UnixStream::connect(&path)
        .map_err(|_| HubError::NotDone("no board listens; start hub board --listen".to_owned()))?;
    writeln!(&stream, "{}", step.trim()).map_err(|source| socket_error(&path, source))?;
    let mut reply = String::new();
    stream
        .set_read_timeout(Some(REPLY_WAIT))
        .and_then(|()| BufReader::new(&stream).read_line(&mut reply))
        .map_err(|source| match source.kind() {
            ErrorKind::WouldBlock | ErrorKind::TimedOut => HubError::NotDone(format!(
                "the board did not answer within {} seconds",
                REPLY_WAIT.as_secs()
            )),
            _ => socket_error(&path, source),
        })?;
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

/// Reads one step from each agent that connects and hands it to the
/// board with the agent's connection. It stops once the board quits,
/// and the agents still waiting then read no reply.
fn pass_steps_to_board(listener: &UnixListener, steps: &Sender<SentStep>) {
    for agent in listener.incoming().flatten() {
        let mut text = String::new();
        // The board writes the reply on its own loop, so an agent that
        // stops reading must not hold it.
        let read = agent
            .set_read_timeout(Some(STEP_WAIT))
            .and_then(|()| agent.set_write_timeout(Some(STEP_WAIT)))
            .and_then(|()| BufReader::new(&agent).read_line(&mut text));
        // A board that checks whether another one listens connects and
        // sends nothing.
        if read.is_err() || text.trim().is_empty() {
            continue;
        }
        let sent = SentStep {
            text: text.trim().to_owned(),
            agent,
        };
        if steps.send(sent).is_err() {
            return;
        }
    }
}

/// Wraps a failure to make or use `board.sock` for `main` to print.
fn socket_error(path: &Path, source: std::io::Error) -> HubError {
    HubError::File {
        path: path.to_owned(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::board::control::answer_sent_step;
    use crate::board::view::Step;
    use crate::board::view::tests::sample_board;

    /// A state folder of its own under the system's temporary folder,
    /// removed on drop.
    struct TempState {
        path: PathBuf,
    }

    impl TempState {
        /// Makes an empty state folder named after `test`.
        fn new(test: &str) -> TempState {
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

    /// An agent drives the board the user watches with `--send`. A step
    /// must reach the board through the socket and come back as the
    /// screen it led to, or the agent reports a screen the user never
    /// saw.
    #[test]
    fn send_step_reaches_board_and_returns_screen() {
        let temp = TempState::new("send-step");
        let (_listener, steps) = StepListener::start(&temp.path).unwrap();
        let mut board = sample_board();

        // The agent sends a tap from another thread, as `--send` does
        // from another terminal.
        let state_folder = temp.path.clone();
        let agent = thread::spawn(move || send_step(&state_folder, "tap card:DEMO-KB-001"));

        // The board takes the step as the live loop does.
        let sent = steps.recv_timeout(Duration::from_secs(5)).unwrap();
        let (going, reply) =
            answer_sent_step(&mut board, &sent.text, |_, step| Ok(step != Step::Quit)).unwrap();
        sent.answer(&reply);

        // The agent prints the details page the tap opened.
        let outcome = agent.join().unwrap().unwrap();
        let screen: serde_json::Value = serde_json::from_str(&outcome.lines[0]).unwrap();
        assert!(going);
        assert_eq!(screen["step"], "tap card:DEMO-KB-001");
        assert_eq!(screen["page"], "details");
    }

    /// An agent that taps a hidden card must fail with exit code 2 and
    /// the step that shows the card. Printing the screen and exiting 0
    /// would let it report a tap that never happened, and the reason is
    /// the only place it learns which step shows that card.
    #[test]
    fn send_step_hidden_tap_fails_naming_show() {
        let temp = TempState::new("send-hidden-card");
        let (_listener, steps) = StepListener::start(&temp.path).unwrap();
        let mut board = sample_board();
        let state_folder = temp.path.clone();
        let agent = thread::spawn(move || send_step(&state_folder, "tap card:DEMO-KB-020"));

        let sent = steps.recv_timeout(Duration::from_secs(5)).unwrap();
        let (_, reply) = answer_sent_step(&mut board, &sent.text, |_, _| Ok(true)).unwrap();
        sent.answer(&reply);

        let error = agent.join().unwrap().unwrap_err();
        assert_eq!(error.exit_code(), 2);
        assert_eq!(
            error.to_string(),
            "card:DEMO-KB-020 is hidden by fold; send tap group:ready first"
        );
    }

    /// A killed board leaves `board.sock` behind, and a new board must
    /// replace it or the user cannot listen again. A second board must
    /// be refused, or it takes the steps meant for the board the user
    /// watches.
    #[test]
    fn step_listener_start_replaces_only_dead_socket() {
        let temp = TempState::new("start-dead-socket");
        fs::create_dir(temp.path.join("hub")).unwrap();
        drop(UnixListener::bind(temp.path.join(SOCKET)).unwrap());

        let first = StepListener::start(&temp.path);
        let second = StepListener::start(&temp.path);

        assert!(first.is_ok());
        assert!(matches!(second, Err(HubError::Usage(_))));
    }

    /// The board drops the connection of a blank line, so without this
    /// check `--send ''` says the board quit before it took the step.
    /// That misleads the agent, so it must say what to send instead.
    #[test]
    fn send_step_blank_step_fails_with_example() {
        let temp = TempState::new("send-blank");

        let error = send_step(&temp.path, "  ").unwrap_err();

        assert_eq!(error.exit_code(), 2);
        assert_eq!(
            error.to_string(),
            "the step is blank; send one such as press j"
        );
    }

    /// An agent that sends a step with no board listening must learn
    /// that at once, with the way to start one. Waiting, or a bare
    /// socket error, leaves it guessing why nothing happened.
    #[test]
    fn send_step_without_board_names_the_fix() {
        let temp = TempState::new("send-no-board");

        let error = send_step(&temp.path, "press j").unwrap_err();

        assert_eq!(error.exit_code(), 3);
        assert!(error.to_string().ends_with("start hub board --listen"));
    }
}
