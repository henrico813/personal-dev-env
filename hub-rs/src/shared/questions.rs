//! Reads the questions that wait on the user from the cards. Each one
//! is an Open Questions item on a card in `LISTED_STAGES`, which
//! `card` numbers and dates.

use std::sync::LazyLock;

use jiff::Timestamp;
use jiff::civil::Date;
use jiff::tz::TimeZone;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::external::cards::{CardError, CardKey, Cards, Stage};
use crate::external::opencode::{OpenCodeDb, OpenCodeError};
use crate::shared::messages::SEND_PREFIX;
use crate::watch::PROD_PREFIX;

/// How hubs and the watch started their messages before `SEND_PREFIX`,
/// such as `From the hub: ...`, `From auto-hub-6. ...` or `Your
/// context is about 168k. ...`. Older sessions still hold them.
static OLD_AGENT_TEXT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:From (?:the hub|\S+-hub-\d+)\b|Your context is about )").unwrap()
});

/// The stages whose cards' Open Questions items wait on the user. A
/// draft card is still being written, and a done or dropped card needs
/// nothing more.
pub(crate) const LISTED_STAGES: [Stage; 4] =
    [Stage::Todo, Stage::Active, Stage::InReview, Stage::Blocked];

/// One Open Questions item that waits on the user.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct OpenQuestion {
    pub key: CardKey,
    /// The card's project, such as `DevEnv`.
    pub project: String,
    /// The item's Q number, which stays until the item is answered.
    pub number: u32,
    /// The item's first line, after its `Q3 asked 2026-11-25: ` head.
    pub title: String,
    /// The item's later lines, such as its choices.
    pub lines: Vec<String>,
    /// The card's OpenCode session IDs, oldest first.
    pub sessions: Vec<String>,
    /// The day the item was asked, or `None` for an item typed by hand.
    pub asked_on: Option<Date>,
}

impl OpenQuestion {
    /// Returns the key and Q number that name the item, such as
    /// `DEMO-516 Q3`.
    pub(crate) fn format_id(&self) -> String {
        format!("{} Q{}", self.key.as_str(), self.number)
    }

    /// Returns the key, Q number and date, such as
    /// `DEMO-516 Q3, asked 2026-11-25` or `DEMO-516 Q3, undated`.
    pub(crate) fn format_label(&self) -> String {
        match self.asked_on {
            Some(date) => format!("{}, asked {date}", self.format_id()),
            None => format!("{}, undated", self.format_id()),
        }
    }
}

/// Returns every Open Questions item on cards in `LISTED_STAGES`, from
/// one `card list` call. The oldest comes first, items asked on one
/// day go in key and number order, and undated items come last.
///
/// # Errors
///
/// The `card` failure, so no caller shows a partial list as all that
/// waits.
pub(crate) fn read_open_questions(cards: &dyn Cards) -> Result<Vec<OpenQuestion>, CardError> {
    let mut questions = Vec::new();
    for card in cards.list_cards()? {
        if !LISTED_STAGES.contains(&card.stage) {
            continue;
        }
        for item in card.open_questions {
            questions.push(OpenQuestion {
                key: card.key.clone(),
                project: card.project.clone(),
                number: item.number,
                title: item.text,
                lines: item.lines,
                sessions: card.sessions.clone(),
                asked_on: item.asked,
            });
        }
    }
    questions.sort_by_key(|question| {
        // `None` sorts before every date, so this puts undated items last.
        let undated = question.asked_on.is_none();
        (
            undated,
            question.asked_on,
            question.key.clone(),
            question.number,
        )
    });
    Ok(questions)
}

/// Returns each of the question's sessions where a person typed a
/// message on or after the day it was asked, with the newest such
/// message's time. The user may have answered there, so the hub reads
/// that pane before it asks again. Messages that start like the hub's
/// or the watch's, now or in older builds, are not a person's. The
/// item gives only the day, so a message typed earlier that day counts
/// too.
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
