//! Reads what the board shows, through the same steps as `hub status`
//! and `hub brief`, into plain values the screen draws. Read
//! `read_queue` first.

use std::collections::{BTreeMap, BTreeSet};

use jiff::tz::TimeZone;
use jiff::{Timestamp, Zoned};

use crate::commands::status::describe_why;
use crate::external::cards::{BoardCounts, CardKey, CardNote, Stage};
use crate::shared::decisions::{self, DecisionEntry, Urgency};
use crate::shared::lifecycle::{self, Lifecycle};
use crate::shared::run_folder::RunFolder;
use crate::shared::{HubError, Tools, events, facts};

/// Everything the board shows, read at one moment.
#[derive(Debug)]
pub(crate) struct Queue {
    /// When the read ran, in the user's time zone.
    pub read_at: Zoned,
    /// Sorted by state, then name.
    pub sessions: Vec<SessionLine>,
    /// Names with an unread report.
    pub unread: Vec<String>,
    /// Active and review cards, and draft or ready cards that a live
    /// session works on, in key order.
    pub cards: Vec<CardLine>,
    /// The `Project: title` line of each card a session or a question
    /// waiting now names, and of each card in `cards`, such as
    /// `Demo: Faster search`. A card with no title has none.
    pub titles: BTreeMap<CardKey, String>,
    /// The entries of `decisions.md`, oldest first.
    pub questions: Vec<Question>,
    /// The table rows of `reviews.md` that name a pull request, in the
    /// hub's merge order.
    pub pull_requests: Vec<PullRequest>,
    /// The other paragraphs and table rows of `reviews.md`.
    pub review_notes: Vec<String>,
    /// The `card board` counts, or `None` when the read failed.
    pub card_counts: Option<BoardCounts>,
    /// One line per source that could not be read.
    pub problems: Vec<String>,
}

/// One session and why it is in its state.
#[derive(Debug)]
pub(crate) struct SessionLine {
    pub name: String,
    pub state: Lifecycle,
    /// The same words as the WHY cell of `hub status`, such as
    /// `unread report`, or `-`.
    pub why: String,
    /// `None` when the session has no card or its card could not be
    /// read.
    pub card: Option<(CardKey, Stage)>,
}

/// One card and who works on it.
#[derive(Debug)]
pub(crate) struct CardLine {
    pub key: CardKey,
    pub stage: Stage,
    /// Empty when the card could not be read.
    pub project: String,
    /// Empty when the card could not be read.
    pub title: String,
    /// What the user does next when the card is in review, such as
    /// `review PR #498`.
    pub next_step: String,
    /// The live sessions named for the card.
    pub workers: Vec<String>,
}

/// One question in `decisions.md`.
#[derive(Debug)]
pub(crate) struct Question {
    pub title: String,
    /// Every line of the entry, for the details screen.
    pub lines: Vec<String>,
    pub urgency: Urgency,
    /// The card the entry names, when it names exactly one card on the
    /// board.
    pub card: Option<CardKey>,
}

/// One table row of `reviews.md`, such as `| #301 | uploads: retry |`.
#[derive(Debug)]
pub(crate) struct PullRequest {
    /// The first cell, such as `#301`.
    pub number: String,
    /// The second cell, or empty.
    pub what: String,
    /// Every cell, for the details screen.
    pub cells: Vec<String>,
}

