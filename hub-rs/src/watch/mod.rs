//! Runs `hub watch`, one program per machine that runs outside every
//! chat and watches every live run on this machine, alongside each
//! hub's `hub next`, and `hub wait`, which a worker uses to wait on a
//! long job. Read `run_watch`, then `check_once`. The watch does only
//! what the hub never does. It prods a worker that stalled or whose
//! wait ended, and marks a blocked worker "needs you". A prod is a line
//! the watch has Herdr type into a worker's pane. A report that waits 10 minutes with no
//! `hub next` line goes back to the hub, as a line its next `hub next`
//! prints, and to the user's phone only when the hub is gone. The watch
//! never types into a hub's pane, so the user's own chat gets nothing
//! from it.
//!
//! Its parts that no other command uses live beside it. `events` keeps
//! its own kinds of line in the run's log, `wait` keeps workers' waits,
//! and `background`, `process` and `phone` start the watch, check a
//! PID and alert the phone.

pub mod background;
mod events;
mod phone;
mod process;
pub mod wait;

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, SystemTime};

use jiff::{SignedDuration, Timestamp, Zoned};

use crate::commands::Outcome;
use crate::external::cards::CardKey;
use crate::external::herdr::{Agent, AgentStatus, HerdrCode, HerdrError};
use crate::shared::events::{self as hub_events, Event, EventKind, Memory};
use crate::shared::lifecycle;
use crate::shared::messages::{self, Delivery, MessageCounts, Target};
use crate::shared::run_folder::{self, ReportedNames, RunFolder};
use crate::shared::{HubError, Tools};
use events::{WatchEvent, WatchMemory, append_watch_event, load_watch_memory};
use wait::WaitFile;

/// How long a worker that owes a report sits idle before its prod. An
/// ordinary pause between tool calls is far shorter.
const STALL_AFTER: SignedDuration = SignedDuration::from_mins(10);

/// How long a report waits with no `hub next` line before it counts as
/// missed.
const MISSED_AFTER: Duration = Duration::from_secs(10 * 60);

/// The oldest missed report that still sends a phone notification. Old
/// run folders keep unread reports from hubs closed long ago.
const PHONE_ALERT_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

/// The start of each run folder's name. The herdr-hub and auto-run
/// skills make run folders as `~/.local/state/herdr-hub.XXXXXX`.
const RUN_PREFIX: &str = "herdr-hub.";

/// The watch's folder in the state folder, which holds its lock and its
/// log.
const WATCH_FOLDER: &str = "hub";
const LOCK_FILE: &str = "watch.lock";
const LOG_FILE: &str = "watch.log";

/// Starts each prod, so a worker tells it from a person's message.
const PROD_PREFIX: &str = "hub watch: ";

/// The prod for a worker that went idle without a report.
const STALL_PROD: &str = "you have been idle 10 minutes without a report. \
    If a long job runs, register it with hub wait and end your turn. \
    If you are done or stuck, write your report with hub report. \
    Otherwise go on.";

/// How `hub watch run` checks, built by `main`.
pub struct WatchOptions {
    /// The wait between checks, from `HUB_POLL_SECONDS`.
    pub poll: Duration,
    /// `None` checks until the process is stopped. Tests give a count.
    pub checks: Option<usize>,
    /// The folder that holds the run folders, `~/.local/state`.
    pub runs_folder: PathBuf,
    /// The program that sends a phone notification, `moshi-hook`.
    pub notifier: PathBuf,
}

/// How `hub watch start` names its hub and starts the watch, built by
/// `main`.
pub struct WatchStart {
    /// This program, which runs in the background as `hub watch run`.
    pub program: PathBuf,
    /// The OpenCode session of the tool call that runs
    /// `hub watch start`, from `OPENCODE_SESSION_ID`, or `None` outside
    /// OpenCode.
    pub session_id: Option<String>,
    /// The folder that holds the run folders, `~/.local/state`.
    pub runs_folder: PathBuf,
}

/// When the watch first saw each idle worker in its present Herdr
/// state. Herdr raises an agent's `state_seq` each time its status
/// changes. The clock lives only in the watch's process, so a restarted watch
/// starts each clock again and prods up to 10 minutes later.
#[derive(Default)]
pub struct IdleClock {
    since: BTreeMap<String, (u64, Timestamp)>,
}

