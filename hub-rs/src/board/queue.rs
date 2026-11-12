//! Reads what the board shows, through the same steps as `hub status`,
//! into plain values the screen draws. Read `read_queue` first.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use jiff::Timestamp;
use jiff::tz::TimeZone;

use crate::commands::status::describe_why;
use crate::external::cards::{CardKey, CardNote, Stage};
use crate::shared::decisions::{self, DecisionEntry};
use crate::shared::lifecycle::{self, Lifecycle};
use crate::shared::run_folder::RunFolder;
use crate::shared::{HubError, Tools, events, facts};

/// Everything the board shows, read at one moment.
#[derive(Debug)]
pub(crate) struct Queue {
    /// The time of the read, such as `10:42`.
    pub read_at: String,
    /// How long the read took.
    pub took: Duration,
    /// Draft, ready, active and review cards, in `card list` order.
    pub cards: Vec<CardLine>,
    /// The entries of `decisions.md`.
    pub questions: Vec<Question>,
    /// Sessions that wait on the user, by name.
    pub waiting: Vec<SessionLine>,
    /// What `card show` gave for each open card a session, a parked
    /// record or a report names.
    /// The details page reads any other card when it opens.
    pub notes: BTreeMap<CardKey, CardNote>,
    /// One line per source that could not be read.
    pub problems: Vec<String>,
}

/// One card and the sessions that work on it.
#[derive(Debug)]
pub(crate) struct CardLine {
    pub key: CardKey,
    pub stage: Stage,
    /// The title as `card list` prints it, maybe cut with `…`.
    pub title: String,
    /// The names of live sessions whose card this is.
    pub workers: Vec<String>,
}

/// One question in `decisions.md`.
#[derive(Debug)]
pub(crate) struct Question {
    pub title: String,
    /// Every line of the entry, for the details page.
    pub lines: Vec<String>,
}

/// One session that waits on the user, and why.
#[derive(Debug)]
pub(crate) struct SessionLine {
    pub name: String,
    /// The same words as the WHY cell of `hub status`, such as
    /// `needs you`.
    pub why: String,
    /// The card and its stage, such as `DEMO-KB-001 active`.
    pub card: Option<String>,
}

/// Reads the cards, sessions and questions once. Times print in
/// `time_zone`.
///
/// # Errors
///
/// `HubError::Herdr` when Herdr cannot list the agents, and
/// `HubError::File` when a run folder file cannot be read. A failed
/// `card` or OpenCode read becomes a problem line instead.
pub(crate) fn read_queue(
    tools: &Tools<'_>,
    run: &RunFolder,
    time_zone: &TimeZone,
) -> Result<Queue, HubError> {
    let started = Instant::now();
    let memory = events::load_memory(run)?;
    let collected = facts::collect_session_facts(tools, run, &memory)?;
    let mut workers: BTreeMap<CardKey, Vec<String>> = BTreeMap::new();
    let mut waiting = Vec::new();
    for facts in &collected.sessions {
        let verdict = lifecycle::decide_lifecycle(facts);
        if let Some(card) = &facts.card
            && is_worker(verdict.state)
        {
            workers
                .entry(card.key.clone())
                .or_default()
                .push(facts.name.clone());
        }
        if verdict.state == Lifecycle::WaitsOnUser {
            waiting.push(SessionLine {
                name: facts.name.clone(),
                why: describe_why(facts, &verdict, time_zone),
                card: facts
                    .card
                    .as_ref()
                    .map(|card| format!("{} {}", card.key.as_str(), card.stage.as_str())),
            });
        }
    }
    waiting.sort_by(|left, right| left.name.cmp(&right.name));
    let cards = collected
        .cards
        .into_iter()
        .filter(|card| {
            matches!(
                card.stage,
                Stage::Draft | Stage::Ready | Stage::Active | Stage::Review
            )
        })
        .map(|card| CardLine {
            workers: workers.remove(&card.key).unwrap_or_default(),
            key: card.key,
            stage: card.stage,
            title: card.title,
        })
        .collect();
    let questions = decisions::read_decisions(&run.read_optional_file("decisions.md")?)
        .iter()
        .map(Question::from_entry)
        .collect();
    Ok(Queue {
        read_at: Timestamp::now()
            .to_zoned(time_zone.clone())
            .strftime("%H:%M")
            .to_string(),
        took: started.elapsed(),
        cards,
        questions,
        waiting,
        notes: collected.notes,
        problems: collected.problems,
    })
}

impl Question {
    /// Returns the entry's title and every line, so the details page
    /// shows the choices and `Recommended:` the user decides from.
    fn from_entry(entry: &DecisionEntry) -> Question {
        Question {
            title: entry.title.clone(),
            lines: entry.lines.clone(),
        }
    }
}

/// Returns true when a session in `state` counts as working on its
/// card. A parked session does not, nor one waiting to be closed or
/// parked, so the details page never names either as a worker.
fn is_worker(state: Lifecycle) -> bool {
    match state {
        Lifecycle::Working
        | Lifecycle::WaitsOnHub
        | Lifecycle::WaitsOnUser
        | Lifecycle::Kept(_) => true,
        Lifecycle::ToClose | Lifecycle::ToPark | Lifecycle::Parked => false,
    }
}
