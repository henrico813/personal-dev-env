//! The board's screen as a list of elements, each with its id, text,
//! place and what a tap on it does. A tap on the live screen finds its
//! element here, and `hub board --dump`, `--send` and `--control` print
//! the same list, so an agent taps what the user taps.

use serde::Serialize;

use super::groups::Group;

/// One thing the screen shows, on one row.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct Element {
    /// Stays the same across reads, such as `card:DEMO-KB-007` or
    /// `group:ready`.
    pub id: String,
    pub kind: Kind,
    /// The words as drawn, with the end trimmed.
    pub text: String,
    /// The first column and the row, both counted from 0 at the top
    /// left.
    pub column: u16,
    pub row: u16,
    /// The columns a tap finds it in, from `column` on.
    pub width: u16,
    /// What a tap does, such as `open`, or nothing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tap: Option<&'static str>,
    /// Marks the line that Enter and space act on.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub chosen: bool,
    #[serde(skip)]
    pub(super) action: Action,
}

/// What an element is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Kind {
    /// A count on the top line, such as `?2 you`.
    Count,
    Heading,
    Item,
    /// The `+ N more` line.
    More,
    Back,
    Text,
}

/// What the board does on a tap on an element.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Action {
    None,
    /// Open the group and scroll to it.
    Jump(Group),
    /// Act on the list line at this index, as Enter does.
    Line(usize),
    Back,
}

/// What `--dump`, `--send` and `--control` print for one screen, as one
/// JSON line.
#[derive(Debug, Serialize)]
pub(crate) struct ScreenDump {
    /// The step that led here, or nothing for the first screen.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<String>,
    /// `list`, `details`, or `quit` once the board has quit.
    pub page: &'static str,
    pub elements: Vec<Element>,
    /// The screen as text, one string per row.
    pub lines: Vec<String>,
}

/// Returns the element under `column` and `row`, if any.
pub(super) fn element_at(elements: &[Element], column: u16, row: u16) -> Option<&Element> {
    elements.iter().find(|element| {
        element.row == row
            && (element.column..element.column.saturating_add(element.width)).contains(&column)
    })
}