impl IdleClock {
    /// Starts the clock of each agent that became idle or done, and
    /// forgets each agent that works, is blocked or is gone.
    fn update(&mut self, agents: &[Agent], now: Timestamp) {
        let idle = agents.iter().filter(|agent| is_idle(agent)).map(|agent| {
            let since = match self.since.get(&agent.name) {
                Some((state_seq, since)) if *state_seq == agent.state_seq => *since,
                _ => now,
            };
            (agent.name.clone(), (agent.state_seq, since))
        });
        self.since = idle.collect();
    }

    /// Tells whether the agent has sat in its present idle state for
    /// `STALL_AFTER` by `now`.
    fn has_stalled(&self, agent: &Agent, now: Timestamp) -> bool {
        self.since
            .get(&agent.name)
            .is_some_and(|(state_seq, since)| {
                *state_seq == agent.state_seq && now.duration_since(*since) >= STALL_AFTER
            })
    }
}

/// A run whose hub Herdr lists now, for `hub board`.
pub(crate) struct LiveRun {
    pub run: RunFolder,
    /// The hub's agent name, such as `kanban-hub-3`.
    pub hub: String,
}

/// A run whose log names its hub.
struct WatchedRun {
    run: RunFolder,
    /// The hub's memory, for announced reports and delivered messages.
    memory: Memory,
    /// What the watch itself did in this run.
    watch: WatchMemory,
    /// The hub as Herdr lists it, or `None` when it is gone.
    hub: Option<Agent>,
}

/// What one check read from a run about its workers.
struct RunWorkers {
    reported: ReportedNames,
    /// Messages the hub queued in `sends/` until each worker can take
    /// them.
    held: BTreeMap<String, MessageCounts>,
    /// The names whose wait in this run stays open.
    open_waits: BTreeSet<String>,
}

impl RunWorkers {
    /// Tells whether NAME works for this run, because it reported in
    /// it, got a message through it or waits in it.
    fn claims(&self, memory: &Memory, name: &str) -> bool {
        self.reported.any.contains(name)
            || memory.last_delivered.contains_key(name)
            || self.open_waits.contains(name)
    }
}

/// Takes the watch lock, then runs `check_once` every `poll`. A second
/// watch, such as one systemd started while a `hub report` started
/// another, waits here and takes over if the first ends. Each failure
/// goes to `log` once while it lasts, and the next check runs, so a
/// Herdr or file failure never ends the watch.
///
/// # Errors
///
/// `HubError::File` when the lock file cannot be made or locked.
pub fn run_watch(
    tools: &Tools<'_>,
    options: &WatchOptions,
    log: &mut dyn Write,
) -> Result<Outcome, HubError> {
    let folder = tools.state_folder.join(WATCH_FOLDER);
    fs::create_dir_all(&folder).map_err(|source| HubError::File {
        path: folder.clone(),
        source,
    })?;
    let _lock = run_folder::take_file_lock(&folder.join(LOCK_FILE))?;
    let mut clock = IdleClock::default();
    let mut last_failures = Vec::new();
    let mut checks = 0;
    while options.checks.is_none_or(|limit| checks < limit) {
        let failures = match check_once(tools, options, &mut clock, &Zoned::now()) {
            Ok(failures) => failures,
            Err(error) => vec![error.to_string()],
        };
        // A lasting failure, such as Herdr being down, would otherwise
        // fill the log every few seconds.
        for failure in failures
            .iter()
            .filter(|failure| !last_failures.contains(*failure))
        {
            // A failed write to the log must not end the watch.
            let _ = writeln!(log, "hub watch: {failure}");
        }
        last_failures = failures;
        checks += 1;
        thread::sleep(options.poll);
    }
    Ok(Outcome { lines: Vec::new() })
}

