//! Runs `hub board`, a full-screen view of the user's cards,
//! sessions and machine gauges, sized for a phone first. Read
//! `run_board` and `print_board` first, `control.rs` for
//! the JSON that agents read, and `listen.rs` for the socket an agent
//! drives the live board through.
//!
//! A second thread reads the hubs' state every 10 seconds, or at once
//! when the user presses r. A read calls Herdr, `card` and OpenCode
//! and takes about a second, so the keys and the mouse would freeze if
//! the drawing thread waited for it. For the same reason, a details
//! page reads its card's note on a thread of its own. The reading
//! thread also runs a jump to a session and the clean pass after c
//! and y.

mod control;
mod draw;
mod elements;
mod groups;
mod listen;
mod machine;
mod panels;
mod queue;
mod sessions;
mod terminal;
mod view;

use std::fs;
use std::io::{self, IsTerminal};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

use crossterm::event;
use jiff::Timestamp;
use jiff::tz::TimeZone;
use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};

use crate::commands::{Outcome, clean};
use crate::external::cards::{CardCli, CardKey, CardNote, Cards};
use crate::shared::run_folder::RunFolder;
use crate::shared::{HubError, OwnedTools, Tools};
use listen::StepListener;
use queue::{Queue, read_queue};
use terminal::Screen;
use view::{Board, Step};

pub use control::{control_board, dump_board};
pub use listen::send_step;
pub use queue::Hubs;

/// How long the board waits for input before it checks for a stop
/// signal or a new read again.
const POLL: Duration = Duration::from_millis(200);

/// How long the reader waits between reads when nobody presses r.
const REFRESH: Duration = Duration::from_secs(10);

/// How long the board has to end after a stop signal before
/// `exit_if_stuck` ends the process.
const STOP_WAIT: Duration = Duration::from_secs(1);

/// The size `--print`, `--dump` and `--control` draw, given as `40x16`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScreenSize {
    pub columns: u16,
    pub rows: u16,
}

/// What the board asks its reader thread to do.
#[derive(Debug, PartialEq, Eq)]
enum Job {
    /// Fetch the hubs' state again now.
    Read,
    /// Show this session in Herdr.
    Jump(String),
    /// Run the clean pass on these sessions only, in this run, as
    /// `hub clean` does.
    Clean { run: PathBuf, names: Vec<String> },
}

