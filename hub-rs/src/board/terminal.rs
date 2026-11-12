//! Takes the terminal for the board and gives it back. `Screen::start`
//! turns on raw mode, the alternate screen and tap reports. Dropping
//! the `Screen`, or a panic, puts the terminal back as the shell had
//! it, so Herdr gets its mouse back when the board ends.

use std::io::{self, Write};
use std::panic;

use crossterm::cursor::{Hide, Show};
use crossterm::execute;
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use super::view::Board;

/// Turns on reports of button presses and releases only (1000), with
/// positions sent as text (1006), such as `\x1b[<0;12;5M`, so a column
/// past 223 still fits. The user found that this mode leaves Herdr's
/// own wheel and taps working. Move reports, focus events, resize
/// reports and the kitty keyboard protocol stay off.
const TAPS_ON: &str = "\x1b[?1000h\x1b[?1006h";

/// Turns off every mouse mode Herdr turns off for its own terminal,
/// in the same order.
const ALL_MOUSE_OFF: &str =
    "\x1b[?1006l\x1b[?1016l\x1b[?1015l\x1b[?1005l\x1b[?1003l\x1b[?1002l\x1b[?1000l";

/// Owns the terminal while the board shows. Dropping it puts the
/// terminal back.
pub(crate) struct Screen {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
}

impl Screen {
    /// Turns on raw mode, the alternate screen and tap reports.
    pub(crate) fn start() -> io::Result<Screen> {
        install_panic_restore();
        let mut screen = Screen {
            terminal: Terminal::new(CrosstermBackend::new(io::stdout()))?,
        };
        // From here a failed step drops `screen`, which puts the
        // terminal back.
        terminal::enable_raw_mode()?;
        let out = screen.terminal.backend_mut();
        execute!(out, EnterAlternateScreen, Hide)?;
        out.write_all(ALL_MOUSE_OFF.as_bytes())?;
        out.write_all(TAPS_ON.as_bytes())?;
        out.flush()?;
        Ok(screen)
    }

    /// Draws `board` over the whole screen.
    pub(crate) fn draw(&mut self, board: &mut Board) -> io::Result<()> {
        self.terminal.draw(|frame| board.render(frame))?;
        Ok(())
    }
}

impl Drop for Screen {
    fn drop(&mut self) {
        restore_terminal();
    }
}

/// Writes the codes that turn off mouse reports, show the cursor and
/// leave the alternate screen.
fn write_restore_codes(out: &mut impl Write) -> io::Result<()> {
    out.write_all(ALL_MOUSE_OFF.as_bytes())?;
    execute!(out, Show, LeaveAlternateScreen)
}

/// Puts the terminal back. Errors are dropped, since the screen that
/// could show them is closing.
fn restore_terminal() {
    let _ = write_restore_codes(&mut io::stdout());
    let _ = terminal::disable_raw_mode();
}

/// Puts the terminal back before a panic message prints, so the message
/// shows in the shell and not on the closing alternate screen.
fn install_panic_restore() {
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        restore_terminal();
        previous(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The board's exit must hand the mouse back to Herdr. A mode left
    /// on after exit keeps sending mouse reports to the shell as text,
    /// and Herdr keeps routing the pane's wheel and taps to it.
    #[test]
    fn write_restore_codes_turns_off_tap_reports() {
        let mut out = Vec::new();
        write_restore_codes(&mut out).unwrap();
        let restore = String::from_utf8(out).unwrap();

        for code in TAPS_ON.split("\x1b[?").skip(1) {
            let number = code.strip_suffix('h').unwrap();
            let turn_off = format!("\x1b[?{number}l");
            assert!(restore.contains(&turn_off), "leaves {number} on");
        }
    }
}
