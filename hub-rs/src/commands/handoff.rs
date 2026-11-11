//! Runs `hub handoff`, which starts the next hub and checks its
//! effort, and `hub adopt`, which takes over a hub started some other
//! way. `start_due_review` starts the old hub's session review for
//! `hub next`. Read `run_handoff` first. A hub once told the user the
//! wrong effort because nothing checked it.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use jiff::Zoned;
use jiff::civil::Date;

use super::Outcome;
use crate::external::herdr::{Agent, AgentStart, Herdr, HerdrError, NewPlace};
use crate::shared::events::{self, Event, EventKind, Memory};
use crate::shared::lifecycle;
use crate::shared::run_folder::RunFolder;
use crate::shared::{AgentName, HubError, Tools};

/// How long the next hub has to give its first reply.
const REPLY_TIME_LIMIT: Duration = Duration::from_secs(120);

/// The effort a session review runs at, whatever the hub's own effort.
const REVIEW_EFFORT: &str = "high";

/// What `start_due_review` did, for the line `hub next` prints.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ReviewStart {
    Started {
        old: String,
        reviewer: String,
    },
    Failed {
        old: String,
        review: PathBuf,
        reason: String,
    },
}

/// Starts the next hub on the model `rules.md` names, with `HUB_RUN`
/// set, records the handoff, and reads back its first reply's effort.
/// Checks every input before it starts anything. Only then does the
/// next hub get its prompt, so it reads the whole handoff.
///
/// # Errors
///
/// `HubError::NotDone` for a missing input, before anything starts.
/// `HubError::NotDone` also for a failure after the start, worded
/// `started NAME, but ...` because the next hub already runs and a
/// rerun would start a second one.
pub fn run_handoff(
    tools: &Tools<'_>,
    run: &RunFolder,
    name: &AgentName,
    handoff: &Path,
    cwd: &Path,
    now: &Zoned,
    poll: Duration,
) -> Result<Outcome, HubError> {
    let Some(model) = run.read_rules_model()? else {
        let rules = run.path().join("rules.md");
        return Err(HubError::NotDone(format!(
            "{} has no --model '...' line",
            rules.display()
        )));
    };
    if !handoff.is_file() {
        return Err(HubError::NotDone(format!(
            "{} does not exist",
            handoff.display()
        )));
    }
    let Some(own_pane) = tools.own_pane else {
        return Err(HubError::NotDone(
            "run hub handoff in the hub's own pane".to_owned(),
        ));
    };
    let old = tools.herdr.get_agent(own_pane)?;
    let Some(old_session) = old.session_id.as_deref() else {
        return Err(HubError::NotDone(
            "Herdr does not know this hub's OpenCode session".to_owned(),
        ));
    };
    let handoff = std::path::absolute(handoff).map_err(|source| HubError::File {
        path: handoff.to_owned(),
        source,
    })?;

    let review = write_review_handoff(
        tools.state_folder,
        run,
        &old,
        old_session,
        name.as_str(),
        now.date(),
    )?;
    start_in_new_tab(
        tools.herdr,
        &old.workspace_id,
        name.as_str(),
        &model,
        run,
        cwd,
    )?;
    let started_but =
        |error: HubError| HubError::NotDone(format!("started {}, but {error}", name.as_str()));
    let handoff_event = Event {
        at: now.timestamp(),
        kind: EventKind::Handoff {
            old: old.name.clone(),
            tab: old.tab_id.clone(),
            new: name.as_str().to_owned(),
            review: Some(review.clone()),
        },
    };
    events::append_event(run, &handoff_event).map_err(started_but)?;
    append_old_hub_steps(&handoff, &old, &review).map_err(started_but)?;
    tools
        .herdr
        .prompt(name.as_str(), &make_handoff_prompt(&handoff))
        .map_err(|error| started_but(error.into()))?;
    let effort = wait_for_reply_effort(tools, name, poll).map_err(started_but)?;

    let expected = model
        .split_once('#')
        .map(|(_, effort)| effort)
        .filter(|effort| !effort.is_empty())
        .unwrap_or("default");
    let Some(effort) = effort else {
        return Err(HubError::NotDone(format!(
            "started {}, but it gave no reply in {} seconds, so its effort is unchecked",
            name.as_str(),
            REPLY_TIME_LIMIT.as_secs()
        )));
    };
    if effort != expected {
        return Err(HubError::NotDone(format!(
            "started {}, but it runs at {effort} effort, not {expected}",
            name.as_str()
        )));
    }
    Ok(Outcome {
        lines: vec![
            format!(
                "started {} on {model}; its first reply ran at {effort}",
                name.as_str()
            ),
            format!("review handoff {}", review.display()),
        ],
    })
}

