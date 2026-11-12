//! Runs `hub board`, which for now only checks that the board's
//! mouse reports leave Herdr working. The user switches between the
//! mouse modes with keys 0 to 4 and tries Herdr's wheel and taps in
//! other panes. Read `run_board` first.

mod terminal;

use std::io::{self, IsTerminal};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};

use crate::commands::Outcome;
use crate::shared::HubError;
use terminal::{MouseMode, Screen};

/// How long the board waits for input before it checks for a stop
/// signal again.
const POLL: Duration = Duration::from_millis(200);

/// Shows the mouse check until the user quits or a signal stops it,
/// then returns one line with the counts.
///
/// # Errors
///
/// `HubError::Usage` when standard input or output is not a terminal.
/// `HubError::File` when the terminal cannot be read or drawn on.
pub fn run_board() -> Result<Outcome, HubError> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(HubError::Usage("board needs a terminal".to_owned()));
    }
    // A signal only sets the flag, so the loop ends and dropping the
    // screen puts the terminal back.
    let stop = Arc::new(AtomicBool::new(false));
    for signal in [SIGTERM, SIGHUP, SIGINT] {
        signal_hook::flag::register(signal, Arc::clone(&stop)).map_err(terminal_error)?;
    }
    let mut check = MouseCheck::new(MouseMode::Taps);
    let mut screen = Screen::start(check.mode).map_err(terminal_error)?;
    screen.draw(&check.describe()).map_err(terminal_error)?;
    while !stop.load(Ordering::Relaxed) {
        if !event::poll(POLL).map_err(terminal_error)? {
            continue;
        }
        let old_mode = check.mode;
        let input = event::read().map_err(terminal_error)?;
        if check.handle_input(&input) == Step::Quit {
            break;
        }
        if check.mode != old_mode {
            screen.set_mouse_mode(check.mode).map_err(terminal_error)?;
        }
        screen.draw(&check.describe()).map_err(terminal_error)?;
    }
    drop(screen);
    Ok(Outcome::line(check.summary()))
}

/// What the board does after one input.
#[derive(Debug, PartialEq, Eq)]
enum Step {
    Stay,
    Quit,
}

/// The mouse mode and what the terminal has sent in it so far.
struct MouseCheck {
    mode: MouseMode,
    taps: u32,
    /// Counts drags as well as plain moves.
    moves: u32,
    wheel_turns: u32,
    /// Words for the newest input, such as `tap at 12,5` or `key x`.
    last: String,
}

impl MouseCheck {
    /// Returns a check in `mode` that has seen no input yet.
    fn new(mode: MouseMode) -> MouseCheck {
        MouseCheck {
            mode,
            taps: 0,
            moves: 0,
            wheel_turns: 0,
            last: "nothing yet".to_owned(),
        }
    }

    /// Counts a mouse report or acts on a key. Raw mode turns Ctrl-C
    /// into a key, so Ctrl-C quits here, as q and Esc do.
    fn handle_input(&mut self, input: &Event) -> Step {
        match input {
            Event::Key(key) => self.handle_key(key),
            Event::Mouse(mouse) => {
                self.count_mouse(mouse);
                Step::Stay
            }
            _ => Step::Stay,
        }
    }

    /// Quits, switches the mouse mode, or only notes the key.
    fn handle_key(&mut self, key: &KeyEvent) -> Step {
        let ctrl_c =
            key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl_c || key.code == KeyCode::Char('q') || key.code == KeyCode::Esc {
            return Step::Quit;
        }
        if let KeyCode::Char(typed) = key.code
            && let Some(mode) = MouseMode::from_key(typed)
        {
            self.mode = mode;
        }
        self.last = format!("key {}", key.code);
        Step::Stay
    }

    /// Adds a mouse report to its count. A button release counts as
    /// nothing, since its tap already counted.
    fn count_mouse(&mut self, mouse: &MouseEvent) {
        let (count, what) = match mouse.kind {
            MouseEventKind::Down(_) => (&mut self.taps, "tap"),
            MouseEventKind::Moved | MouseEventKind::Drag(_) => (&mut self.moves, "move"),
            MouseEventKind::ScrollUp
            | MouseEventKind::ScrollDown
            | MouseEventKind::ScrollLeft
            | MouseEventKind::ScrollRight => (&mut self.wheel_turns, "wheel"),
            MouseEventKind::Up(_) => return,
        };
        *count = count.saturating_add(1);
        self.last = format!("{what} at {},{}", mouse.column, mouse.row);
    }

    /// Returns the screen's lines, each at most 40 columns wide so they
    /// fit a phone.
    fn describe(&self) -> Vec<String> {
        vec![
            "hub board mouse check, q quits".to_owned(),
            format!("now: {}", self.mode.label()),
            "1 taps only   2 taps and drags".to_owned(),
            "3 every move  4 every move, pixels".to_owned(),
            "0 no mouse".to_owned(),
            format!("taps {}", self.taps),
            format!("moves {}", self.moves),
            format!("wheel {}", self.wheel_turns),
            format!("last: {}", self.last),
            String::new(),
            "Scroll and click in another pane.".to_owned(),
            "Does Herdr still work?".to_owned(),
        ]
    }

    /// Returns the line `hub board` prints once the screen closes.
    fn summary(&self) -> String {
        format!(
            "taps {}, moves {}, wheel {}, last mode {}",
            self.taps,
            self.moves,
            self.wheel_turns,
            self.mode.label()
        )
    }
}

/// Wraps a terminal read or write failure for `main` to print.
fn terminal_error(source: io::Error) -> HubError {
    HubError::File {
        path: PathBuf::from("the terminal"),
        source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crossterm::event::{KeyEventKind, KeyEventState};

    /// Raw mode turns Ctrl-C into a key, so a board that misses one
    /// of these keys leaves the user no way out but closing the pane.
    #[test]
    fn handle_input_quits_on_exit_keys() {
        let cases = [
            ("q", KeyCode::Char('q'), KeyModifiers::NONE),
            ("escape", KeyCode::Esc, KeyModifiers::NONE),
            ("ctrl-c", KeyCode::Char('c'), KeyModifiers::CONTROL),
        ];

        for (case_name, code, modifiers) in cases {
            let mut check = MouseCheck::new(MouseMode::Taps);
            let key = KeyEvent {
                code,
                modifiers,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            };

            let step = check.handle_input(&Event::Key(key));

            assert_eq!(step, Step::Quit, "{case_name}");
        }
    }

    /// The user tries the board on a phone 40 columns wide. A longer
    /// line wraps and pushes the instructions off the screen.
    #[test]
    fn describe_fits_forty_columns() {
        for key in ['0', '1', '2', '3', '4'] {
            let mut check = MouseCheck::new(MouseMode::from_key(key).unwrap());
            check.taps = u32::MAX;
            check.moves = u32::MAX;
            check.wheel_turns = u32::MAX - 1;
            let wheel = MouseEvent {
                kind: MouseEventKind::ScrollDown,
                column: 199,
                row: 59,
                modifiers: KeyModifiers::NONE,
            };
            check.handle_input(&Event::Mouse(wheel));

            let lines = check.describe();

            for line in lines {
                assert!(line.chars().count() <= 40, "{key}: {line}");
            }
        }
    }
}
