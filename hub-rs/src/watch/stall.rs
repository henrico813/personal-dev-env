//! Tells whether an approved auto run has stalled. It has when no
//! card's worker makes progress for 15 minutes while some card is not
//! done. `hub auto now` prints the stall and `hub watch` sends it to
//! the phone. Read `describe_stall_or_end`, then `find_quiet_run` and
//! `judge_quiet_run`.
//!
//! The first step reads only Herdr and the run folder, so the watch
//! can run it at every check. The second reads the cards, which is
//! slow, so the watch runs it only when an alert may be due.

use std::time::SystemTime;

use jiff::civil::DateTime;
use jiff::{SignedDuration, Timestamp, Zoned};

use super::wait;
use crate::external::cards::{CardKey, ListedCard, Stage};
use crate::external::herdr::{Agent, AgentStatus};
use crate::shared::events::{self, Memory};
use crate::shared::lifecycle;
use crate::shared::questions::OpenQuestion;
use crate::shared::run_folder::RunFolder;
use crate::shared::{HubError, Tools};

/// How long a run goes with no news from its cards before it counts as
/// quiet.
const QUIET_AFTER: SignedDuration = SignedDuration::from_mins(15);

/// The width of the user's phone terminal.
const PHONE_COLUMNS: usize = 40;

/// What Herdr and the run folder say of one card of the run.
pub(crate) struct CardFacts {
    pub key: CardKey,
    /// Its worker wrote `status-<key>-auto.md`.
    pub has_status: bool,
    /// The Herdr state of each live agent named after the card.
    pub live: Vec<AgentStatus>,
}

/// A run in which no worker works, waits on a job or sent news for 15
/// minutes.
pub(crate) struct QuietRun {
    /// The newest news from the run's cards, or the approval.
    pub since: Timestamp,
    pub cards: Vec<CardFacts>,
    /// The auto hub wrote `report-auto-hub-<N>.md`, which it does for
    /// its end brief.
    pub end_brief_sent: bool,
}

/// Where a quiet run stands once its cards are read.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Verdict {
    /// The run ended, for the reason given, such as `every card
    /// stopped at a pull request`.
    Ended(&'static str),
    Stalled(Stall),
}

/// A stalled run, with why each card that is not done waits.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Stall {
    pub since: Timestamp,
    /// Each card that is not done, with its reason, such as
    /// `not started`.
    pub waiting: Vec<(CardKey, String)>,
    pub done: usize,
    pub total: usize,
}

impl Stall {
    /// Returns the stall as one line, such as `since 12:52: DEMO-KB-002,
    /// not started (1 of 2 cards done)`, with the time in `now`'s zone.
    pub(crate) fn describe(&self, now: &Zoned) -> String {
        let since = self.since.to_zoned(now.time_zone().clone());
        let reasons: Vec<String> = self
            .waiting
            .iter()
            .map(|(key, reason)| format!("{}, {reason}", key.as_str()))
            .collect();
        format!(
            "since {}: {} ({} of {} cards done)",
            since.strftime("%H:%M"),
            reasons.join("; "),
            self.done,
            self.total
        )
    }
}

/// Returns the lines `hub auto now` ends with, `STALLED since ...` for
/// a stalled run or `Run ended ...` for one whose cards all stopped at
/// a pull request or whose auto hub sent its end brief. They break at
/// spaces to fit 40 columns, the width of the user's phone, so a card
/// key never splits. Returns no lines while the run makes progress, or
/// for an `approved.md` with no `Approved:` line.
///
/// # Errors
///
/// A Herdr, `card` or file failure.
pub(crate) fn describe_stall_or_end(
    tools: &Tools<'_>,
    run: &RunFolder,
    approved_text: &str,
    number: Option<u32>,
    keys: &[CardKey],
    questions: &[OpenQuestion],
    now: &Zoned,
) -> Result<Vec<String>, HubError> {
    let Some(approved) = read_approval_time(approved_text, now) else {
        return Ok(Vec::new());
    };
    let agents = tools.herdr.list_agents()?;
    let memory = events::load_memory(run)?;
    let Some(quiet) = find_quiet_run(
        run,
        number,
        keys,
        approved.timestamp(),
        &agents,
        &memory,
        now,
    )?
    else {
        return Ok(Vec::new());
    };
    let line = match judge_quiet_run(&quiet, &tools.cards.list_cards()?, questions) {
        Verdict::Ended(reason) => format!("Run ended: {reason}"),
        Verdict::Stalled(stall) => format!("STALLED {}", stall.describe(now)),
    };
    Ok(wrap_at_spaces(&line, PHONE_COLUMNS))
}

/// Returns the time on the `Approved: 2026-11-20 12:30` line that
/// `hub auto approve` writes, in `now`'s time zone, as it wrote it.
pub(crate) fn read_approval_time(approved_text: &str, now: &Zoned) -> Option<Zoned> {
    let text = approved_text
        .lines()
        .find_map(|line| line.strip_prefix("Approved: "))?;
    let time = DateTime::strptime("%Y-%m-%d %H:%M", text.trim()).ok()?;
    time.to_zoned(now.time_zone().clone()).ok()
}