/// Makes this hub take over `old`, an older hub of the run that no
/// `hub handoff` named. Writes the handoff for `old`'s session review
/// and records a `handoff` event, so `hub clean` closes `old` once it
/// is idle and `hub next` then starts the review. Changes nothing in
/// Herdr.
///
/// # Errors
///
/// `HubError::NotDone` outside the hub's own pane, when `old` is not
/// an older hub left without a handoff, or when Herdr does not know
/// `old`'s OpenCode session.
pub fn run_adopt(
    tools: &Tools<'_>,
    run: &RunFolder,
    old: &AgentName,
    now: &Zoned,
) -> Result<Outcome, HubError> {
    let Some(own_pane) = tools.own_pane else {
        return Err(HubError::NotDone(
            "run hub adopt in the hub's own pane".to_owned(),
        ));
    };
    let hub = tools.herdr.get_agent(own_pane)?;
    let memory = events::load_memory(run)?;
    let old_hub = tools.herdr.get_agent(old.as_str())?;
    if !lifecycle::is_unadopted_hub(&hub.name, &old_hub.name, memory.predecessor.as_deref()) {
        return Err(HubError::NotDone(format!(
            "{} is not an older hub left without hub handoff",
            old_hub.name
        )));
    }
    let Some(old_session) = old_hub.session_id.as_deref() else {
        return Err(HubError::NotDone(format!(
            "Herdr does not know the OpenCode session of {}",
            old_hub.name
        )));
    };

    let review = write_review_handoff(
        tools.state_folder,
        run,
        &old_hub,
        old_session,
        &hub.name,
        now.date(),
    )?;
    let adopted = Event {
        at: now.timestamp(),
        kind: EventKind::Handoff {
            old: old_hub.name.clone(),
            tab: old_hub.tab_id.clone(),
            new: hub.name.clone(),
            review: Some(review),
        },
    };
    events::append_event(run, &adopted)?;
    Ok(Outcome::line(format!(
        "adopted {}; hub clean closes its tab once it is idle, then hub next starts {}",
        old_hub.name,
        name_reviewer(&old_hub.name)
    )))
}

/// Starts the session review of an old hub once its tab has closed,
/// and returns what happened. Returns `None` when no review is due,
/// outside Herdr, and when the review already runs, as when the hub
/// started it by hand. Only that last case is recorded here. `hub next`
/// records a start or a failure when it prints the line, so a lost
/// line means one more try. `agents` are the live agents `hub next`
/// listed.
///
/// # Errors
///
/// `HubError::File` when the event log cannot be written. A failed
/// start is `ReviewStart::Failed`, not an error.
pub(crate) fn start_due_review(
    tools: &Tools<'_>,
    run: &RunFolder,
    memory: &Memory,
    agents: &[Agent],
    cwd: &Path,
    now: &Zoned,
) -> Result<Option<ReviewStart>, HubError> {
    let Some(own_pane) = tools.own_pane else {
        return Ok(None);
    };
    let is_live = |name: &str| agents.iter().any(|agent| agent.name == name);
    // The review's handoff tells the reviewer the old hub's tab is
    // closed, so the review waits for that.
    let Some((old, review)) = memory.reviews_due.iter().find(|(old, _)| !is_live(old)) else {
        return Ok(None);
    };
    let reviewer = name_reviewer(old);
    if is_live(&reviewer) {
        let started = Event {
            at: now.timestamp(),
            kind: EventKind::ReviewStarted {
                old: old.clone(),
                reviewer,
            },
        };
        events::append_event(run, &started)?;
        return Ok(None);
    }
    let start = match start_review(tools, run, own_pane, &reviewer, review, cwd) {
        Ok(()) => ReviewStart::Started {
            old: old.clone(),
            reviewer,
        },
        Err(error) => ReviewStart::Failed {
            old: old.clone(),
            review: review.clone(),
            reason: error.to_string(),
        },
    };
    Ok(Some(start))
}

