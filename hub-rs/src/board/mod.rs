//! Runs `hub board`, a full-screen list of what waits on the user and
//! on the hub, sized for a phone first. Read `run_board` and
//! `print_board` first.
//!
//! A second thread reads the hub's state every 10 seconds, or at once
//! when the user presses r. A read calls Herdr, `card` and OpenCode
//! and takes about a second, so the keys and the mouse would freeze if
//! the drawing thread waited for it.

mod queue;
mod terminal;
mod view;

use std::io::{self, IsTerminal};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::thread;
use std::time::Duration;

use crossterm::event;
use jiff::tz::TimeZone;
use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};

use crate::commands::Outcome;
use crate::shared::run_folder::RunFolder;
use crate::shared::{HubError, OwnedTools, Tools};
use queue::{Queue, read_queue};
use terminal::Screen;
use view::{Board, Step};

/// How long the board waits for input before it checks for a stop
/// signal or a new read again.
const POLL: Duration = Duration::from_millis(200);

/// How long the reader waits between reads when nobody presses r.
const REFRESH: Duration = Duration::from_secs(10);

/// The size `--print` draws, given as `40x16`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScreenSize {
    pub columns: u16,
    pub rows: u16,
}

/// Reads `COLSxROWS`, such as `40x16`, with neither number zero.
impl FromStr for ScreenSize {
    type Err = String;

    fn from_str(text: &str) -> Result<ScreenSize, String> {
        let parsed = text
            .split_once('x')
            .and_then(|(columns, rows)| Some((columns.parse().ok()?, rows.parse().ok()?)));
        match parsed {
            Some((columns, rows)) if columns > 0 && rows > 0 => Ok(ScreenSize { columns, rows }),
            _ => Err(format!("{text} is not a size such as 40x16")),
        }
    }
}

/// Shows the board until the user quits or a signal stops it. The
/// reader thread owns `tools` and `run`.
///
/// # Errors
///
/// `HubError::Usage` when standard input or output is not a terminal.
/// `HubError::File` when the terminal cannot be read or drawn on.
/// `HubError::NotDone` when the reader thread stops, which only a
/// panic in it causes.
pub fn run_board(
    tools: OwnedTools,
    run: RunFolder,
    time_zone: TimeZone,
) -> Result<Outcome, HubError> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(HubError::Usage("board needs a terminal".to_owned()));
    }
    // A signal only sets the flag, so the loop ends and dropping the
    // screen puts the terminal back.
    let stop = Arc::new(AtomicBool::new(false));
    for signal in [SIGTERM, SIGHUP, SIGINT] {
        signal_hook::flag::register(signal, Arc::clone(&stop)).map_err(terminal_error)?;
    }
    let (reading_sender, readings) = mpsc::channel();
    let (wake, wake_receiver) = mpsc::channel();
    // The thread is never joined, so q quits at once even while a read
    // runs. The thread ends at its next send or wait after the
    // channels close, or with the process.
    thread::spawn(move || {
        read_until_closed(&tools, &run, &time_zone, &reading_sender, &wake_receiver);
    });

    let mut board = Board::new();
    let mut screen = Screen::start().map_err(terminal_error)?;
    screen.draw(&mut board).map_err(terminal_error)?;
    while !stop.load(Ordering::Relaxed) {
        let mut changed = false;
        loop {
            match readings.try_recv() {
                Ok(reading) => {
                    board.show_reading(reading);
                    changed = true;
                }
                Err(TryRecvError::Empty) => break,
                // The panic hook already put the terminal back, so
                // drawing on would write over the shell.
                Err(TryRecvError::Disconnected) => {
                    return Err(HubError::NotDone("the board's reader stopped".to_owned()));
                }
            }
        }
        if event::poll(POLL).map_err(terminal_error)? {
            let input = event::read().map_err(terminal_error)?;
            match board.handle_input(&input) {
                Step::Quit => break,
                Step::Refresh => {
                    board.note_reading();
                    // A send fails only when the reader has stopped.
                    let _ = wake.send(());
                }
                Step::Stay => {}
            }
            changed = true;
        }
        if changed {
            screen.draw(&mut board).map_err(terminal_error)?;
        }
    }
    drop(screen);
    Ok(Outcome { lines: Vec::new() })
}

/// Reads the hub's state once and returns the board's first screen as
/// text, `size` large, with no terminal needed.
///
/// # Errors
///
/// `HubError::Herdr` when Herdr cannot list the agents, and
/// `HubError::File` when a run folder file cannot be read.
pub fn print_board(
    tools: &Tools<'_>,
    run: &RunFolder,
    time_zone: &TimeZone,
    size: ScreenSize,
) -> Result<Outcome, HubError> {
    let mut board = Board::new();
    board.show_reading(Ok(read_queue(tools, run, time_zone)?));
    Ok(Outcome {
        lines: board.render_lines(size.columns, size.rows),
    })
}

/// Sends a read, then waits for a wake or `REFRESH`, until the board
/// stops listening.
fn read_until_closed(
    tools: &OwnedTools,
    run: &RunFolder,
    time_zone: &TimeZone,
    readings: &Sender<Result<Queue, String>>,
    wake: &Receiver<()>,
) {
    loop {
        let reading = read_queue(&tools.lend(), run, time_zone).map_err(|error| error.to_string());
        if readings.send(reading).is_err() {
            return;
        }
        // Presses of r during the read ask for the read just sent.
        while wake.try_recv().is_ok() {}
        match wake.recv_timeout(REFRESH) {
            Ok(()) | Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

/// Wraps a terminal read or write failure for `main` to print.
fn terminal_error(source: io::Error) -> HubError {
    HubError::File {
        path: PathBuf::from("the terminal"),
        source,
    }
}
