//! The board's page as a list of elements, each with its id, text,
//! place and what a tap on it does. A tap on the live screen finds its
//! element here, and `hub board --dump`, `--send` and `--control` print
//! the same list, so an agent taps what the user taps. The list also
//! holds what the page does not draw, such as rows scrolled off or the
//! cards of a folded group, each with the step that shows it.

use serde::Serialize;

use super::groups::Group;

/// One thing the page holds, on one row.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct Element {
    /// Stays the same across reads, such as `card:DEMO-KB-007` or
    /// `group:ready`.
    pub id: String,
    pub kind: Kind,
    /// The words as drawn, or as they would be drawn, with the end
    /// trimmed.
    pub text: String,
    /// Where it is drawn, or nothing when it is hidden.
    #[serde(flatten)]
    pub place: Option<Place>,
    /// What a tap does, such as `open`, or nothing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tap: Option<&'static str>,
    /// Marks the line that Enter and space act on.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub chosen: bool,
    /// True when the page holds it but does not draw it. Set by
    /// `Element::hide`, which also fills `hiding`.
    pub hidden: bool,
    #[serde(flatten)]
    pub hiding: Option<Hiding>,
    #[serde(skip)]
    pub(super) action: Action,
}

/// Where an element is drawn. The column and the row count from 0 at
/// the top left.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct Place {
    pub column: u16,
    pub row: u16,
    /// The columns a tap finds it in, from `column` on.
    pub width: u16,
}

/// Why an element is hidden, and the next step toward showing it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct Hiding {
    pub hidden_by: HiddenBy,
    /// A step to send as it is, such as `tap group:ready`. One step
    /// may only bring the element closer, such as one page of scroll.
    pub show: String,
}

/// What keeps an element off the screen. JSON and errors name it by
/// `HiddenBy::word`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HiddenBy {
    /// Scrolled above or below what fits.
    Scroll,
    /// Inside a folded group.
    Fold,
    /// Behind a `+ N more` line.
    More,
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
    /// The sessions panel's jump bar.
    Bar,
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
    /// Pick the session at this index in the queue's sessions.
    Choose(usize),
    /// Jump to the chosen session.
    Go,
    Back,
}

impl HiddenBy {
    /// Returns `scroll`, `fold` or `more`.
    pub(crate) fn word(self) -> &'static str {
        match self {
            HiddenBy::Scroll => "scroll",
            HiddenBy::Fold => "fold",
            HiddenBy::More => "more",
        }
    }
}

impl Serialize for HiddenBy {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.word())
    }
}

impl Element {
    /// Returns the element hidden by `hidden_by`, with no place and
    /// `show` as the step toward showing it.
    pub(super) fn hide(self, hidden_by: HiddenBy, show: String) -> Element {
        Element {
            place: None,
            hidden: true,
            hiding: Some(Hiding { hidden_by, show }),
            ..self
        }
    }
}

/// What `--dump`, `--send` and `--control` print for one screen, as one
/// JSON line.
#[derive(Debug, Serialize)]
pub(crate) struct ScreenDump {
    /// The step that led here, or nothing for the first screen.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<String>,
    /// `list`, `details`, `confirm` for the clean dialog, `clean` for
    /// its output, or `quit` once the board has quit.
    pub page: &'static str,
    pub elements: Vec<Element>,
    /// The screen as text, one string per row.
    pub lines: Vec<String>,
}

/// Returns the drawn element under `column` and `row`, if any.
pub(super) fn element_at(elements: &[Element], column: u16, row: u16) -> Option<&Element> {
    elements.iter().find(|element| {
        element.place.is_some_and(|place| {
            place.row == row
                && (place.column..place.column.saturating_add(place.width)).contains(&column)
        })
    })
}