/// Starts `reviewer` in a new tab of the hub's own workspace, on the
/// `rules.md` model at `REVIEW_EFFORT`, and gives it the `review`
/// handoff.
fn start_review(
    tools: &Tools<'_>,
    run: &RunFolder,
    own_pane: &str,
    reviewer: &str,
    review: &Path,
    cwd: &Path,
) -> Result<(), HubError> {
    let Some(model) = run.read_rules_model()? else {
        return Err(HubError::NotDone(
            "rules.md has no --model '...' line".to_owned(),
        ));
    };
    let base = model
        .split_once('#')
        .map_or(model.as_str(), |(base, _)| base);
    let model = format!("{base}#{REVIEW_EFFORT}");
    let hub = tools.herdr.get_agent(own_pane)?;
    start_in_new_tab(tools.herdr, &hub.workspace_id, reviewer, &model, run, cwd)?;
    tools.herdr.prompt(reviewer, &make_handoff_prompt(review))?;
    Ok(())
}

/// Returns the first prompt of a session started from `handoff`.
fn make_handoff_prompt(handoff: &Path) -> String {
    format!(
        "Read the handoff at {} and follow it. Load each skill it names when it says.",
        handoff.display()
    )
}

/// Writes the handoff for the session review of the old hub and
/// returns its path. Replaces a file of the same name.
fn write_review_handoff(
    state_folder: &Path,
    run: &RunFolder,
    old: &Agent,
    old_session: &str,
    new: &str,
    today: Date,
) -> Result<PathBuf, HubError> {
    let handoffs = state_folder.join("handoffs");
    let reviews = state_folder.join("session-reviews");
    let day = today.strftime("%Y%m%d").to_string();
    let reviewer = name_reviewer(&old.name);
    let previous = match find_newest_review(&reviews) {
        Some(path) => format!(
            r#"
The newest earlier review is `{path}`. Read it so you do not repeat
its proposals, and say when `{old}` repeated a mistake it named.
"#,
            path = path.display(),
            old = old.name,
        ),
        None => String::new(),
    };
    let text = format!(
        r#"# Handoff: session review of the hub {old}

You are a new session, agent `{reviewer}`, started by the hub
`{new}`. Review the hub session `{old}`, which ran the hub in Herdr
workspace `{workspace}` and handed off to `{new}` on {date}. Its
OpenCode session ID is `{session}`. Its tab is closed; do not look for
it in Herdr.

Load `session-review` now.

## The session to review

Follow `session-review` with `--session {session}`. The hub's run
folder `{run}` holds `notes.md` (a dated log) and `rules.md` (the
user's standing decisions).
{previous}
## Your work

Follow `session-review`: collect the session's numbers, give at most
three proposals, list lessons as a pick list, and say what local
models could do. Also judge the autonomous workflow in its own
section:

- what ran without the user
- what still needed the user, and whether it had to
- mistakes made without the user, and how the user would have caught
  them
- proposals that cut both the user's load and those mistakes

Save the review as `{review}`, have it checked by one fresh subagent
as the skill says, then report in the skill's output format.

## Coordination

- Read: the OpenCode database (read-only), the run folder, the
  handoffs folder `{handoffs}`, the session-reviews folder
  `{reviews}`, and skills (read only).
- Edit: only `{review}` and your report `{report}`.
- Never edit skills, lessons, memories, rules files, cards or the run
  folder; the user picks what to apply.
- Never read or repeat a card or session the hub's handoffs mark as
  private.
- Report only to the hub `{new}`: write your report (replace its old
  text), then end your turn. Put questions there too. Never prompt the
  hub. Never use an interactive question tool, since Herdr then
  rejects every message to you.
- When the hub says the work is done, close your own tab with
  `herdr tab close <tab>`, where `<tab>` is `.result.agent.tab_id`
  from `herdr agent get "$HERDR_PANE_ID"`.

## Rules

No emojis; never the words "canonical" or "contract". Never report
dollars. Never print secret values.
"#,
        old = old.name,
        new = new,
        workspace = old.workspace_id,
        date = today,
        session = old_session,
        run = run.path().display(),
        review = reviews.join(format!("{day}-{}.md", old.name)).display(),
        handoffs = handoffs.display(),
        reviews = reviews.display(),
        report = run.path().join(format!("report-{reviewer}.md")).display(),
    );
    let path = handoffs.join(format!("{day}-{}-review.md", old.name));
    fs::create_dir_all(&handoffs)
        .and_then(|()| fs::write(&path, text))
        .map_err(|source| HubError::File {
            path: path.clone(),
            source,
        })?;
    Ok(path)
}

/// Returns the most recently changed review in `reviews`, or `None`
/// when the folder holds none.
fn find_newest_review(reviews: &Path) -> Option<PathBuf> {
    let entries = fs::read_dir(reviews).ok()?;
    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "md"))
        .max_by_key(|path| {
            fs::metadata(path)
                .and_then(|metadata| metadata.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH)
        })
}