/// Reads the sessions, cards, reports, questions and pull requests once.
/// Times print in `time_zone`.
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
    let memory = events::load_memory(run)?;
    let collected = facts::collect_session_facts(tools, run, &memory)?;
    let mut workers: BTreeMap<CardKey, Vec<String>> = BTreeMap::new();
    let mut sessions: Vec<SessionLine> = collected
        .sessions
        .into_iter()
        .map(|facts| {
            let verdict = lifecycle::decide_lifecycle(&facts);
            if let Some(card) = &facts.card
                && is_worker(verdict.state)
            {
                workers
                    .entry(card.key.clone())
                    .or_default()
                    .push(facts.name.clone());
            }
            SessionLine {
                why: describe_why(&facts, &verdict, time_zone),
                state: verdict.state,
                card: facts.card.map(|card| (card.key, card.stage)),
                name: facts.name,
            }
        })
        .collect();
    sessions.sort_by(|left, right| {
        (left.state.rank(), &left.name).cmp(&(right.state.rank(), &right.name))
    });
    let mut problems = collected.problems;
    let card_counts = match tools.cards.read_board() {
        Ok(board) => Some(board),
        Err(error) => {
            problems.push(format!("cards: {error}"));
            None
        }
    };
    let now = Timestamp::now().to_zoned(time_zone.clone());
    // `collect_session_facts` already reports a failed card list.
    let stages = tools.cards.list_stages().unwrap_or_default();
    let questions: Vec<Question> =
        decisions::read_decisions(&run.read_optional_file("decisions.md")?)
            .iter()
            .map(|entry| Question::from_entry(entry, &now, &stages))
            .collect();
    let reviews = split_reviews(&run.read_optional_file("reviews.md")?);
    let named = sessions
        .iter()
        .filter_map(|session| session.card.as_ref().map(|(key, _)| key))
        .chain(
            questions
                .iter()
                .filter(|question| question.urgency == Urgency::Now)
                .filter_map(|question| question.card.as_ref()),
        )
        .cloned()
        .collect();
    let (cards, titles) = read_cards(tools, &stages, workers, &named, &mut problems);
    Ok(Queue {
        read_at: now,
        sessions,
        unread: collected.unread,
        cards,
        titles,
        questions,
        pull_requests: reviews.pull_requests,
        review_notes: reviews.notes,
        card_counts,
        problems,
    })
}

/// Reads each card the board shows, and the title of each card in
/// `named`, one `card show` per card. A card that cannot be read still
/// shows by its key, and its error goes to `problems`.
fn read_cards(
    tools: &Tools<'_>,
    stages: &BTreeMap<CardKey, Stage>,
    mut workers: BTreeMap<CardKey, Vec<String>>,
    named: &BTreeSet<CardKey>,
    problems: &mut Vec<String>,
) -> (Vec<CardLine>, BTreeMap<CardKey, String>) {
    let mut cards = Vec::new();
    let mut titles = BTreeMap::new();
    for (key, &stage) in stages {
        let card_workers = workers.remove(key).unwrap_or_default();
        let is_shown = is_followed(stage, !card_workers.is_empty());
        if !is_shown && !named.contains(key) {
            continue;
        }
        let note = tools.cards.read_card(key).unwrap_or_else(|error| {
            problems.push(format!("cards: {error}"));
            CardNote::default()
        });
        if let Some(title) = format_card_title(&note.project, &note.title) {
            titles.insert(key.clone(), title);
        }
        if is_shown {
            cards.push(CardLine {
                next_step: next_step(&note),
                project: note.project,
                title: note.title,
                key: key.clone(),
                stage,
                workers: card_workers,
            });
        }
    }
    (cards, titles)
}

/// Returns a card's `Project: title` line, or the title alone when the
/// card has no project. A card with no title gets `None`.
pub(crate) fn format_card_title(project: &str, title: &str) -> Option<String> {
    match (project, title) {
        (_, "") => None,
        ("", title) => Some(title.to_owned()),
        (project, title) => Some(format!("{project}: {title}")),
    }
}

/// Returns true when a session in `state` counts as working on its
/// card. A parked session does not, nor one waiting to be closed or
/// parked, so the board never names either as a worker.
fn is_worker(state: Lifecycle) -> bool {
    match state {
        Lifecycle::Working
        | Lifecycle::WaitsOnHub
        | Lifecycle::WaitsOnUser
        | Lifecycle::Kept(_) => true,
        Lifecycle::ToClose | Lifecycle::ToPark | Lifecycle::Parked => false,
    }
}