/// What the reader thread sends back to the board.
enum Reply {
    /// Boxed, since a queue is far larger than the other replies.
    Reading(Box<Result<Queue, String>>),
    /// A jump failed, with the reason.
    JumpFailed(String),
    /// The clean pass's lines, or why it could not run.
    Cleaned(Vec<String>),
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

/// Shows the board until the user quits or a signal stops it. With
/// `listen`, it also follows the steps `send_step` sends, as if the user
/// tapped or pressed them.
///
/// # Errors
///
/// `HubError::Usage` when standard input or output is not a terminal,
/// or with `listen` when another board listens.
/// `HubError::File` when the terminal cannot be read or drawn on, or
/// the socket cannot be made. `HubError::NotDone` when the reader
/// thread stops, which only a panic in it causes.
pub fn run_board(
    tools: OwnedTools,
    hubs: Hubs,
    time_zone: TimeZone,
    listen: bool,
) -> Result<Outcome, HubError> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(HubError::Usage("board needs a terminal".to_owned()));
    }
    // Dropping the listener when the board ends removes its socket.
    let listener = if listen {
        Some(StepListener::start(&tools.state_folder)?)
    } else {
        None
    };
    // A signal only sets the flag, so the loop ends and dropping the
    // screen puts the terminal back. When the loop is stuck on a closed
    // terminal, `exit_if_stuck` ends the process instead.
    let stop = Arc::new(AtomicBool::new(false));
    for signal in [SIGTERM, SIGHUP, SIGINT] {
        signal_hook::flag::register(signal, Arc::clone(&stop)).map_err(terminal_error)?;
    }
    let socket = listener
        .as_ref()
        .map(|(listener, _)| listener.path().to_owned());
    exit_if_stuck(Arc::clone(&stop), socket);
    let (reply_sender, replies) = mpsc::channel();
    let (jobs, job_receiver) = mpsc::channel();
    let (note_sender, notes) = mpsc::channel();
    let cards = tools.cards.clone();
    // The thread is never joined, so q quits at once even while a read
    // runs. The thread ends at its next send or wait after the
    // channels close, or with the process.
    thread::spawn(move || {
        read_until_closed(&tools, &hubs, &time_zone, &reply_sender, &job_receiver);
    });

    // NO_COLOR set to any value but empty turns colors off, as
    // https://no-color.org asks.
    let color = std::env::var_os("NO_COLOR").is_none_or(|value| value.is_empty());
    let mut board = Board::new(color);
    let mut screen = Screen::start().map_err(terminal_error)?;
    screen.draw(&mut board).map_err(terminal_error)?;
    'board: while !stop.load(Ordering::Relaxed) {
        let mut changed = false;
        while let Ok((key, note)) = notes.try_recv() {
            board.show_card_note(&key, note);
            changed = true;
        }
        loop {
            match replies.try_recv() {
                Ok(Reply::Reading(reading)) => {
                    board.show_reading(*reading);
                    changed = true;
                }
                Ok(Reply::JumpFailed(reason)) => {
                    board.show_status(format!("jump failed: {reason}"));
                    changed = true;
                }
                Ok(Reply::Cleaned(lines)) => {
                    board.show_clean_output(lines);
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
            let step = board.handle_input(&input);
            if !follow_step(step, &mut board, &jobs, &cards, &note_sender) {
                break;
            }
            changed = true;
        }
        if let Some((_, steps)) = &listener {
            for sent in steps.try_iter() {
                let (going, reply) =
                    control::answer_sent_step(&mut board, &sent.text, |board, step| {
                        let going = follow_step(step, board, &jobs, &cards, &note_sender);
                        if going {
                            screen.draw(board).map_err(terminal_error)?;
                        }
                        Ok(going)
                    })?;
                sent.answer(&reply);
                if !going {
                    break 'board;
                }
            }
        }
        if changed {
            screen.draw(&mut board).map_err(terminal_error)?;
        }
    }
    drop(screen);
    Ok(Outcome { lines: Vec::new() })
}

/// Reads the hubs' state once and returns the board's first screen as
/// text, `size` large, with no terminal needed.
///
/// # Errors
///
/// `HubError::Herdr` when Herdr cannot list the agents, and
/// `HubError::File` when a run folder file cannot be read.
pub fn print_board(
    tools: &Tools<'_>,
    hubs: &Hubs,
    time_zone: &TimeZone,
    size: ScreenSize,
) -> Result<Outcome, HubError> {
    // Text output carries no colors.
    let mut board = Board::new(false);
    board.show_reading(Ok(read_queue(tools, hubs, time_zone)?));
    Ok(Outcome {
        lines: board.render_lines(size.columns, size.rows),
    })
}

/// Ends the process when the board has not ended `STOP_WAIT` after
/// `stop` is set, removing `socket` first. Once the terminal closes,
/// crossterm 0.29 spins inside `event::poll`, so the loop never sees
/// `stop` and would hold `board.sock` at full CPU.
fn exit_if_stuck(stop: Arc<AtomicBool>, socket: Option<PathBuf>) {
    // A board that ends in time exits with the process, so this thread
    // is never joined.
    thread::spawn(move || {
        while !stop.load(Ordering::Relaxed) {
            thread::sleep(POLL);
        }
        thread::sleep(STOP_WAIT);
        if let Some(path) = socket {
            let _ = fs::remove_file(path);
        }
        terminal::restore_terminal();
        std::process::exit(1);
    });
}

