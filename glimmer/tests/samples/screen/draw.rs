use std::io::{self, Write};

/// Draws the rows on the terminal, clearing each row first so a shorter row
/// leaves nothing of the longer row it replaces.
///
/// 面 and other wide characters take two columns.
pub fn draw_rows(out: &mut impl Write, rows: &[String]) -> io::Result<()> {
    for (index, row) in rows.iter().enumerate() {
        // Moves to the row's start and clears it.
        write!(out, "\x1b[{};1H\x1b[2K{row}", index + 1)?;
    }
    out.flush()
}