/// Returns `review-<old>`, cut to the 32 characters Herdr allows.
fn name_reviewer(old: &str) -> String {
    format!("review-{old}").chars().take(32).collect()
}

/// Adds to the next hub's handoff how the old hub's tab closes and its
/// session review starts.
fn append_old_hub_steps(handoff: &Path, old: &Agent, review: &Path) -> Result<(), HubError> {
    let steps = format!(
        r#"
## The old hub

`{old}` handed off to you. Once it is idle or done, `hub next` lists
it as to-close; run `hub clean` to close its tab. After that, `hub next`
starts its session review `{reviewer}` from `{review}`. Bring the
review's pick list to the user.
"#,
        old = old.name,
        reviewer = name_reviewer(&old.name),
        review = review.display(),
    );
    OpenOptions::new()
        .append(true)
        .open(handoff)
        .and_then(|mut file| file.write_all(steps.as_bytes()))
        .map_err(|source| HubError::File {
            path: handoff.to_owned(),
            source,
        })
}

/// Starts an OpenCode session in a new tab of the hub's workspace,
/// with `HUB_RUN` set in the pane, so the session's `hub` commands
/// need no `--run`.
fn start_in_new_tab(
    herdr: &dyn Herdr,
    workspace_id: &str,
    name: &str,
    model: &str,
    run: &RunFolder,
    cwd: &Path,
) -> Result<(), HerdrError> {
    let place = NewPlace {
        label: name.to_owned(),
        cwd: cwd.to_owned(),
        env: vec![("HUB_RUN".to_owned(), run.path().display().to_string())],
    };
    let tab = herdr.create_tab(workspace_id, &place)?;
    let start = AgentStart {
        name: name.to_owned(),
        pane_id: tab.root_pane_id,
        args: vec!["--model".to_owned(), model.to_owned(), "--auto".to_owned()],
    };
    herdr.start_agent(&start)
}

/// Waits up to two minutes for the session's first reply, and returns
/// its effort, or `None` when no reply came in time. Herdr learns the
/// session's OpenCode ID only after the session starts. So each `poll`
/// asks Herdr for the ID and the database for the reply again.
fn wait_for_reply_effort(
    tools: &Tools<'_>,
    name: &AgentName,
    poll: Duration,
) -> Result<Option<String>, HubError> {
    let deadline = Instant::now() + REPLY_TIME_LIMIT;
    loop {
        let agent = tools.herdr.get_agent(name.as_str())?;
        if let Some(session) = &agent.session_id
            && let Some(effort) = tools.opencode.read_first_reply_effort(session)?
        {
            return Ok(Some(effort));
        }
        if Instant::now() >= deadline {
            return Ok(None);
        }
        thread::sleep(poll);
    }
}
