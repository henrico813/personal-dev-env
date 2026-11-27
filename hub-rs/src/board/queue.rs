//! Reads what the board shows, through the same steps as `run_status`,
//! into plain values the screen draws. Read `read_queue` first.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{SignedDuration, Timestamp, Zoned};
use serde::{Deserialize, Serialize};

use super::away::{self, AwayEvent};
use super::machine::{self, Gauges};
use super::pull_requests::{PullRequestLine, read_pull_requests};
use super::seen;
use crate::commands::auto;
use crate::commands::status::describe_why;
use crate::external::cards::{BoardCounts, CardKey, CardNote, ListedCard, Stage};
use crate::external::herdr::Agent;
use crate::shared::lifecycle::{self, Lifecycle};
use crate::shared::questions::{OpenQuestion, read_open_questions};
use crate::shared::run_folder::RunFolder;
use crate::shared::{HubError, Tools, events, facts, format_span};
use crate::watch::{self, stall};

/// How long a worker counts as working after its status file last
/// changed, even while Herdr shows it idle.
const FRESH_STATUS: SignedDuration = SignedDuration::from_mins(30);

/// The hubs whose waiting sessions WAITING ON YOU shows.
pub enum Hubs {
    /// Every hub Herdr lists now, found among the `herdr-hub.*` folders
    /// in this folder, `~/.local/state`.
    Live(PathBuf),
    /// Only the run in this folder, from the board's hidden run option.
    Given(PathBuf),
}

/// What WAITING ON YOU shows when no hub runs.
pub(crate) const NO_HUB: &str = "no hub running";

const WATCH_STOPPED: &str = "hub watcher stopped";

/// Everything the board shows, read at one moment.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Queue {
    /// The time of the read, such as `10:42`.
    pub read_at: String,
    /// The day of the read, which question ages count up to.
    pub read_on: Date,
    /// How long the read took.
    pub took: Duration,
    /// The cards `is_on_board` keeps, in the order `Cards::list_cards`
    /// gives.
    pub cards: Vec<CardLine>,
    /// Every open question, oldest first, as `read_open_questions` gives them.
    pub questions: Vec<OpenQuestion>,
    /// Sessions that wait on the user, by name.
    pub waiting: Vec<SessionLine>,
    /// Every session, live or parked, by name, for the sessions panel.
    pub sessions: Vec<SessionRow>,
    pub gauges: Gauges,
    /// The reports and `notes.md` lines of the live runs, and of gone
    /// hubs' runs once a board saved a time, newest first.
    pub events: Vec<AwayEvent>,
    /// True when no hub ran at the read, so WAITING ON YOU says `NO_HUB`.
    pub no_hub: bool,
    /// The run whose facts the board shows, where y in the clean dialog
    /// cleans, or `None` with no hub running.
    pub run: Option<PathBuf>,
    /// What `Cards::read_cards` gave for each open card a session, a
    /// parked record or a report names.
    /// The details page reads any other card when it opens, as it does
    /// for every card of a saved read, which leaves these out.
    #[serde(skip)]
    pub notes: BTreeMap<CardKey, CardNote>,
    /// One line per source that could not be read.
    pub problems: Vec<String>,
    /// The open pull requests the `prs` tool last listed, or `None`
    /// without its list.
    pub pull_requests: Option<Vec<PullRequestLine>>,
    /// The active and in-review counts and limits `card board` gave, or
    /// `None` when it failed.
    pub board: Option<BoardCounts>,
}

/// One card and the sessions that work on it.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct CardLine {
    pub key: CardKey,
    pub stage: Stage,
    /// The folder the card is in, such as `DevEnv`.
    pub project: String,
    pub title: String,
    /// The names of live sessions whose card this is.
    pub workers: Vec<String>,
}

/// One session that waits on the user, and why.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct SessionLine {
    pub name: String,
    /// The words `describe_why` gives, such as `needs you`.
    pub why: String,
    /// The card and its stage, such as `DEMO-KB-001 active`.
    pub card: Option<String>,
}

/// One session and the lifecycle verdict the tidy pass acts on.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct SessionRow {
    pub name: String,
    pub state: Lifecycle,
    /// True when Herdr shows the session working, or its status file
    /// changed within `FRESH_STATUS`.
    pub working: bool,
    pub status: Option<WorkerStatus>,
    /// For a kept session, the reason's label, such as `unpushed`.
    /// Otherwise what `describe_why` gives, empty for its `-`.
    pub why: String,
    /// The stage of the session's card, when it has one.
    pub stage: Option<Stage>,
    /// The tokens in the session's context, when OpenCode gave them.
    pub context_tokens: Option<u64>,
}