/// Returns true when the board shows a card at `stage`. A draft or
/// ready card shows only while a session works on it.
fn is_followed(stage: Stage, has_worker: bool) -> bool {
    match stage {
        Stage::Active | Stage::Review => true,
        Stage::Draft | Stage::Ready => has_worker,
        Stage::Accepted | Stage::Dropped => false,
    }
}

/// Returns the card's newest `next:` Log line, else a step from its
/// newest PR link or first plan, else `open the card`. A link alone
/// cannot tell a merged PR from an open one, so the hub's `next:` line
/// wins.
fn next_step(note: &CardNote) -> String {
    if let Some(step) = &note.next_step {
        return step.clone();
    }
    if let Some(link) = note.prs.last() {
        let number = link.rsplit('/').next().unwrap_or(link);
        return format!("review PR #{number}");
    }
    if let Some(plan) = note.plans.first() {
        let plan_key = plan.split_whitespace().next().unwrap_or(plan);
        return format!("read plan {plan_key}");
    }
    "open the card".to_owned()
}

impl Question {
    /// Returns the entry with its urgency at `now` and its card.
    fn from_entry(
        entry: &DecisionEntry,
        now: &Zoned,
        stages: &BTreeMap<CardKey, Stage>,
    ) -> Question {
        Question {
            title: entry.title.clone(),
            lines: entry.lines.clone(),
            urgency: entry.urgency(now, stages),
            card: entry.find_card(stages).map(|(key, _)| key.clone()),
        }
    }
}

/// The two kinds of item in `reviews.md`.
#[derive(Default)]
struct Reviews {
    pull_requests: Vec<PullRequest>,
    notes: Vec<String>,
}

impl Reviews {
    /// Adds a table row, as a pull request when its first cell is a
    /// number such as `#301`, else as a note.
    fn add_row(&mut self, cells: Vec<String>) {
        let is_pull_request = cells.first().is_some_and(|cell| {
            cell.strip_prefix('#')
                .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
        });
        if is_pull_request {
            self.pull_requests.push(PullRequest {
                number: cells[0].clone(),
                what: cells.get(1).cloned().unwrap_or_default(),
                cells,
            });
        } else {
            self.notes.push(cells.join("  "));
        }
    }
}

/// Splits `reviews.md`, which is free prose, into pull requests and
/// notes. A table row whose first cell starts with `#` and a digit is a
/// pull request. Every other table row and every paragraph is a note,
/// with a paragraph's lines joined by spaces. Headings and table header
/// rows are left out.
fn split_reviews(text: &str) -> Reviews {
    let mut reviews = Reviews::default();
    let mut paragraph: Vec<&str> = Vec::new();
    // A table row waits for the next line, since a `| --- |` line under
    // it makes it the table's header.
    let mut held_row: Option<Vec<String>> = None;
    for line in text.lines().map(str::trim) {
        let cells = line.starts_with('|').then(|| table_cells(line));
        let is_separator = cells.as_ref().is_some_and(|cells| {
            !cells.is_empty()
                && cells
                    .iter()
                    .all(|cell| cell.chars().all(|c| c == '-' || c == ':'))
        });
        if let Some(row) = held_row.take()
            && !is_separator
        {
            reviews.add_row(row);
        }
        // A heading needs a space after its marks, so `#468 (tests)` is text.
        let is_heading = line.starts_with('#') && line.trim_start_matches('#').starts_with(' ');
        if !line.is_empty() && !is_heading && cells.is_none() {
            paragraph.push(line);
            continue;
        }
        if !paragraph.is_empty() {
            reviews.notes.push(paragraph.join(" "));
            paragraph.clear();
        }
        held_row = cells.filter(|cells| !cells.is_empty() && !is_separator);
    }
    if let Some(row) = held_row {
        reviews.add_row(row);
    }
    if !paragraph.is_empty() {
        reviews.notes.push(paragraph.join(" "));
    }
    reviews
}

