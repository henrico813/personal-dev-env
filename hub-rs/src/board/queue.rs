//! Reads what the board shows, through the same steps as `hub status`,
//! into plain values the screen draws. Read `read_queue` first.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use jiff::Timestamp;
use jiff::tz::TimeZone;

use super::machine::{self, FeedLine, Gauges};
use crate::commands::status::describe_why;
use crate::external::cards::{CardKey, CardNote, Stage};
use crate::shared::decisions::{self, DecisionEntry};
use crate::shared::lifecycle::{self, Lifecycle};
use crate::shared::run_folder::RunFolder;
use crate::shared::{HubError, Tools, events, facts};
use crate::watch::{self, LiveRun};

/// The hubs whose questions and waiting sessions Needs you shows.
pub enum Hubs {
    /// Every hub Herdr lists now, found among the `herdr-hub.*` folders
    /// in this folder, `~/.local/state`.
    Live(PathBuf),
    /// Only the run in this folder, from the hidden `hub board --run`.
    Given(PathBuf),
}

/// What the Needs you group shows when no hub runs.
pub(crate) const NO_HUB: &str = "no hub running";

/// Everything the board shows, read at one moment.
#[derive(Debug)]
pub(crate) struct Queue {
    /// The time of the read, such as `10:42`.
    pub read_at: String,
    /// How long the read took.
    pub took: Duration,
    /// Draft, ready, active and review cards, in `card list` order.
    pub cards: Vec<CardLine>,
    /// The entries of each hub's `decisions.md`.
    pub questions: Vec<Question>,
    /// Sessions that wait on the user, by name.
    pub waiting: Vec<SessionLine>,
    /// Every session, live or parked, by name, for the sessions panel.
    pub sessions: Vec<SessionRow>,
    pub gauges: Gauges,
    /// The newest report files of the live runs, newest first.
    pub feed: Vec<FeedLine>,
    /// True when no hub ran at the read, so Needs you says `NO_HUB`.
    pub no_hub: bool,
    /// The run whose facts the board shows, where y in the clean dialog
    /// cleans, or `None` with no hub running.
    pub run: Option<PathBuf>,
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
    /// The folder the card is in, such as `DevEnv`.
    pub project: String,
    pub title: String,
    /// The names of live sessions whose card this is.
    pub workers: Vec<String>,
}