impl SessionRow {
    /// Makes a row with `why` as `describe_why` gives it. Its `-`, for
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
            working: state == Lifecycle::Working,
            status: None,
            state,
            why,
            stage,
            context_tokens,
        }
    }
}

/// The first line of a worker's `status-<name>.md`, which says where its
/// card stands, and how long ago the file changed, such as `12m`.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct WorkerStatus {
    pub line: String,
    pub age: String,
}

/// Reads the cards, sessions and questions once. Times print in
/// `time_zone`. With no hub running, the queue holds the cards, their
/// open questions and, once a board saved a time, the SINCE YOU LEFT
/// events of hubs that are gone.
///
/// # Errors
///
/// `HubError::Herdr` when Herdr cannot list the agents, and
/// `HubError::File` when the runs folder or a run folder file cannot be
/// read. A failed `Cards` or OpenCode read becomes a problem line
/// instead.
pub(crate) fn read_queue(
    tools: &Tools<'_>,
    hubs: &Hubs,
    time_zone: &TimeZone,
) -> Result<Queue, HubError> {
    let started = Instant::now();
    let now = Timestamp::now().to_zoned(time_zone.clone());
    let mut problems = Vec::new();
    // The state folder sits in the home folder, so its disk is the one
    // that fills.
    let gauges = machine::read_gauges(tools.state_folder);
    let pull_requests = read_pull_requests(tools.pr_list);
    // A failed `card board` drops only the limits from the headings.
    let board = tools.cards.read_board().ok().map(|board| board.counts);
    let agents = tools.herdr.list_agents()?;
    let (runs, gone) = match hubs {
        Hubs::Given(path) => (vec![RunFolder::open(path)?], Vec::new()),
        Hubs::Live(runs_folder) => {
            let found = watch::find_hub_runs(runs_folder, &agents)?;
            problems.extend(
                found
                    .failures
                    .into_iter()
                    .map(|failure| format!("hubs: {failure}")),
            );
            (found.live, found.gone)
        }
    };
    let mut faults = Vec::new();
    // The hidden run option reads one run with no watch to report on.
    if matches!(hubs, Hubs::Live(_)) && !runs.is_empty() {
        match watch::is_watch_running(tools.state_folder) {
            Ok(true) => {}
            Ok(false) => faults.push(WATCH_STOPPED.to_owned()),
            Err(error) => problems.push(format!("watch: {error}")),
        }
    }
    let all_runs: Vec<&RunFolder> = runs.iter().chain(&gone).collect();
    // SINCE YOU LEFT also reads the runs of gone hubs, so what a hub did
    // before it ended while the user was away still shows. With no saved
    // time it shows the newest events, which would list hubs that ended
    // long ago, so gone runs wait until a board saves a time.
    let mut away_runs: Vec<&RunFolder> = runs.iter().collect();
    if seen::has_saved_time(tools.state_folder) {
        away_runs.extend(&gone);
    }
    let away_feed = machine::read_feed(&away_runs, time_zone);
    let away_events = away::read_events(&away_runs, &away_feed, time_zone)?;
    let questions = read_open_questions(tools.cards).unwrap_or_else(|error| {
        problems.push(format!("questions: {error}"));
        Vec::new()
    });
    let Some(first_run) = runs.first() else {
        let cards = tools.cards.list_cards().unwrap_or_else(|error| {
            problems.push(format!("cards: {error}"));
            Vec::new()
        });
        faults.extend(find_stalled_runs(
            &all_runs, &agents, &cards, &questions, &now,
        ));
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
            read_on: now.date(),
            took: started.elapsed(),
            cards: cards.collect(),
            questions,
            waiting: Vec::new(),
            sessions: Vec::new(),
            gauges,
            events: away_events,
            no_hub: true,
            run: None,
            notes: BTreeMap::new(),
            problems: [faults, problems].concat(),
            pull_requests,
            board,
        });
    };
    // Sessions come from Herdr for the whole machine, so one run's facts
    // cover every hub's workers. A worker reports only to its own hub,
    // so a report in any live run counts as reported.
    let memory = events::load_memory(first_run)?;
    let mut collected = facts::collect_session_facts(tools, first_run, &memory)?;
    let mut reported = BTreeSet::new();
    for run in &runs {
        reported.extend(run.list_reported_names()?.any);
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
        let mut row = SessionRow::new(
            facts.name.clone(),
            verdict.state,
            match verdict.state {
                Lifecycle::Kept(reason) => reason.label().to_owned(),
                _ => describe_why(facts, &verdict, time_zone),
            },
            facts.card.as_ref().map(|card| card.stage),
            facts.context_tokens,
        );
        if let Some((line, age)) = read_status_file(&runs, &facts.name, now.timestamp()) {
            row.working |= age < FRESH_STATUS;
            row.status = Some(WorkerStatus {
                line,
                age: format_span(age),
            });
        }
        sessions.push(row);
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
    faults.extend(find_stalled_runs(
        &all_runs,
        &agents,
        &collected.cards,
        &questions,
        &now,
    ));
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
        read_on: now.date(),
        took: started.elapsed(),
        cards,
        questions,
        waiting,
        sessions,
        gauges,
        events: away_events,
        no_hub: false,
        run: Some(first_run.path().to_owned()),
        notes: collected.notes,
        problems: [faults, problems].concat(),
        pull_requests,
        board,
    })
}