/// Returns a table row's cells, trimmed, with empty cells left out.
fn table_cells(line: &str) -> Vec<String> {
    line.trim_matches('|')
        .split('|')
        .map(str::trim)
        .filter(|cell| !cell.is_empty())
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::shared::lifecycle::KeptReason;

    /// The board's review rows tell the user the one thing to do next.
    /// The hub's `next:` line must win over links, since a PR link stays
    /// on the card after the merge, and the user must always get a step
    /// to take. Of several PR links, the newest is the one to review.
    #[test]
    fn next_step_prefers_hub_line_then_links() {
        let plan = vec!["DEMO-12 Retry failed uploads".to_owned()];
        let pr = vec![
            "https://github.com/demo/app/pull/451".to_owned(),
            "https://github.com/demo/app/pull/498".to_owned(),
        ];
        let cases = [
            (
                "hub line",
                Some("accept the card"),
                pr.clone(),
                plan.clone(),
                "accept the card",
            ),
            ("pr", None, pr, plan.clone(), "review PR #498"),
            ("plan", None, Vec::new(), plan, "read plan DEMO-12"),
            ("nothing", None, Vec::new(), Vec::new(), "open the card"),
        ];

        for (case_name, next, prs, plans, expected) in cases {
            let note = CardNote {
                next_step: next.map(str::to_owned),
                prs,
                plans,
                ..CardNote::default()
            };
            assert_eq!(next_step(&note), expected, "{case_name}");
        }
    }

    /// WORKING ON NOW names who works on each card and marks a draft
    /// card someone started. A parked or finished session counted as a
    /// worker would tell the user work goes on when it stopped.
    #[test]
    fn is_worker_skips_parked_and_finished_sessions() {
        let cases = [
            ("working", Lifecycle::Working, true),
            ("waits on hub", Lifecycle::WaitsOnHub, true),
            ("kept", Lifecycle::Kept(KeptReason::UserInPane), true),
            ("parked", Lifecycle::Parked, false),
            ("to close", Lifecycle::ToClose, false),
            ("to park", Lifecycle::ToPark, false),
        ];

        for (case_name, state, expected) in cases {
            assert_eq!(is_worker(state), expected, "{case_name}");
        }
    }

    /// WORKING ON NOW answers which cards are worked on. A draft or
    /// ready card nobody works on would bury the real work under ideas,
    /// and a missing active or review card hides work the user expects.
    #[test]
    fn is_followed_picks_cards_by_stage_and_worker() {
        let cases = [
            ("active alone", Stage::Active, false, true),
            ("review alone", Stage::Review, false, true),
            ("draft alone", Stage::Draft, false, false),
            ("ready with worker", Stage::Ready, true, true),
            ("accepted with worker", Stage::Accepted, true, false),
            ("dropped with worker", Stage::Dropped, true, false),
        ];

        for (case_name, stage, has_worker, expected) in cases {
            assert_eq!(is_followed(stage, has_worker), expected, "{case_name}");
        }
    }

    /// `reviews.md` is prose the hub writes by hand. A note counted as
    /// a pull request inflates the count the user reads, and a table
    /// header or a paragraph cut where a line starts with `#471` puts
    /// items on the board that are not there. A status row the hub wrote
    /// must stay as a note, or it vanishes from the board.
    #[test]
    fn split_reviews_separates_pull_requests_from_notes() {
        let text = "\
# Pull requests waiting for the user's review

| PR | What it does | Worker |
| --- | --- | --- |

| #301 | Retry failed uploads | demo-kb-001-impl |
| merged | #299 | demo-kb-002-impl |

Merge in order 471, 468:
#471 (docs), #468 (tests).
";

        let reviews = split_reviews(text);

        let numbers: Vec<&str> = reviews
            .pull_requests
            .iter()
            .map(|pull_request| pull_request.number.as_str())
            .collect();
        assert_eq!(numbers, ["#301"]);
        assert_eq!(reviews.pull_requests[0].what, "Retry failed uploads");
        assert_eq!(
            reviews.notes,
            [
                "merged  #299  demo-kb-002-impl",
                "Merge in order 471, 468: #471 (docs), #468 (tests)."
            ]
        );
    }
}
