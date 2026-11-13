//! Takes the terminal for the board and gives it back. `Screen::start`
//! turns on raw mode, the alternate screen and tap reports. Dropping
//! the `Screen`, or a panic, puts the terminal back as the shell had
//! it, so Herdr gets its mouse back when the board ends.

use std::io::{self, Write};
use std::panic;
use std::sync::LazyLock;

use crossterm::cursor::{Hide, Show};
use crossterm::execute;
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use regex::bytes::{Captures, Regex};

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

/// A style code, such as `\x1b[1;38;5;3m`, with its numbers in group 1.
static STYLE_CODE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\x1b\[([0-9;]*)m").unwrap());

/// Owns the terminal while the board shows. Dropping it puts the
/// terminal back.
pub(crate) struct Screen {
    terminal: Terminal<CrosstermBackend<BasicColors<io::Stdout>>>,
}

/// Holds what crossterm writes until a flush, then writes it with each
/// of the 16 theme colors as its basic code, such as `33` for yellow.
/// crossterm writes them as 256-color codes, such as `38;5;3`, and some
/// phone terminals, such as Moshi, show those in other colors.
struct BasicColors<W: Write> {
    out: W,
    pending: Vec<u8>,
}

impl Screen {
    /// Turns on raw mode, the alternate screen and tap reports.
    pub(crate) fn start() -> io::Result<Screen> {
        install_panic_restore();
        let mut screen = Screen {
            terminal: Terminal::new(CrosstermBackend::new(BasicColors::new(io::stdout())))?,
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

impl<W: Write> BasicColors<W> {
    fn new(out: W) -> BasicColors<W> {
        BasicColors {
            out,
            pending: Vec::new(),
        }
    }
}

/// ratatui flushes after each frame, so each flush writes whole codes.
impl<W: Write> Write for BasicColors<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.pending.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        let basic = STYLE_CODE.replace_all(&self.pending, |found: &Captures| {
            // The pattern matches only digits and semicolons.
            let numbers = std::str::from_utf8(&found[1]).unwrap_or_default();
            format!("\x1b[{}m", to_basic_colors(numbers)).into_bytes()
        });
        self.out.write_all(&basic)?;
        self.pending.clear();
        self.out.flush()
    }
}

/// Returns a style code's numbers with each `38;5;N` or `48;5;N` for N
/// under 16 written as its basic code, such as `33` for `38;5;3` and
/// `91` for `38;5;9`. Other numbers stay as they are.
fn to_basic_colors(numbers: &str) -> String {
    let parts: Vec<&str> = numbers.split(';').collect();
    let mut rest = parts.as_slice();
    let mut basic = Vec::new();
    while let Some((first, after)) = rest.split_first() {
        if let [layer @ ("38" | "48"), "5", color, tail @ ..] = rest
            && let Ok(color @ 0..16) = color.parse::<u8>()
        {
            // Text colors start at 30, backgrounds at 40, and the bright
            // eight 60 higher.
            let base = if *layer == "38" { 30 } else { 40 };
            let code = if color < 8 {
                base + color
            } else {
                base + 60 + color - 8
            };
            basic.push(code.to_string());
            rest = tail;
        } else {
            basic.push((*first).to_owned());
            rest = after;
        }
    }
    basic.join(";")
}

/// Writes the codes that turn off mouse reports, show the cursor and
/// leave the alternate screen.
fn write_restore_codes(out: &mut impl Write) -> io::Result<()> {
    out.write_all(ALL_MOUSE_OFF.as_bytes())?;
    execute!(out, Show, LeaveAlternateScreen)
}

/// Puts the terminal back. Errors are dropped, since the screen that
/// could show them is closing.
pub(crate) fn restore_terminal() {
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

    use ratatui::backend::Backend;
    use ratatui::buffer::Cell;
    use ratatui::style::Color;

    /// Phone terminals such as Moshi use their theme's colors for the
    /// basic codes, such as `33`. A group drawn with a 256-color code
    /// shows in another color than the user's theme gives it.
    #[test]
    fn basic_colors_writes_basic_codes() {
        let mut written = Vec::new();
        let mut backend = CrosstermBackend::new(BasicColors::new(&mut written));
        let mut yellow = Cell::new("◐");
        yellow.set_fg(Color::Yellow);
        let mut bright_red = Cell::new("!");
        bright_red.set_fg(Color::LightRed);

        backend
            .draw([(0, 0, &yellow), (1, 0, &bright_red)].into_iter())
            .unwrap();
        Backend::flush(&mut backend).unwrap();
        drop(backend);

        let written = String::from_utf8(written).unwrap();
        assert!(written.contains("\x1b[33;49m◐"), "{written:?}");
        assert!(written.contains("\x1b[91;49m!"), "{written:?}");
        assert!(!written.contains("38;5;"), "{written:?}");
    }

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