/// Returns the run as quiet when neither its auto hub `auto-hub-<N>`,
/// N being NUMBER, nor any of its cards' workers works in Herdr, no
/// live agent waits on a job with `hub wait`, and no news
/// came from the cards in the last 15 minutes. News is a status file
/// change, a report or a message the hub delivered, and the approval
/// counts as news. Returns `None` while the run makes progress.
///
/// # Errors
///
/// `HubError::File` when the run folder cannot be read.
pub(crate) fn find_quiet_run(
    run: &RunFolder,
    number: Option<u32>,
    keys: &[CardKey],
    approved: Timestamp,
    agents: &[Agent],
    memory: &Memory,
    now: &Zoned,
) -> Result<Option<QuietRun>, HubError> {
    let card_agents: Vec<(&CardKey, &Agent)> = agents
        .iter()
        .filter(|agent| agent.machine.is_none() && !lifecycle::is_hub_name(&agent.name))
        .filter_map(|agent| {
            let found = CardKey::find_in_name(&agent.name)?;
            keys.iter()
                .find(|key| **key == found)
                .map(|key| (key, agent))
        })
        .collect();
    // The auto hub works alone at the start, moving cards and working
    // out stacks before it starts any worker.
    let auto_hub = number.map(|number| format!("auto-hub-{number}"));
    let hub_works = agents.iter().any(|agent| {
        Some(&agent.name) == auto_hub.as_ref() && agent.status == AgentStatus::Working
    });
    if hub_works
        || card_agents
            .iter()
            .any(|(_, agent)| agent.status == AgentStatus::Working)
    {
        return Ok(None);
    }
    // A wait whose agent Herdr no longer lists, as for a parked worker,
    // ends no job anyone watches.
    let waiting = wait::list_waits(run)?.into_iter().any(|wait| {
        wait.deadline > now.timestamp() && agents.iter().any(|agent| agent.name == wait.name)
    });
    if waiting {
        return Ok(None);
    }
    let mut since = approved;
    let mut cards = Vec::new();
    for key in keys {
        let status = run
            .path()
            .join(format!("status-{}-auto.md", key.as_str().to_lowercase()));
        let modified = status.metadata().and_then(|metadata| metadata.modified());
        if let Ok(modified) = modified {
            since = since.max(to_timestamp(modified));
        }
        cards.push(CardFacts {
            key: key.clone(),
            has_status: modified.is_ok(),
            live: card_agents
                .iter()
                .filter(|(card, _)| *card == key)
                .map(|(_, agent)| agent.status)
                .collect(),
        });
    }
    let names = run.list_reported_names()?.any;
    for name in names.iter().filter(|name| names_run_card(name, keys)) {
        if let Some(reported) = run.find_newest_report_time(name)? {
            since = since.max(to_timestamp(reported));
        }
    }
    for (name, delivered) in &memory.last_delivered {
        if names_run_card(name, keys) {
            since = since.max(*delivered);
        }
    }
    if now.timestamp().duration_since(since) < QUIET_AFTER {
        return Ok(None);
    }
    let end_brief_sent = match &auto_hub {
        Some(name) => run.find_newest_report_time(name)?.is_some(),
        None => false,
    };
    Ok(Some(QuietRun {
        since,
        cards,
        end_brief_sent,
    }))
}

/// Judges a quiet run from its cards' stages and open questions. A
/// card is done when its worker wrote a status file, its card is in
/// review or done, and it asks no question. A run whose auto hub sent
/// its end brief, or with every card done, has ended.
pub(crate) fn judge_quiet_run(
    quiet: &QuietRun,
    stages: &[ListedCard],
    questions: &[OpenQuestion],
) -> Verdict {
    // After the end brief, cards parked on a question or waiting on a
    // parent have stopped as the auto-run skill means, not stalled.
    if quiet.end_brief_sent {
        return Verdict::Ended("the auto hub sent its end brief");
    }
    let mut waiting = Vec::new();
    for card in &quiet.cards {
        let question = questions.iter().find(|question| question.key == card.key);
        let stage = stages
            .iter()
            .find(|listed| listed.key == card.key)
            .map(|listed| listed.stage);
        let reason = if let Some(question) = question {
            format!("waits on your question: {}", question.title)
        } else if !card.has_status && card.live.is_empty() {
            "not started".to_owned()
        } else if card.has_status && matches!(stage, Some(Stage::InReview | Stage::Done)) {
            continue;
        } else if card.live.contains(&AgentStatus::Blocked) {
            "waits on a prompt in its pane".to_owned()
        } else if card.live.is_empty() {
            "has no worker".to_owned()
        } else {
            "has an idle worker".to_owned()
        };
        waiting.push((card.key.clone(), reason));
    }
    if waiting.is_empty() {
        return Verdict::Ended("every card stopped at a pull request");
    }
    Verdict::Stalled(Stall {
        since: quiet.since,
        done: quiet.cards.len() - waiting.len(),
        total: quiet.cards.len(),
        waiting,
    })
}

/// Tells whether NAME, such as `demo-kb-002-auto-2`, names a card of
/// the run.
fn names_run_card(name: &str, keys: &[CardKey]) -> bool {
    CardKey::find_in_name(name).is_some_and(|found| keys.contains(&found))
}

/// Converts a file time to a timestamp. A time jiff cannot hold, far
/// outside any real file time, reads as 1970.
fn to_timestamp(time: SystemTime) -> Timestamp {
    Timestamp::try_from(time).unwrap_or(Timestamp::UNIX_EPOCH)
}

/// Splits TEXT at spaces into lines of at most WIDTH characters. A word
/// longer than WIDTH gets a line of its own.
fn wrap_at_spaces(text: &str, width: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for word in text.split(' ') {
        match lines.last_mut() {
            Some(line) if line.chars().count() + 1 + word.chars().count() <= width => {
                line.push(' ');
                line.push_str(word);
            }
            _ => lines.push(word.to_owned()),
        }
    }
    lines
}
