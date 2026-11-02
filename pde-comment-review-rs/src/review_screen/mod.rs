//! This module shows a Review Session one Feedback Request at a time, sized
//! for a 40x16 phone popup. Answers change the Review Session in memory and
//! nothing is saved. Read the `show_review_screen(repo, review_session)`
//! function to understand the module.

mod draw;
mod fit;
mod keys;
mod screen;
mod surrounding_code;

use std::io::{self, Write};
use std::path::Path;
use std::time::{Duration, Instant};

use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event;
use crossterm::terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{execute, queue};

use crate::shared::review_session::ReviewSession;
use screen::{AfterKey, ReviewScreen};

/// Turns on mouse reports for taps and wheel turns only, sent as text such as
/// `\x1b[<65;20;5M`. A phone app sends a swipe as wheel turns.
const MOUSE_ON: &str = "\x1b[?1000h\x1b[?1006h";
/// Turns off the mouse reports `MOUSE_ON` turned on.
const MOUSE_OFF: &str = "\x1b[?1006l\x1b[?1000l";

/// Shows each Feedback Request until the user quits or every request has User
/// Feedback. Reads each commented file under `repo` to show its code.
///
/// # Errors
///
/// Fails when the terminal cannot be read or drawn on.
pub fn show_review_screen(repo: &Path, review_session: &mut ReviewSession) -> io::Result<()> {
    let (cols, rows) = terminal::size()?;
    let size = (usize::from(cols), usize::from(rows));
    let Some(mut screen) = ReviewScreen::new(repo, review_session, size) else {
        return Ok(());
    };
    let mut out = io::stdout();
    let _terminal = RawTerminal::start(&mut out)?;
    loop {
        let (cols, rows) = terminal::size()?;
        screen.set_size(usize::from(cols), usize::from(rows));
        draw(&mut out, &screen)?;
        if screen.is_waiting_for_draw() {
            // Drops keys pressed before this request showed, such as a second
            // tap read while the last answer was checked, but not Ctrl-C.
            while event::poll(Duration::ZERO)? {
                if keys::is_waiting_quit(&event::read()?) {
                    return Ok(());
                }
            }
            screen.start_wait(Instant::now());
        }
        if let Some(key) = keys::read_key(&event::read()?)
            && screen.press_key(key, Instant::now()) == AfterKey::Quit
        {
            return Ok(());
        }
    }
}

/// Draws every row of the screen over the last ones.
fn draw(out: &mut impl Write, screen: &ReviewScreen) -> io::Result<()> {
    let lines: Vec<String> = screen
        .build_screen()
        .iter()
        .map(draw::Row::to_ansi)
        .collect();
    queue!(out, MoveTo(0, 0), Clear(ClearType::All))?;
    out.write_all(lines.join("\r\n").as_bytes())?;
    out.flush()
}

/// Holds the terminal in raw mode on its own screen, and puts it back when
/// dropped, even after an error.
struct RawTerminal;

impl RawTerminal {
    /// Turns on raw mode, the alternate screen and mouse reports.
    fn start(out: &mut impl Write) -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        let raw_terminal = RawTerminal;
        execute!(out, EnterAlternateScreen, Hide)?;
        out.write_all(MOUSE_ON.as_bytes())?;
        out.flush()?;
        Ok(raw_terminal)
    }
}

impl Drop for RawTerminal {
    fn drop(&mut self) {
        let mut out = io::stdout();
        // Nothing could show these errors while the terminal is put back.
        let _ = out.write_all(MOUSE_OFF.as_bytes());
        let _ = execute!(out, Show, LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}