/// Runs one check of every run whose log names a hub, at `now`. It
/// lists Herdr's agents once and reads each run, passing on missed
/// reports. In each run whose hub runs, it prods workers whose wait
/// ended. Then it checks each worker in the runs that claim it. It
/// marks a claimed blocked worker "needs you" and prods a claimed
/// stalled one, but only for a run whose hub runs. A run whose hub is
/// gone still claims its workers, so no other run prods them. A worker
/// that two runs claim gets at most one prod per Herdr state, and a
/// wait open in either run keeps it from counting as stalled. Returns
/// each failure, most of them led by their run folder, so one broken
/// run never stops the others.
///
/// # Errors
///
/// A failure to list Herdr's agents or the run folders, which ends this
/// check.
pub fn check_once(
    tools: &Tools<'_>,
    options: &WatchOptions,
    clock: &mut IdleClock,
    now: &Zoned,
) -> Result<Vec<String>, HubError> {
    // The watch cannot read another machine's files, so it leaves that
    // machine's sessions to its own watch.
    let agents: Vec<Agent> = tools
        .herdr
        .list_agents()?
        .into_iter()
        .filter(|agent| agent.machine.is_none())
        .collect();
    clock.update(&agents, now.timestamp());
    let mut failures = Vec::new();
    let runs = find_watched_runs(&options.runs_folder, &agents, &mut failures)?;
    // A run that could not be read may claim any agent.
    let mut every_run_read = failures.is_empty();
    // Each run's log keeps its own last prod of a worker, and a prod in
    // any run counts. Keeping one state number per worker would let an
    // older prod in one run hide a newer prod in another.
    let mut prodded: BTreeSet<(String, u64)> = runs
        .iter()
        .flat_map(|watched| watched.watch.prodded.clone())
        .collect();
    let mut read_runs = Vec::new();
    for watched in &runs {
        if let Err(error) = pass_on_missed_reports(options, watched, now) {
            failures.push(describe_run_failure(&watched.run, &error));
        }
        // A run whose hub is gone prods nobody, but its claims and open
        // waits still count.
        let read = if watched.hub.is_some() {
            prod_waits_and_read_run(tools, watched, &agents, &mut prodded, now, &mut failures)
        } else {
            read_run_claims(watched)
        };
        match read {
            Ok(workers) => read_runs.push((watched, workers)),
            Err(error) => {
                failures.push(describe_run_failure(&watched.run, &error));
                every_run_read = false;
            }
        }
    }
    let open_waits: BTreeSet<&str> = read_runs
        .iter()
        .flat_map(|(_, workers)| workers.open_waits.iter().map(String::as_str))
        .collect();
    // A hub that `hub watch start` named need not end in `-hub-N`.
    let hubs: BTreeSet<&str> = runs
        .iter()
        .filter_map(|watched| watched.watch.hub.as_deref())
        .collect();
    for agent in agents
        .iter()
        .filter(|agent| !lifecycle::is_hub_name(&agent.name) && !hubs.contains(agent.name.as_str()))
    {
        let mut claims: Vec<_> = read_runs
            .iter()
            .filter(|(watched, workers)| workers.claims(&watched.memory, &agent.name))
            .collect();
        // The herdr-hub skill names a card's worker after its card, and
        // a new worker may not have reported yet. With no run claiming
        // it, the first run whose hub runs checks it.
        if claims.is_empty() && every_run_read && CardKey::find_in_name(&agent.name).is_some() {
            claims.extend(read_runs.iter().find(|(watched, _)| watched.hub.is_some()));
        }
        if claims.is_empty() {
            continue;
        }
        if agent.status == AgentStatus::Blocked && !agent.marked {
            let marked = tools
                .herdr
                .mark_needs_you(&agent.pane_id, &[AgentStatus::Blocked]);
            if let Err(error) = marked {
                failures.push(HubError::from(error).to_string());
            }
        }
        if !clock.has_stalled(agent, now.timestamp()) || open_waits.contains(agent.name.as_str()) {
            continue;
        }
        for (watched, workers) in claims {
            if watched.hub.is_none() {
                continue;
            }
            if let Err(error) = prod_if_stalled(tools, watched, workers, agent, &mut prodded, now) {
                failures.push(describe_run_failure(&watched.run, &error));
            }
        }
    }
    Ok(failures)
}

/// Prods each worker whose wait in the run ended, and returns what the
/// run says about its workers. A wait that fails adds to `failures`.
fn prod_waits_and_read_run(
    tools: &Tools<'_>,
    watched: &WatchedRun,
    agents: &[Agent],
    prodded: &mut BTreeSet<(String, u64)>,
    now: &Zoned,
    failures: &mut Vec<String>,
) -> Result<RunWorkers, HubError> {
    let held = messages::count_messages(&watched.run)?;
    let open_waits = prod_ended_waits(tools, &watched.run, agents, &held, prodded, now, failures)?;
    let reported = watched.run.list_reported_names()?;
    Ok(RunWorkers {
        reported,
        held,
        open_waits,
    })
}