/// Returns a fault row for each run in `runs` that `stall` judges
/// stalled, as the watch's phone alert does, such as
/// `auto run 3 stalled since 12:52: DEMO-KB-001, not started (0 of 1
/// cards done)`. A run that cannot be read gives a `stall:` row.
fn find_stalled_runs(
    runs: &[&RunFolder],
    agents: &[Agent],
    cards: &[ListedCard],
    questions: &[OpenQuestion],
    now: &Zoned,
) -> Vec<String> {
    runs.iter()
        .filter_map(|run| {
            describe_run_stall(run, agents, cards, questions, now)
                .unwrap_or_else(|error| Some(format!("stall: {error}")))
        })
        .collect()
}

/// Returns the fault row of one run approved in the last
/// `STALL_ALERT_MAX_AGE`, or `None` while it makes progress or once it
/// has ended.
///
/// # Errors
///
/// `HubError::File` when a file of the run cannot be read.
fn describe_run_stall(
    run: &RunFolder,
    agents: &[Agent],
    cards: &[ListedCard],
    questions: &[OpenQuestion],
    now: &Zoned,
) -> Result<Option<String>, HubError> {
    let Some(approved_text) = run.read_file_if_exists("approved.md")? else {
        return Ok(None);
    };
    let Some(approved) = stall::read_approval_time(&approved_text, now) else {
        return Ok(None);
    };
    if now.timestamp().duration_since(approved.timestamp()) > watch::STALL_ALERT_MAX_AGE {
        return Ok(None);
    }
    let list = auto::read_run_list(&approved_text);
    let memory = events::load_memory(run)?;
    let quiet = stall::find_quiet_run(
        run,
        list.number,
        list.run_keys(),
        approved.timestamp(),
        agents,
        &memory,
        now,
    )?;
    let Some(quiet) = quiet else {
        return Ok(None);
    };
    let stall::Verdict::Stalled(stall) = stall::judge_quiet_run(&quiet, cards, questions) else {
        return Ok(None);
    };
    let name = list.number.map_or_else(
        || "an auto run".to_owned(),
        |number| format!("auto run {number}"),
    );
    Ok(Some(format!("{name} stalled {}", stall.describe(now))))
}

/// Tells whether the board shows cards in this stage.
fn is_on_board(stage: Stage) -> bool {
    matches!(
        stage,
        Stage::Draft | Stage::Todo | Stage::Active | Stage::InReview | Stage::Blocked
    )
}

/// Returns the first line of the newest `status-<name>.md` in `runs`,
/// and how long before `now` it changed. A file that cannot be read
/// counts as missing.
fn read_status_file(
    runs: &[RunFolder],
    name: &str,
    now: Timestamp,
) -> Option<(String, SignedDuration)> {
    let file_name = format!("status-{name}.md");
    let (modified, path) = runs
        .iter()
        .filter_map(|run| {
            let path = run.path().join(&file_name);
            let modified = path.metadata().and_then(|metadata| metadata.modified());
            Some((modified.ok()?, path))
        })
        .max()?;
    let text = fs::read_to_string(path).ok()?;
    let line = text.lines().next().unwrap_or_default().trim().to_owned();
    let modified = Timestamp::try_from(modified).ok()?;
    Some((line, now.duration_since(modified)))
}

/// Returns the time now in `time_zone`, such as `10:42`.
fn format_read_time(time_zone: &TimeZone) -> String {
    Timestamp::now()
        .to_zoned(time_zone.clone())
        .strftime("%H:%M")
        .to_string()
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
