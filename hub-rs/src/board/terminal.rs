//! Takes the terminal for the board and gives it back. `Screen::start`
//! turns on raw mode, the alternate screen and one kind of mouse
//! report. Dropping the `Screen`, or a panic, puts the terminal back as
//! the shell had it, so Herdr gets its mouse back when the board ends.

use std::io::{self, Write};
use std::panic;

use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{execute, queue};

/// Which mouse reports the terminal sends to the board. Herdr passes
/// the mouse to the board only in the modes the board asked for, so
/// each mode can change what Herdr itself does with the mouse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MouseMode {
    Off,
    Taps,
    TapsAndDrags,
    EveryMove,
    EveryMoveInPixels,
}

impl MouseMode {
    /// Returns the mode for keys `0` to `4`, such as `Taps` for `1`.
    pub(crate) fn from_key(key: char) -> Option<MouseMode> {
        match key {
            '0' => Some(MouseMode::Off),
            '1' => Some(MouseMode::Taps),
            '2' => Some(MouseMode::TapsAndDrags),
            '3' => Some(MouseMode::EveryMove),
            '4' => Some(MouseMode::EveryMoveInPixels),
            _ => None,
        }
    }

    /// Returns the key and the words the screen shows for the mode.
    pub(crate) fn label(self) -> &'static str {
        match self {
            MouseMode::Off => "0 no mouse",
            MouseMode::Taps => "1 taps only",
            MouseMode::TapsAndDrags => "2 taps and drags",
            MouseMode::EveryMove => "3 every move",
            MouseMode::EveryMoveInPixels => "4 every move, pixels",
        }
    }

    /// Returns the codes that turn the mode on. 1006 sends positions as
    /// text, such as `\x1b[<0;12;5M`, so a column past 223 still fits.
    fn turn_on_codes(self) -> &'static str {
        match self {
            MouseMode::Off => "",
            MouseMode::Taps => "\x1b[?1000h\x1b[?1006h",
            MouseMode::TapsAndDrags => "\x1b[?1002h\x1b[?1006h",
            // Textual 8, which drew the Python dashboard, sends these.
            MouseMode::EveryMove => "\x1b[?1000h\x1b[?1003h\x1b[?1015h\x1b[?1006h",
            // Textual adds 1016 when the terminal supports it.
            MouseMode::EveryMoveInPixels => {
                "\x1b[?1000h\x1b[?1003h\x1b[?1015h\x1b[?1006h\x1b[?1016h"
            }
        }
    }
}

/// Turns off every mouse mode Herdr turns off for its own terminal,
/// in the same order.
const ALL_MOUSE_OFF: &str =
    "\x1b[?1006l\x1b[?1016l\x1b[?1015l\x1b[?1005l\x1b[?1003l\x1b[?1002l\x1b[?1000l";

/// Owns the terminal while the board shows. Dropping it puts the
/// terminal back.
pub(crate) struct Screen {
    out: io::Stdout,
}

impl Screen {
    /// Turns on raw mode, the alternate screen and `mode`.
    pub(crate) fn start(mode: MouseMode) -> io::Result<Screen> {
        install_panic_restore();
        terminal::enable_raw_mode()?;
        // From here on a failed step returns early and drops the
        // screen, which turns raw mode off again.
        let mut screen = Screen { out: io::stdout() };
        execute!(screen.out, EnterAlternateScreen, Hide)?;
        screen.set_mouse_mode(mode)?;
        Ok(screen)
    }

    /// Switches the terminal to `mode`.
    pub(crate) fn set_mouse_mode(&mut self, mode: MouseMode) -> io::Result<()> {
        write_mouse_mode(&mut self.out, mode)
    }

    /// Draws `lines` over the whole screen.
    pub(crate) fn draw(&mut self, lines: &[String]) -> io::Result<()> {
        queue!(self.out, MoveTo(0, 0), Clear(ClearType::All))?;
        self.out.write_all(lines.join("\r\n").as_bytes())?;
        self.out.flush()
    }
}

impl Drop for Screen {
    fn drop(&mut self) {
        restore_terminal();
    }
}

/// Writes the codes that switch the terminal to `mode`. The old reports
/// go off first, since encodings such as 1016 stay on until turned off.
fn write_mouse_mode(out: &mut impl Write, mode: MouseMode) -> io::Result<()> {
    out.write_all(ALL_MOUSE_OFF.as_bytes())?;
    out.write_all(mode.turn_on_codes().as_bytes())?;
    out.flush()
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

    /// Encodings such as 1016 stay on until turned off. Without the
    /// off codes, a switch from 4 to 1 still sends pixel positions, and
    /// the user tests the wrong mode.
    #[test]
    fn write_mouse_mode_clears_old_reports_first() {
        let cases = [
            ("taps", MouseMode::Taps, "\x1b[?1000h\x1b[?1006h"),
            ("no-mouse", MouseMode::Off, ""),
        ];

        for (case_name, mode, turn_on) in cases {
            let mut out = Vec::new();
            write_mouse_mode(&mut out, mode).unwrap();

            let expected = format!("{ALL_MOUSE_OFF}{turn_on}");
            assert_eq!(String::from_utf8(out).unwrap(), expected, "{case_name}");
        }
    }

    /// The board's exit must hand the mouse back to Herdr. A mode left
    /// on after exit keeps sending mouse reports to the shell as text,
    /// and Herdr keeps routing the pane's wheel and taps to it.
    #[test]
    fn write_restore_codes_turns_off_every_mode() {
        let mut out = Vec::new();
        write_restore_codes(&mut out).unwrap();
        let restore = String::from_utf8(out).unwrap();

        for key in ['0', '1', '2', '3', '4'] {
            let mode = MouseMode::from_key(key).unwrap();
            for code in mode.turn_on_codes().split("\x1b[?").skip(1) {
                let number = code.strip_suffix('h').unwrap();
                let turn_off = format!("\x1b[?{number}l");
                assert!(restore.contains(&turn_off), "{mode:?} leaves {number} on");
            }
        }
    }
}