/// One question in a hub's `decisions.md`.
#[derive(Debug)]
pub(crate) struct Question {
    /// The hub that asks, such as `kanban-hub-3`.
    pub hub: String,
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

/// One session and the lifecycle verdict `hub clean` acts on.
#[derive(Debug)]
pub(crate) struct SessionRow {
    pub name: String,
    pub state: Lifecycle,
    /// For a kept session, the reason's label, such as `unpushed`.
    /// Otherwise the WHY cell of `hub status`, empty for its `-`.
    pub why: String,
    /// The stage of the session's card, when it has one.
    pub stage: Option<Stage>,
    /// The tokens in the session's context, when OpenCode gave them.
    pub context_tokens: Option<u64>,
}

impl SessionRow {
    /// Makes a row with `why` as `hub status` gives it. Its `-`, for
    /// nothing to say, becomes an empty why, so the sessions panel shows
    /// the state word instead.
    pub(crate) fn new(
        name: String,
        state: Lifecycle,
        why: String,
        stage: Option<Stage>,
        context_tokens: Option<u64>,
    ) -> SessionRow {
        let why = if why == "-" { String::new() } else { why };
        SessionRow {
            name,
            state,
            why,
            stage,
            context_tokens,
        }
    }
}

/// Reads the cards, sessions and questions once. Times print in
/// `time_zone`. With no hub running, the queue holds the cards alone.
///
/// # Errors
///
/// `HubError::Herdr` when Herdr cannot list the agents, and
/// `HubError::File` when the runs folder or a run folder file cannot be
/// read. A failed `card` or OpenCode read becomes a problem line
/// instead.
pub(crate) fn read_queue(
    tools: &Tools<'_>,
    hubs: &Hubs,
    time_zone: &TimeZone,
) -> Result<Queue, HubError> {
    let started = Instant::now();
    let mut problems = Vec::new();
    // The state folder sits in the home folder, so its disk is the one
    // that fills.
    let gauges = machine::read_gauges(tools.state_folder);
    let runs = match hubs {
        Hubs::Given(path) => {
            let run = RunFolder::open(path)?;
            let hub = watch::read_hub_name(&run)?.unwrap_or_else(|| "hub".to_owned());
            vec![LiveRun { run, hub }]
        }
        Hubs::Live(runs_folder) => {
            let (runs, failures) = watch::find_live_runs(runs_folder, &tools.herdr.list_agents()?)?;
            problems.extend(
                failures
                    .into_iter()
                    .map(|failure| format!("hubs: {failure}")),
            );
            runs
        }
    };
    let mut questions = Vec::new();
    for live in &runs {
        let entries = decisions::read_decisions(&live.run.read_optional_file("decisions.md")?);
        questions.extend(
            entries
                .iter()
                .map(|entry| Question::from_entry(entry, &live.hub)),
        );
    }
    let Some(LiveRun { run: first_run, .. }) = runs.first() else {
        let cards = tools.cards.list_cards().unwrap_or_else(|error| {
            problems.push(format!("cards: {error}"));
            Vec::new()
        });
        let cards = cards
            .into_iter()
            .filter(|card| is_on_board(card.stage))
            .map(|card| CardLine {
                key: card.key,
                stage: card.stage,
                project: card.project,
                title: card.title,
                workers: Vec::new(),
            });
        return Ok(Queue {
            read_at: format_read_time(time_zone),
            took: started.elapsed(),
            cards: cards.collect(),
            questions,
            waiting: Vec::new(),
            sessions: Vec::new(),
            gauges,
            feed: Vec::new(),
            no_hub: true,
            run: None,
            notes: BTreeMap::new(),
            problems,
        });
    };
    // Sessions come from Herdr for the whole machine, so one run's facts
    // cover every hub's workers. A worker reports only to its own hub,
    // so a report in any live run counts as reported.
    let memory = events::load_memory(first_run)?;
    let mut collected = facts::collect_session_facts(tools, first_run, &memory)?;
    let mut reported = BTreeSet::new();
    for live in &runs {
        reported.extend(live.run.list_reported_names()?.any);
    }
    for facts in &mut collected.sessions {
        facts.reports.ever_reported |= reported.contains(&facts.name);
    }
    problems.extend(collected.problems);
    let mut workers: BTreeMap<CardKey, Vec<String>> = BTreeMap::new();
    let mut waiting = Vec::new();
    let mut sessions = Vec::new();
    for facts in &collected.sessions {
        let verdict = lifecycle::decide_lifecycle(facts);
        sessions.push(SessionRow::new(
            facts.name.clone(),
            verdict.state,
            match verdict.state {
                Lifecycle::Kept(reason) => reason.label().to_owned(),
                _ => describe_why(facts, &verdict, time_zone),
            },
            facts.card.as_ref().map(|card| card.stage),
            facts.context_tokens,
        ));
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
    sessions.sort_by(|left, right| left.name.cmp(&right.name));
    let run_folders: Vec<&RunFolder> = runs.iter().map(|live| &live.run).collect();
    let feed = machine::read_feed(&run_folders, time_zone);
    let cards = collected
        .cards
        .into_iter()
        .filter(|card| is_on_board(card.stage))
        .map(|card| CardLine {
            workers: workers.remove(&card.key).unwrap_or_default(),
            key: card.key,
            stage: card.stage,
            project: card.project,
            title: card.title,
        })
        .collect();
    Ok(Queue {
        read_at: format_read_time(time_zone),
        took: started.elapsed(),
        cards,
        questions,
        waiting,
        sessions,
        gauges,
        feed,
        no_hub: false,
        run: Some(first_run.path().to_owned()),
        notes: collected.notes,
        problems,
    })
}

/// Tells whether the board shows cards in this stage.
fn is_on_board(stage: Stage) -> bool {
    matches!(
        stage,
        Stage::Draft | Stage::Ready | Stage::Active | Stage::Review
    )
}

/// Returns the time now in `time_zone`, such as `10:42`.
fn format_read_time(time_zone: &TimeZone) -> String {
    Timestamp::now()
        .to_zoned(time_zone.clone())
        .strftime("%H:%M")
        .to_string()
}

impl Question {
    /// Returns the entry's title and every line, so the details page
    /// shows the choices and `Recommended:` the user decides from, with
    /// the `hub` that asks.
    fn from_entry(entry: &DecisionEntry, hub: &str) -> Question {
        Question {
            hub: hub.to_owned(),
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