/// Returns what a run whose hub is gone says about its workers, with
/// every wait counted as open, and prods nobody.
fn read_run_claims(watched: &WatchedRun) -> Result<RunWorkers, HubError> {
    let open_waits = wait::list_waits(&watched.run)?
        .into_iter()
        .map(|wait| wait.name)
        .collect();
    Ok(RunWorkers {
        reported: watched.run.list_reported_names()?,
        held: messages::count_messages(&watched.run)?,
        open_waits,
    })
}

/// Prods a worker the caller found idle long enough, unless it was
/// prodded in this Herdr state, has an unread report or owes this run
/// no report. The prod is recorded in the run's event log after Herdr
/// took it.
fn prod_if_stalled(
    tools: &Tools<'_>,
    watched: &WatchedRun,
    workers: &RunWorkers,
    agent: &Agent,
    prodded: &mut BTreeSet<(String, u64)>,
    now: &Zoned,
) -> Result<(), HubError> {
    let should_prod = !prodded.contains(&(agent.name.clone(), agent.state_seq))
        && !workers.reported.unread.contains(&agent.name)
        && owes_report(&watched.run, &watched.memory, &agent.name)?;
    let held_count = count_held(&workers.held, &agent.name);
    if should_prod && prod_worker(tools, agent, held_count, STALL_PROD)? {
        record_prod(&watched.run, agent, "stalled", now)?;
        prodded.insert((agent.name.clone(), agent.state_seq));
    }
    Ok(())
}

/// Leads a failure with the run folder it came from, for the log.
fn describe_run_failure(run: &RunFolder, error: &HubError) -> String {
    format!("{}: {error}", run.path().display())
}

/// Removes each wait that ended, then prods its worker. Returns
/// the names whose wait stays open, including a wait whose prod has to
/// wait, as while the user is in the worker's pane. Such a wait ends
/// again at the next check. A wait that fails adds to `failures` and
/// stays open, and the run's other waits are still checked.
fn prod_ended_waits(
    tools: &Tools<'_>,
    run: &RunFolder,
    agents: &[Agent],
    held: &BTreeMap<String, MessageCounts>,
    prodded: &mut BTreeSet<(String, u64)>,
    now: &Zoned,
    failures: &mut Vec<String>,
) -> Result<BTreeSet<String>, HubError> {
    let mut open = BTreeSet::new();
    for wait in wait::list_waits(run)? {
        match prod_if_wait_ended(tools, run, agents, held, prodded, &wait, now) {
            Ok(false) => {}
            Ok(true) => {
                open.insert(wait.name);
            }
            Err(error) => {
                failures.push(describe_run_failure(run, &error));
                open.insert(wait.name);
            }
        }
    }
    Ok(open)
}

/// Removes the worker's wait when it ended, then prods the worker.
/// Returns whether the wait counts as open in this check.
fn prod_if_wait_ended(
    tools: &Tools<'_>,
    run: &RunFolder,
    agents: &[Agent],
    held: &BTreeMap<String, MessageCounts>,
    prodded: &mut BTreeSet<(String, u64)>,
    wait: &WaitFile,
    now: &Zoned,
) -> Result<bool, HubError> {
    let Some(why) = wait.find_end(now) else {
        return Ok(true);
    };
    let Some(agent) = agents.iter().find(|agent| agent.name == wait.name) else {
        // Herdr can leave a worker out of its list for a moment, as
        // after a restart, so its wait stays until its deadline.
        if now.timestamp() >= wait.deadline {
            wait::remove_wait_if_unchanged(run, wait)?;
        }
        return Ok(false);
    };
    let held_count = count_held(held, &agent.name);
    if !can_prod_now(agent, held_count) {
        return Ok(true);
    }
    // Removing first means a removal that keeps failing cannot prod the
    // worker at every check. The worker may have written a newer wait
    // since the list.
    wait::remove_wait_if_unchanged(run, wait)?;
    let text = format!("your wait ended, since {why}. Check the job and go on.");
    if prod_worker(tools, agent, held_count, &text)? {
        record_prod(run, agent, &format!("wait ended, {why}"), now)?;
        prodded.insert((agent.name.clone(), agent.state_seq));
    }
    Ok(false)
}

