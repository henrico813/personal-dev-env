//! This module fits text to the screen's width. Text is measured and cut by
//! what the terminal draws as one character, so 面 or ❤️ counts as two
//! columns. Read the `fold(line, width)` and `escape_control_characters(text)`
//! functions to understand the module.

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// Returns how many columns the terminal draws `text` in.
pub(super) fn display_width(text: &str) -> usize {
    text.graphemes(true).map(UnicodeWidthStr::width).sum()
}

/// Returns the start of `text` that fits in `width` columns.
pub(super) fn clip(text: &str, width: usize) -> &str {
    let mut used = 0;
    for (index, cluster) in text.grapheme_indices(true) {
        used += cluster.width();
        if used > width {
            return &text[..index];
        }
    }
    text
}

/// Returns the end of `text` that fits in `width` columns.
pub(super) fn last_columns(text: &str, width: usize) -> &str {
    let mut used = 0;
    for (index, cluster) in text.grapheme_indices(true).rev() {
        used += cluster.width();
        if used > width {
            return &text[index + cluster.len()..];
        }
    }
    text
}

/// Returns `text` in at most `width` columns, cutting its middle so both ends
/// stay readable.
pub(super) fn cut_middle(text: &str, width: usize) -> String {
    if display_width(text) <= width || width < 3 {
        return clip(text, width).to_owned();
    }
    let head = clip(text, (width - 1) / 2);
    let tail = last_columns(text, width - 1 - display_width(head));
    format!("{head}…{tail}")
}

/// Breaks a line into rows of at most `width` columns, keeping every
/// character, so code keeps its spaces. Rows after the first start with two
/// more spaces, and a row ends after its last space when it has one.
pub(super) fn fold(line: &str, width: usize) -> Vec<String> {
    let mut rows = Vec::new();
    let mut rest = line;
    let mut room = width.max(1);
    while display_width(rest) > room {
        let mut head = clip(rest, room);
        // A two-column character in a one-column row still takes the row.
        if head.is_empty() {
            head = rest.graphemes(true).next().unwrap_or(rest);
        }
        // A space inside the indent would leave a row of only spaces.
        let end = match head.rfind(' ') {
            Some(space) if !head[..space].trim().is_empty() => space + 1,
            _ => head.len(),
        };
        rows.push(&rest[..end]);
        rest = &rest[end..];
        room = width.saturating_sub(2).max(1);
    }
    rows.push(rest);
    rows.iter()
        .enumerate()
        .map(|(index, row)| match index {
            0 => row.to_string(),
            _ => format!("  {row}"),
        })
        .collect()
}

/// Writes out control and text direction characters, such as `^[` for Esc,
/// because agent text could otherwise clear the screen or reorder itself.
pub(super) fn escape_control_characters(text: &str) -> String {
    let mut escaped = String::new();
    for character in text.chars() {
        match character {
            '\0'..='\x1f' => {
                escaped.push('^');
                escaped.push(char::from(b'@' + character as u8));
            }
            '\x7f' => escaped.push_str("^?"),
            '\u{061c}'
            | '\u{200b}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2060}'..='\u{2069}'
            | '\u{feff}' => escaped.push_str(&format!("\\u{:04x}", u32::from(character))),
            _ if character.is_control() => {
                escaped.push_str(&format!("\\u{:04x}", u32::from(character)));
            }
            _ => escaped.push(character),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `fold` must keep every character, spaces included, in rows no wider
    /// than the width. Otherwise a long word or a wide character runs past
    /// the phone screen, or code shows text the file does not hold.
    #[test]
    fn fold_keeps_every_character_within_width() {
        let line =
            "    面面面面面面面面面面面 println!(\"a  b\") https://example.com/docs/saving-answers";

        let rows = fold(line, 20);

        for row in &rows {
            assert!(display_width(row) <= 20, "{row:?} is too wide");
        }
        let mut joined = rows[0].clone();
        for row in &rows[1..] {
            joined.push_str(
                row.strip_prefix("  ")
                    .expect("later rows start with two spaces"),
            );
        }
        assert_eq!(joined, line);
    }

    /// `escape_control_characters` must write out Esc and text direction
    /// characters. Otherwise an agent's comment could clear the screen or
    /// reorder its text.
    #[test]
    fn escape_control_characters_writes_out_terminal_codes() {
        let escaped = escape_control_characters("a\x1b[2Jb\u{202e}c\u{85}d\u{61c}e");

        assert_eq!(escaped, "a^[[2Jb\\u202ec\\u0085d\\u061ce");
    }

    /// `fold` must count an emoji sequence as the two columns the terminal
    /// draws. Otherwise a row of hearts runs past the phone screen or splits a
    /// heart in two.
    #[test]
    fn fold_counts_emoji_sequences_as_drawn() {
        let heart = "\u{2764}\u{fe0f}";

        let rows = fold(&heart.repeat(30), 39);

        assert_eq!(rows, [heart.repeat(19), format!("  {}", heart.repeat(11))]);
    }
}
