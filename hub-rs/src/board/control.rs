//! Runs `hub board --dump` and `hub board --control`, which show the
//! board to an agent as JSON. `--control` reads one step per line, such
//! as `tap card:DEMO-KB-007` or `press esc`, and prints the screen after
//! each, so an agent can tap what the user taps without a phone. Read
//! `control_board` first.

use std::io::{self, BufRead, ErrorKind, Write};
use std::path::PathBuf;

use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use jiff::tz::TimeZone;

use super::ScreenSize;
use super::elements::ScreenDump;
use super::queue::read_queue;
use super::view::{Board, Step};
use crate::commands::Outcome;
use crate::shared::run_folder::RunFolder;
use crate::shared::{HubError, Tools};

/// Reads the hub's state once and returns the board's first screen,
/// `size` large, as one JSON line.
///
/// # Errors
///
/// `HubError::Herdr` when Herdr cannot list the agents, and
/// `HubError::File` when a run folder file cannot be read.
pub fn dump_board(
    tools: &Tools<'_>,
    run: &RunFolder,
    time_zone: &TimeZone,
    size: ScreenSize,
) -> Result<Outcome, HubError> {
    let mut board = Board::new(false);
    board.show_reading(Ok(read_queue(tools, run, time_zone)?));
    let dump = board.dump(size.columns, size.rows, None);
    Ok(Outcome {
        lines: vec![to_json(&dump)?],
    })
}

/// Writes the board's first screen to `output` as one JSON line, then
/// one more after each step it reads from `input`. It stops at the end
/// of `input`, after a step that quits, or when the reader of `output`
/// closes it. r reads the hub's state again, and opening a card that
/// the read did not get reads it before the screen prints.
///
/// # Errors
///
/// `HubError::Usage` for a step it cannot follow, such as a tap on an
/// id the screen does not show. The error names the step's line, and
/// the screens before it are already written. `HubError::Herdr` and
/// `HubError::File` as `dump_board` gives them, and `HubError::File`
/// when `input` or `output` fails.
pub fn control_board(
    tools: &Tools<'_>,
    run: &RunFolder,
    time_zone: &TimeZone,
    size: ScreenSize,
    input: &mut dyn BufRead,
    output: &mut dyn Write,
) -> Result<Outcome, HubError> {
    let done = Outcome { lines: Vec::new() };
    let mut board = Board::new(false);
    board.show_reading(Ok(read_queue(tools, run, time_zone)?));
    if !write_dump(output, &board.dump(size.columns, size.rows, None))? {
        return Ok(done);
    }
    for (number, line) in input.lines().enumerate() {
        let line = line.map_err(|source| HubError::File {
            path: PathBuf::from("standard input"),
            source,
        })?;
        let step = line.trim();
        if step.is_empty() {
            continue;
        }
        let event = parse_step(step, &board)
            .map_err(|reason| HubError::Usage(format!("line {}: {reason}", number + 1)))?;
        let dump = match board.handle_input(&event) {
            Step::Quit => ScreenDump {
                step: Some(step.to_owned()),
                page: "quit",
                elements: Vec::new(),
                lines: Vec::new(),
            },
            Step::Refresh => {
                let reading = read_queue(tools, run, time_zone).map_err(|error| error.to_string());
                board.show_reading(reading);
                board.dump(size.columns, size.rows, Some(step.to_owned()))
            }
            Step::ReadCard(key) => {
                let note = tools
                    .cards
                    .read_card(&key)
                    .map_err(|error| error.to_string());
                board.show_card_note(&key, note);
                board.dump(size.columns, size.rows, Some(step.to_owned()))
            }
            Step::Stay => board.dump(size.columns, size.rows, Some(step.to_owned())),
        };
        if !write_dump(output, &dump)? || dump.page == "quit" {
            break;
        }
    }
    Ok(done)
}

/// Turns one `--control` line into the input it stands for. `tap ID`
/// taps the first column of the element with that id, `tap COLUMN ROW`
/// taps that place, `press KEY` presses a key such as `q`, `esc` or
/// `ctrl-c`, and `wheel up` or `wheel down` turns the wheel.
fn parse_step(step: &str, board: &Board) -> Result<Event, String> {
    let (verb, rest) = step.split_once(' ').unwrap_or((step, ""));
    let rest = rest.trim();
    let mouse = |kind, column, row| {
        Event::Mouse(MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        })
    };
    match verb {
        "tap" => {
            let tap = MouseEventKind::Down(MouseButton::Left);
            if let Some((column, row)) = parse_place(rest) {
                return Ok(mouse(tap, column, row));
            }
            let elements = board.elements();
            let element = elements
                .iter()
                .find(|element| element.id == rest)
                .ok_or_else(|| format!("the screen shows no {rest}"))?;
            Ok(mouse(tap, element.column, element.row))
        }
        "press" => parse_key(rest)
            .map(Event::Key)
            .ok_or_else(|| format!("{rest} is not a key such as q, esc or ctrl-c")),
        "wheel" => match rest {
            "up" => Ok(mouse(MouseEventKind::ScrollUp, 0, 0)),
            "down" => Ok(mouse(MouseEventKind::ScrollDown, 0, 0)),
            _ => Err(format!("wheel takes up or down, not {rest}")),
        },
        _ => Err(format!("{step} is not a tap, press or wheel step")),
    }
}

/// Reads `COLUMN ROW`, such as `0 5`.
fn parse_place(text: &str) -> Option<(u16, u16)> {
    let (column, row) = text.split_once(' ')?;
    Some((column.parse().ok()?, row.trim().parse().ok()?))
}

/// Reads a key name, such as `enter`, `pagedown` or one character.
fn parse_key(name: &str) -> Option<KeyEvent> {
    let code = match name {
        "ctrl-c" => return Some(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
        "enter" => KeyCode::Enter,
        "esc" => KeyCode::Esc,
        "space" => KeyCode::Char(' '),
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "pageup" => KeyCode::PageUp,
        "pagedown" => KeyCode::PageDown,
        _ => {
            let mut chars = name.chars();
            match (chars.next(), chars.next()) {
                (Some(only), None) => KeyCode::Char(only),
                _ => return None,
            }
        }
    };
    Some(KeyEvent::new(code, KeyModifiers::NONE))
}

/// Writes `dump` as one line and returns false when the reader closed
/// `output` early, as `head` does.
fn write_dump(output: &mut dyn Write, dump: &ScreenDump) -> Result<bool, HubError> {
    let written = writeln!(output, "{}", to_json(dump)?).and_then(|()| output.flush());
    match written {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == ErrorKind::BrokenPipe => Ok(false),
        Err(source) => Err(output_error(source)),
    }
}

/// Returns `dump` as one line of JSON.
fn to_json(dump: &ScreenDump) -> Result<String, HubError> {
    serde_json::to_string(dump).map_err(|error| output_error(io::Error::from(error)))
}

/// Wraps a failure to write standard output for `main` to print.
fn output_error(source: io::Error) -> HubError {
    HubError::File {
        path: PathBuf::from("standard output"),
        source,
    }
}