/// Returns how many messages the hub holds for NAME.
fn count_held(held: &BTreeMap<String, MessageCounts>, name: &str) -> usize {
    held.get(name).map_or(0, |counts| counts.held)
}

/// Tells whether a worker owes the hub a report. It does when it never
/// reported, or when a message reached it after its last report. A
/// worker that reported and got nothing since waits on the hub.
fn owes_report(run: &RunFolder, memory: &Memory, name: &str) -> Result<bool, HubError> {
    let Some(reported) = run.find_newest_report_time(name)? else {
        return Ok(true);
    };
    Ok(memory
        .last_delivered
        .get(name)
        .is_some_and(|delivered| SystemTime::from(*delivered) > reported))
}

/// Tells whether a held message would go to the worker now. It would
/// not while the user is in the worker's pane, or while the hub holds
/// a message for it.
fn can_prod_now(agent: &Agent, held_count: usize) -> bool {
    messages::choose_delivery(&Target::Live(agent.clone()), held_count) == Delivery::Now
}

/// Has Herdr type the prod into the worker's pane, only when a held
/// message would go now. So it never types while the user is in the
/// worker's pane, and never ahead of a message the hub holds for it.
/// Returns false when the prod waits for a later check.
fn prod_worker(
    tools: &Tools<'_>,
    agent: &Agent,
    held_count: usize,
    text: &str,
) -> Result<bool, HubError> {
    if !can_prod_now(agent, held_count) {
        return Ok(false);
    }
    match tools
        .herdr
        .prompt(&agent.name, &format!("{PROD_PREFIX}{text}"))
    {
        Ok(()) => Ok(true),
        // Herdr reports a stall or a timeout only after the text went
        // in. A stall can leave it pasted, and one Enter sends it.
        Err(error) if error.may_have_arrived() => {
            if let HerdrError::Refused {
                code: HerdrCode::AgentPromptStalled,
                ..
            } = error
            {
                messages::submit_pasted_text(tools.herdr, &agent.name);
            }
            Ok(true)
        }
        Err(error) => Err(error.into()),
    }
}

/// Records a prod of the worker in its present Herdr state.
fn record_prod(run: &RunFolder, agent: &Agent, why: &str, now: &Zoned) -> Result<(), HubError> {
    let prodded = WatchEvent::WorkerProdded {
        name: agent.name.clone(),
        state_seq: agent.state_seq,
        why: why.to_owned(),
    };
    append_watch_event(run, prodded, now)
}

/// Passes on each report that waited 10 minutes with no `hub next`
/// line, once per report version. While the run's hub runs, a
/// `report_missed` event makes its next `hub next` print
/// `missed report NAME`, and nothing types into the hub's pane. When
/// the hub is gone from Herdr, one phone notification tells the user
/// instead, for reports from the last day only. It is recorded only
/// once sent, so a refused one is tried again at the next check.
///
/// # Errors
///
/// A file failure, or the notifier's failure, which leaves the
/// notification unrecorded.
fn pass_on_missed_reports(
    options: &WatchOptions,
    watched: &WatchedRun,
    now: &Zoned,
) -> Result<(), HubError> {
    let (memory, watch) = (&watched.memory, &watched.watch);
    let late: Vec<_> = watched
        .run
        .list_reports(now)?
        .into_iter()
        .filter(|report| report.age >= MISSED_AFTER)
        .filter(|report| memory.announced.get(&report.name) != Some(&report.mtime_ns))
        .collect();
    if watched.hub.is_some() {
        for report in late
            .iter()
            .filter(|report| memory.missed.get(&report.name) != Some(&report.mtime_ns))
        {
            let missed = Event {
                at: now.timestamp(),
                kind: EventKind::ReportMissed {
                    name: report.name.clone(),
                    mtime_ns: report.mtime_ns,
                },
            };
            hub_events::append_event(&watched.run, &missed)?;
        }
        return Ok(());
    }
    let new: Vec<_> = late
        .iter()
        .filter(|report| report.age <= PHONE_ALERT_MAX_AGE)
        .filter(|report| watch.alerted.get(&report.name) != Some(&report.mtime_ns))
        .collect();
    if new.is_empty() {
        return Ok(());
    }
    let hub = watch.hub.as_deref().unwrap_or("the hub");
    let names: Vec<&str> = new.iter().map(|report| report.name.as_str()).collect();
    let title = format!("{hub} is gone");
    let message = format!(
        "Report {} waits in {} with no hub running.",
        names.join(", "),
        watched.run.path().display()
    );
    phone::send_phone_notification(&options.notifier, &title, &message)
        .map_err(HubError::NotDone)?;
    for report in &new {
        let alerted = WatchEvent::PhoneAlerted {
            name: report.name.clone(),
            mtime_ns: report.mtime_ns,
        };
        append_watch_event(&watched.run, alerted, now)?;
    }
    Ok(())
}