/// Does what `step` asks beyond the board itself, such as a read or a
/// jump, and returns false when the board quits.
fn follow_step(
    step: Step,
    board: &mut Board,
    jobs: &Sender<Job>,
    cards: &CardCli,
    notes: &Sender<(CardKey, Result<CardNote, String>)>,
) -> bool {
    match step {
        Step::Quit => return false,
        Step::Refresh => {
            board.note_reading();
            // A send fails only when the reader has stopped.
            let _ = jobs.send(Job::Read);
        }
        Step::Jump(name) => {
            let _ = jobs.send(Job::Jump(name));
        }
        Step::Clean { run, names } => {
            let _ = jobs.send(Job::Clean { run, names });
        }
        Step::ReadCard(key) => read_card_aside(cards, key, notes),
        Step::Stay => {}
    }
    true
}

/// Sends a read, then runs jobs until one asks for a read or
/// `REFRESH` passes. Stops when the board stops listening.
fn read_until_closed(
    tools: &OwnedTools,
    hubs: &Hubs,
    time_zone: &TimeZone,
    replies: &Sender<Reply>,
    jobs: &Receiver<Job>,
) {
    let tools = tools.lend();
    loop {
        let reading = read_queue(&tools, hubs, time_zone).map_err(|error| error.to_string());
        if replies.send(Reply::Reading(Box::new(reading))).is_err() {
            return;
        }
        let next_read = Instant::now() + REFRESH;
        // Presses of r during the read ask for the read just sent.
        let mut read_again = false;
        for job in jobs.try_iter().filter(|job| *job != Job::Read) {
            read_again |= run_job(&tools, time_zone, job, replies);
        }
        while !read_again {
            let wait = next_read.saturating_duration_since(Instant::now());
            read_again = match jobs.recv_timeout(wait) {
                Ok(job) => run_job(&tools, time_zone, job, replies),
                Err(RecvTimeoutError::Timeout) => true,
                Err(RecvTimeoutError::Disconnected) => return,
            };
        }
    }
}

/// Runs one job and returns whether the board should read again.
fn run_job(tools: &Tools<'_>, time_zone: &TimeZone, job: Job, replies: &Sender<Reply>) -> bool {
    match job {
        Job::Read => true,
        Job::Jump(name) => {
            if let Err(error) = tools.herdr.focus_agent(&name) {
                // A send fails only after the board has quit.
                let _ = replies.send(Reply::JumpFailed(error.to_string()));
            }
            false
        }
        Job::Clean { run, names } => {
            let lines = clean_listed_sessions(tools, time_zone, &run, &names);
            let _ = replies.send(Reply::Cleaned(lines));
            true
        }
    }
}

/// Runs the clean pass on `names` in `run`, the run the clean dialog
/// listed them from, and returns its lines or why it could not run.
fn clean_listed_sessions(
    tools: &Tools<'_>,
    time_zone: &TimeZone,
    run: &Path,
    names: &[String],
) -> Vec<String> {
    let now = Timestamp::now().to_zoned(time_zone.clone());
    let cleaned =
        RunFolder::open(run).and_then(|run| clean::run_clean_listed(tools, &run, names, &now));
    cleaned.map_or_else(
        |error| vec![format!("clean failed: {error}")],
        |outcome| outcome.lines,
    )
}

/// Reads one card's note on a new thread and sends it with its key.
/// Like the reading thread, it is never joined, so q quits at once.
fn read_card_aside(
    cards: &CardCli,
    key: CardKey,
    notes: &Sender<(CardKey, Result<CardNote, String>)>,
) {
    let cards = cards.clone();
    let notes = notes.clone();
    thread::spawn(move || {
        let note = cards.read_card(&key).map_err(|error| error.to_string());
        // A send fails only after the board has quit.
        let _ = notes.send((key, note));
    });
}

/// Wraps a terminal read or write failure for `main` to print.
fn terminal_error(source: io::Error) -> HubError {
    HubError::File {
        path: PathBuf::from("the terminal"),
        source,
    }
}
