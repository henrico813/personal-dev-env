//! Shows a Review Session one Feedback Request at a time, sized for a 40x16
//! phone popup. Answers change the Review Session in memory and nothing is
//! saved.

mod code_around;
mod draw;
mod fit;
mod keys;
mod view;

use std::io::{self, Write};
use std::path::Path;
use std::time::{Duration, Instant};

use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event;
use crossterm::terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{execute, queue};

use crate::shared::review_session::ReviewSession;
use view::{AfterKey, View};

/// Turns on mouse reports for taps and wheel turns only, sent as text such as
/// `\x1b[<65;20;5M`. A phone app sends a swipe as wheel turns.
const MOUSE_ON: &str = "\x1b[?1000h\x1b[?1006h";
const MOUSE_OFF: &str = "\x1b[?1006l\x1b[?1000l";

/// Shows each Feedback Request until the user quits or every request has User
/// Feedback. Reads each commented file under `repo` to show its code.
///
/// # Errors
///
/// Fails when the terminal cannot be read or drawn on.
pub fn run_review_screen(repo: &Path, review_session: &mut ReviewSession) -> io::Result<()> {
    let (cols, rows) = terminal::size()?;
    let size = (usize::from(cols), usize::from(rows));
    let Some(mut view) = View::new(repo, review_session, size) else {
        return Ok(());
    };
    let mut out = io::stdout();
    let _terminal = RawTerminal::start(&mut out)?;
    loop {
        let (cols, rows) = terminal::size()?;
        view.set_size(usize::from(cols), usize::from(rows));
        draw(&mut out, &view)?;
        if view.is_waiting_for_draw() {
            // Drops keys pressed before this request showed, such as a second
            // tap read while the last answer was checked, but not Ctrl-C.
            while event::poll(Duration::ZERO)? {
                if keys::is_waiting_quit(&event::read()?) {
                    return Ok(());
                }
            }
            view.start_wait(Instant::now());
        }
        if let Some(key) = keys::read_key(&event::read()?)
            && view.press_key(key, Instant::now()) == AfterKey::Quit
        {
            return Ok(());
        }
    }
}

fn draw(out: &mut impl Write, view: &View) -> io::Result<()> {
    let lines: Vec<String> = view.build_screen().iter().map(draw::Row::to_ansi).collect();
    queue!(out, MoveTo(0, 0), Clear(ClearType::All))?;
    out.write_all(lines.join("\r\n").as_bytes())?;
    out.flush()
}

/// Holds the terminal in raw mode on its own screen, and puts it back when
/// dropped, even after an error.
struct RawTerminal;

impl RawTerminal {
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