/// Returns each `herdr-hub.*` folder in `runs_folder` whose hub Herdr
/// lists in `agents`, in folder order, with a line for each folder whose
/// log cannot be read. `hub board` gathers what waits on the user from
/// these runs.
///
/// # Errors
///
/// `HubError::File` when `runs_folder` cannot be read.
pub(crate) fn find_live_runs(
    runs_folder: &Path,
    agents: &[Agent],
) -> Result<(Vec<LiveRun>, Vec<String>), HubError> {
    let mut failures = Vec::new();
    let runs = find_watched_runs(runs_folder, agents, &mut failures)?
        .into_iter()
        .filter_map(|watched| {
            let hub = watched.hub?.name;
            Some(LiveRun {
                run: watched.run,
                hub,
            })
        })
        .collect();
    Ok((runs, failures))
}

/// Returns the hub the run's log names, or `None` before
/// `hub watch start` named one.
///
/// # Errors
///
/// `HubError::File` when the log cannot be read.
pub(crate) fn read_hub_name(run: &RunFolder) -> Result<Option<String>, HubError> {
    Ok(load_watch_memory(run)?.hub)
}

/// Returns each `herdr-hub.*` folder in `runs_folder` whose event log
/// names a hub, in folder order, with that hub when Herdr lists it. A
/// folder whose log cannot be read is left out, with its failure added
/// to `failures`.
fn find_watched_runs(
    runs_folder: &Path,
    agents: &[Agent],
    failures: &mut Vec<String>,
) -> Result<Vec<WatchedRun>, HubError> {
    let file_error = |source| HubError::File {
        path: runs_folder.to_owned(),
        source,
    };
    let entries = match fs::read_dir(runs_folder) {
        Ok(entries) => entries,
        // No state folder means no hub ever ran here.
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => return Err(file_error(source)),
    };
    let mut runs = Vec::new();
    for entry in entries {
        let path = entry.map_err(file_error)?.path();
        if !is_run_folder_name(&path) {
            continue;
        }
        // A file with that name is not a run folder.
        let Ok(run) = RunFolder::open(&path) else {
            continue;
        };
        let read =
            load_watch_memory(&run).and_then(|watch| Ok((hub_events::load_memory(&run)?, watch)));
        let (memory, watch) = match read {
            Ok(read) => read,
            Err(error) => {
                failures.push(describe_run_failure(&run, &error));
                continue;
            }
        };
        let Some(hub_name) = watch.hub.as_deref() else {
            continue;
        };
        let hub = agents.iter().find(|agent| agent.name == hub_name).cloned();
        runs.push(WatchedRun {
            run,
            memory,
            watch,
            hub,
        });
    }
    runs.sort_by(|left, right| left.run.path().cmp(right.run.path()));
    Ok(runs)
}

/// Tells whether the path's last part starts with `herdr-hub.`.
fn is_run_folder_name(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|name| name.to_string_lossy().starts_with(RUN_PREFIX))
}

/// Tells whether Herdr shows the agent idle or done.
fn is_idle(agent: &Agent) -> bool {
    matches!(agent.status, AgentStatus::Idle | AgentStatus::Done)
}

