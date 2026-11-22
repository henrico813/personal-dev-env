//! Reads the questions that wait on the user from the cards. `run_ask`
//! puts each one in its card's Open Questions and marks the card
//! needs-you with the question's title, so the open mark finds the item.

use std::sync::LazyLock;

use jiff::Timestamp;
use jiff::civil::Date;
use jiff::tz::TimeZone;
use regex::Regex;

use crate::external::cards::{CardError, CardKey, Cards};
use crate::external::opencode::{OpenCodeDb, OpenCodeError};
use crate::shared::messages::SEND_PREFIX;
use crate::watch::PROD_PREFIX;

/// How hubs and the watch started their messages before `SEND_PREFIX`,
/// such as `From the hub: ...`, `From auto-hub-6. ...` or `Your
/// context is about 168k. ...`. Older sessions still hold them.
static OLD_AGENT_TEXT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:From (?:the hub|\S+-hub-\d+)\b|Your context is about )").unwrap()
});

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
    /// The card's OpenCode session IDs, oldest first.
    pub sessions: Vec<String>,
    /// The date of the needs-you mark.
    pub asked_on: Option<Date>,
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
        // `run_ask` adds its item last, after any older item with that title.
        let lines = note
            .open_questions
            .into_iter()
            .rfind(|item| item.first() == Some(&title))
            .map(|item| item[1..].to_vec())
            .unwrap_or_default();
        questions.push(OpenQuestion {
            key,
            project: note.project,
            title,
            lines,
            sessions: note.sessions,
            asked_on: note.needs_you_date,
        });
    }
    Ok(questions)
}

/// Returns each of the question's sessions where a person typed a
/// message on or after the day it was asked, with the newest such
/// message's time. The user may have answered there, so the hub reads
/// that pane before it asks again. Messages that start like the hub's
/// or the watch's, now or in older builds, are not a person's. The
/// card Log gives only the
/// day, so a message typed earlier that day counts too.
///
/// # Errors
///
/// The first OpenCode read that fails.
pub(crate) fn find_typed_since_asked(
    opencode: &OpenCodeDb,
    question: &OpenQuestion,
    time_zone: &TimeZone,
) -> Result<Vec<(String, Timestamp)>, OpenCodeError> {
    let Some(since) = question
        .asked_on
        .and_then(|date| date.to_zoned(time_zone.clone()).ok())
    else {
        return Ok(Vec::new());
    };
    let mut typed = Vec::new();
    for session in &question.sessions {
        let newest = opencode
            .read_user_messages_since(session, since.timestamp())?
            .into_iter()
            .filter(|message| {
                !message.text.trim().is_empty()
                    && !message.text.starts_with(SEND_PREFIX)
                    && !message.text.starts_with(PROD_PREFIX)
                    && !OLD_AGENT_TEXT.is_match(&message.text)
            })
            .map(|message| message.at)
            .max();
        typed.extend(newest.map(|at| (session.clone(), at)));
    }
    Ok(typed)
}
