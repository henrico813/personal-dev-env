//! Reads the questions that wait on the user from the cards. `hub ask`
//! puts each one in its card's Open Questions and marks the card
//! needs-you with the question's title, so the open mark finds the item.

use crate::external::cards::{CardError, CardKey, Cards};

/// A card's open question.
#[derive(Debug)]
pub(crate) struct OpenQuestion {
    pub key: CardKey,
    /// The card's project, such as `DevEnv`.
    pub project: String,
    /// The text of the card's open needs-you mark.
    pub title: String,
    /// The Open Questions item's lines after its title, or none when the
    /// mark names no item, as when a worker marked the card itself.
    pub lines: Vec<String>,
}

/// Returns the open question of each card whose needs-you mark is open,
/// in key order. A card answered between the search and its read is
/// left out.
///
/// # Errors
///
/// The first `card` failure, so no caller shows a partial list as
/// all that waits.
pub(crate) fn read_open_questions(cards: &dyn Cards) -> Result<Vec<OpenQuestion>, CardError> {
    let keys = cards.list_needs_you()?;
    let notes = cards.read_cards(&keys);
    let mut questions = Vec::new();
    for (key, note) in keys.into_iter().zip(notes) {
        let note = note?;
        let Some(title) = note.needs_you else {
            continue;
        };
        let lines = note
            .open_questions
            .into_iter()
            .find(|item| item.first() == Some(&title))
            .map(|item| item[1..].to_vec())
            .unwrap_or_default();
        questions.push(OpenQuestion {
            key,
            project: note.project,
            title,
            lines,
        });
    }
    Ok(questions)
}