/// Names the hub that runs this command as the run's hub, then starts
/// `hub watch run` in the background unless a watch already runs on
/// this machine. The name goes to `events.jsonl`, so the watch serves
/// this run from its next check.
///
/// # Errors
///
/// `HubError::NotDone` when the run folder is not one the watch reads,
/// when no hub agent is found, and when the watch cannot start.
/// Herdr's failures, and `HubError::File` when the event log cannot be
/// written.
pub fn run_watch_start(
    tools: &Tools<'_>,
    run: &RunFolder,
    start: &WatchStart,
    now: &Zoned,
) -> Result<Outcome, HubError> {
    let is_read =
        is_run_folder_name(run.path()) && run.path().parent() == Some(start.runs_folder.as_path());
    if !is_read {
        return Err(HubError::NotDone(format!(
            "hub watch reads only {}/{RUN_PREFIX}* folders",
            start.runs_folder.display()
        )));
    }
    let hub = find_hub_agent(tools, start.session_id.as_deref())?;
    let started = WatchEvent::WatchStarted {
        hub: hub.name.clone(),
    };
    append_watch_event(run, started, now)?;
    if is_watch_running(tools.state_folder)? {
        return Ok(Outcome::line(format!(
            "hub watch runs; it watches the workers of {}",
            hub.name
        )));
    }
    start_watch_process(tools.state_folder, &start.program)
        .map_err(|reason| HubError::NotDone(format!("hub watch run did not start: {reason}")))?;
    Ok(Outcome::line(format!(
        "hub watch started; it watches the workers of {}",
        hub.name
    )))
}

/// Starts the watch again when the run has a hub and no watch runs, as
/// after a crash. `hub report` and `hub wait` call it, so a worker's
/// next step brings a dead watch back. Returns the line to add to their
/// output, or `None` when nothing was started. A run with no hub, such
/// as a handoff folder or a test's folder, starts none.
pub fn restart_stopped_watch(
    run: &RunFolder,
    state_folder: &Path,
    program: &Path,
) -> Option<String> {
    let has_hub = load_watch_memory(run).is_ok_and(|watch| watch.hub.is_some());
    if !has_hub {
        return None;
    }
    let started = match is_watch_running(state_folder) {
        Ok(true) => return None,
        Ok(false) => start_watch_process(state_folder, program),
        Err(error) => Err(error.to_string()),
    };
    Some(match started {
        Ok(()) => "hub watch restarted".to_owned(),
        Err(reason) => format!("hub watch did not restart: {reason}"),
    })
}

/// Tells whether a watch holds this machine's watch lock now.
fn is_watch_running(state_folder: &Path) -> Result<bool, HubError> {
    run_folder::is_file_locked(&state_folder.join(WATCH_FOLDER).join(LOCK_FILE))
}

/// Starts `program` as `hub watch run` in the background, with its
/// output in the watch's log.
fn start_watch_process(state_folder: &Path, program: &Path) -> Result<(), String> {
    let folder = state_folder.join(WATCH_FOLDER);
    fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
    let args: [OsString; 2] = ["watch".into(), "run".into()];
    background::start_in_background(program, &args, &folder.join(LOG_FILE))
        .map(drop)
        .map_err(|error| error.to_string())
}

/// Returns the agent whose session is `session_id`, else the agent in
/// this pane. OpenCode's background service can give its tool commands
/// the `HERDR_PANE_ID` of another pane, but sets `OPENCODE_SESSION_ID`
/// for each tool call.
fn find_hub_agent(tools: &Tools<'_>, session_id: Option<&str>) -> Result<Agent, HubError> {
    if let Some(session_id) = session_id {
        let agents = tools.herdr.list_agents()?;
        let own = agents
            .into_iter()
            .find(|agent| agent.session_id.as_deref() == Some(session_id));
        if let Some(hub) = own {
            return Ok(hub);
        }
    }
    let Some(own_pane) = tools.own_pane else {
        return Err(HubError::NotDone(
            "run hub watch start in the hub's own Herdr pane".to_owned(),
        ));
    };
    let hub = tools.herdr.get_agent(own_pane)?;
    match (session_id, hub.session_id.as_deref()) {
        (Some(wanted), Some(found)) if wanted != found => Err(HubError::NotDone(format!(
            "HERDR_PANE_ID names {}, which runs another OpenCode session",
            hub.name
        ))),
        _ => Ok(hub),
    }
}
