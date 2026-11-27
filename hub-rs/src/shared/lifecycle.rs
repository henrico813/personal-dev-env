//! Works out one session's lifecycle state from its facts, with no
//! external calls. Read `decide_lifecycle` first. Its rules apply in
//! order and the first match wins, so a blocked session is `flagged`
//! even when its card's work is over. The later tidy pass closes or
//! parks a session only in the `to-close` or `to-park` state, so each
//! rule that gives one of those decides what the hub may close.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::messages::MessageCounts;
use super::parking::ParkedRecord;
use crate::external::cards::{CardKey, Stage};
use crate::external::git::Unsaved;
use crate::external::herdr::{Agent, AgentKind, AgentStatus};

/// The review session that `run_handoff` creates for the old hub gets
/// a name starting with this, so `review-kanban-hub-16` is not a hub.
const REVIEW_PREFIX: &str = "review-";
/// The word before `-hub-` in an auto run's hub, as in `auto-hub-13`.
const AUTO_HUB_WORD: &str = "auto";
/// The word before `-hub-` in the name of each hub the user works the
/// kanban board with.
const KANBAN_HUB_WORD: &str = "kanban";

/// One session's place in its life. Each `match` on it must name
/// every state. Code that forgets to handle a new state then fails to
/// build.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Lifecycle {
    Working,
    WaitsOnHub,
    WaitsOnUser,
    Kept(KeptReason),
    ToClose,
    ToPark,
    Parked,
}

impl Lifecycle {
    /// Returns the word `run_status` shows, such as `hub` or `to-park`.
    pub(crate) fn word(self) -> &'static str {
        match self {
            Lifecycle::Working => "working",
            Lifecycle::WaitsOnHub => "hub",
            Lifecycle::WaitsOnUser => "flagged",
            Lifecycle::Kept(_) => "kept",
            Lifecycle::ToClose => "to-close",
            Lifecycle::ToPark => "to-park",
            Lifecycle::Parked => "parked",
        }
    }

    /// Returns the state's place in the status table and counts line,
    /// working first and parked last.
    pub(crate) fn rank(self) -> u8 {
        match self {
            Lifecycle::Working => 0,
            Lifecycle::WaitsOnHub => 1,
            Lifecycle::WaitsOnUser => 2,
            Lifecycle::Kept(_) => 3,
            Lifecycle::ToClose => 4,
            Lifecycle::ToPark => 5,
            Lifecycle::Parked => 6,
        }
    }
}

/// Why a finished session stays open.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum KeptReason {
    UnsavedFiles,
    Unpushed,
    GitFailed,
    UserInPane,
    ApprovalWaits,
    RecentRequest,
    CannotReopen,
    AnotherHub,
    OtherMachine,
    Yours,
    CardUnknown,
    WaitsOnJob,
}

impl KeptReason {
    /// Returns the short words `run_status` shows, such as
    /// `unsaved files`.
    pub(crate) fn label(self) -> &'static str {
        match self {
            KeptReason::UnsavedFiles => "unsaved files",
            KeptReason::Unpushed => "unpushed",
            KeptReason::GitFailed => "git failed",
            KeptReason::UserInPane => "user in pane",
            KeptReason::ApprovalWaits => "approval waits",
            KeptReason::RecentRequest => "request in the last hour",
            KeptReason::CannotReopen => "cannot reopen",
            KeptReason::AnotherHub => "another hub",
            KeptReason::OtherMachine => "other machine",
            KeptReason::Yours => "yours",
            KeptReason::CardUnknown => "card unknown",
            KeptReason::WaitsOnJob => "waits on a job",
        }
    }
}

/// A session's state, and the short reason a person reads beside it,
/// such as `blocked` or the worktree that holds unsaved files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Verdict {
    pub state: Lifecycle,
    pub why: String,
}

impl Verdict {
    /// Returns the why, or the state's word when the why is empty, such
    /// as `working`.
    pub(crate) fn pick_why_or_state(&self) -> &str {
        if self.why.is_empty() {
            self.state.word()
        } else {
            &self.why
        }
    }
}

/// Everything `decide_lifecycle` needs about one session.
#[derive(Clone, Debug)]
pub(crate) struct SessionFacts {
    pub name: String,
    /// `None` when the session is parked and has no tab.
    pub agent: Option<Agent>,
    pub parked: Option<ParkedRecord>,
    pub card: Option<CardFacts>,
    /// The card cannot be read, because a `Cards` call failed or the
    /// board has no stage for the key in the session's name.
    pub card_unknown: bool,
    pub reports: ReportFacts,
    pub messages: MessageCounts,
    pub unsaved: Unsaved,
    /// True when a pending, approved or running approval request came
    /// from this session's pane or session ID. Such a request
    /// asks the user to approve a pull request change. A request file
    /// that does not parse sets it for every live session.
    pub approval_pending: bool,
    /// True when this session's pane or session ID filed a
    /// approval request that is no longer open and changed in the
    /// last hour. A worker that just merged one pull request may still
    /// have more to do.
    pub filed_request_recently: bool,
    /// The run folder holds a wait record of this session that the watch
    /// has not ended, as while the worker waits on CI before a merge.
    pub wait_open: bool,
    /// A `handoff` event of this run named the session as the old hub.
    pub handed_off: bool,
    pub context_tokens: Option<u64>,
}

/// What the lifecycle needs from the session's card.
#[derive(Clone, Debug)]
pub(crate) struct CardFacts {
    pub key: CardKey,
    pub stage: Stage,
    /// The card holds an Open Questions item that `hub waiting` lists.
    pub has_open_question: bool,
    /// A later session on the same card is live, as after a handoff.
    pub newer_session_live: bool,
    /// The card comes from the session's newest report, not its name
    /// or session ID.
    pub from_report: bool,
}

/// What the run folder says about the session's reports.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ReportFacts {
    pub unread: bool,
    pub ever_reported: bool,
    /// The hub delivered a message after its last read report.
    pub replied_since_report: bool,
}

/// Applies the lifecycle rules in order and returns the first that
/// matches.
pub(crate) fn decide_lifecycle(facts: &SessionFacts) -> Verdict {
    let stage = facts.card.as_ref().map(|card| card.stage);
    // Rule 1. Facts give a session with no live agent only for a
    // parked record. Its record goes once its card's work is over.
    let Some(agent) = &facts.agent else {
        return match stage {
            Some(Stage::Done | Stage::Dropped) => make_verdict(Lifecycle::ToClose, "card done"),
            _ => make_verdict(Lifecycle::Parked, ""),
        };
    };
    // Rule 2. git runs only on this machine, so the hub never closes a
    // session on another one.
    if agent.machine.is_some() {
        return match agent.status {
            AgentStatus::Working => make_verdict(Lifecycle::Working, ""),
            AgentStatus::Blocked => make_verdict(Lifecycle::WaitsOnUser, "blocked"),
            _ if agent.marked => make_verdict(Lifecycle::WaitsOnUser, "needs you"),
            _ => make_kept(KeptReason::OtherMachine),
        };
    }
    // Rule 3. Only the user can move these on.
    if agent.status == AgentStatus::Blocked {
        return make_verdict(Lifecycle::WaitsOnUser, "blocked");
    }
    if agent.marked {
        return make_verdict(Lifecycle::WaitsOnUser, "needs you");
    }
    if facts
        .card
        .as_ref()
        .is_some_and(|card| card.has_open_question)
    {
        return make_verdict(Lifecycle::WaitsOnUser, "open question");
    }
    // Rule 4. A session in an unknown state is never closed.
    match agent.status {
        AgentStatus::Working => return make_verdict(Lifecycle::Working, ""),
        AgentStatus::Unknown => return make_verdict(Lifecycle::Working, "state unknown"),
        AgentStatus::Idle | AgentStatus::Done | AgentStatus::Blocked => {}
    }
    // Rule 5. The hub owes it a read, or a message still waits.
    if facts.reports.unread {
        return make_verdict(Lifecycle::WaitsOnHub, "unread report");
    }
    if facts.messages.has_any() {
        return make_verdict(Lifecycle::WaitsOnHub, "");
    }
    // Rule 6. An old hub closes only once `run_handoff` or `run_adopt`
    // named it, and not while the user types in it or its approval
    // waits.
    if is_hub_name(&facts.name) {
        return if facts.handed_off {
            make_close_unless_user_waits(facts, agent, "handed off")
        } else {
            make_kept(KeptReason::AnotherHub)
        };
    }
    // Rules 7 to 10 close or park by the card's stage, so a card that
    // cannot be read keeps the session.
    if facts.card_unknown {
        return make_kept(KeptReason::CardUnknown);
    }
    match (&facts.card, stage) {
        // Rule 7. The card's work goes on. A newer session owns the
        // worktree, so closing this tab changes no file. A card needing
        // the user's review that the newest report names counts too,
        // since its worker may still have merges to run.
        (Some(card), Some(Stage::Draft | Stage::Todo | Stage::Active | Stage::InReview))
            if card.stage != Stage::InReview || card.from_report =>
        {
            if card.newer_session_live {
                make_close_unless_user_waits(facts, agent, "newer session")
            } else if !facts.reports.ever_reported {
                make_verdict(Lifecycle::WaitsOnUser, "never reported")
            } else if facts.reports.replied_since_report {
                make_verdict(Lifecycle::WaitsOnHub, "idle after reply")
            } else {
                make_verdict(Lifecycle::WaitsOnHub, "no reply yet")
            }
        }
        // Rule 8. A finished worker waits for the user's review or for
        // people outside the team, and parking keeps a way back to it.
        (Some(_), Some(Stage::InReview | Stage::Blocked)) => find_kept_reason(facts, agent, true)
            .unwrap_or_else(|| make_verdict(Lifecycle::ToPark, "")),
        // Rule 9. Nothing waits on the session of a card whose work
        // is over.
        (Some(_), _) => find_kept_reason(facts, agent, false)
            .unwrap_or_else(|| make_verdict(Lifecycle::ToClose, "card done")),
        // Rule 10. A session with no card that never reported is the
        // user's own.
        (None, _) if !facts.reports.ever_reported => make_kept(KeptReason::Yours),
        (None, _) => find_kept_reason(facts, agent, false)
            .unwrap_or_else(|| make_verdict(Lifecycle::ToClose, "report read")),
    }
}

/// Tells whether a name belongs to a hub, such as `kanban-hub-13`.
pub(crate) fn is_hub_name(name: &str) -> bool {
    !name.starts_with(REVIEW_PREFIX)
        && name.rsplit_once("-hub-").is_some_and(|(_, number)| {
            !number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit())
        })
}

/// Tells whether `name` is a hub older than `own` that no `handoff`
/// event named, such as `kanban-hub-3` left open when the user started
/// `kanban-hub-4` by hand. Only hubs of the same run count, which share
/// the word before `-hub-`. `handed_off` holds the old hubs of the
/// run's `handoff` events.
pub(crate) fn is_unadopted_hub(own: &str, name: &str, handed_off: &BTreeSet<String>) -> bool {
    let (Some((own_word, own_number)), Some((word, number))) =
        (split_hub_name(own), split_hub_name(name))
    else {
        return false;
    };
    word == own_word && number < own_number && !handed_off.contains(name)
}

/// Tells whether `run_adopt`, run by `own`, may take over `name`, an
/// unadopted hub, or an auto run's hub, such as `auto-hub-13`, that no
/// `handoff` event named. An auto hub hands off to no one, so the front
/// hub adopts it once its run is over, and Rule 6 can close it.
pub(crate) fn can_adopt_hub(own: &str, name: &str, handed_off: &BTreeSet<String>) -> bool {
    is_unadopted_hub(own, name, handed_off)
        || (is_auto_hub(name) && name != own && !handed_off.contains(name))
}

/// Tells whether `name` is an auto run's hub, such as `auto-hub-13`.
pub(crate) fn is_auto_hub(name: &str) -> bool {
    split_hub_name(name).is_some_and(|(word, _)| word == AUTO_HUB_WORD)
}

/// Returns 13 for `kanban-hub-13`, and `None` for any other kind of hub
/// or name.
pub(crate) fn parse_kanban_hub_number(name: &str) -> Option<u64> {
    split_hub_name(name)
        .filter(|(word, _)| *word == KANBAN_HUB_WORD)
        .map(|(_, number)| number)
}

/// Returns 24 for `auto-hub-24`, an auto run's hub, which carries its
/// run's number, and `None` for any other kind of hub or name.
pub(crate) fn parse_auto_hub_number(name: &str) -> Option<u64> {
    split_hub_name(name)
        .filter(|(word, _)| *word == AUTO_HUB_WORD)
        .map(|(_, number)| number)
}

/// Splits a hub name such as `kanban-hub-13` into `kanban` and 13.
fn split_hub_name(name: &str) -> Option<(&str, u64)> {
    if !is_hub_name(name) {
        return None;
    }
    let (word, number) = name.rsplit_once("-hub-")?;
    Some((word, number.parse().ok()?))
}

/// Returns the first check that keeps a finished session open. The
/// checks look for unsaved work, then the user in its pane, then a
/// waiting or recent approval request, then an open wait record. A
/// session that would be parked also needs a way to reopen it.
fn find_kept_reason(facts: &SessionFacts, agent: &Agent, needs_reopen: bool) -> Option<Verdict> {
    match &facts.unsaved {
        Unsaved::Clean => {}
        Unsaved::Files(worktree) => {
            let why = format!("unsaved files in {}", worktree.display());
            return Some(make_kept_why(KeptReason::UnsavedFiles, why));
        }
        Unsaved::Commits(worktree) => {
            let why = format!("unpushed in {}", worktree.display());
            return Some(make_kept_why(KeptReason::Unpushed, why));
        }
        Unsaved::GitFailed(message) => {
            let why = format!("git failed, {message}");
            return Some(make_kept_why(KeptReason::GitFailed, why));
        }
        // Facts look with git before any session reaches these rules,
        // so this means git never ran, and nothing is known to be safe.
        Unsaved::NotChecked => {
            let why = "git failed, it did not run".to_owned();
            return Some(make_kept_why(KeptReason::GitFailed, why));
        }
    }
    if let Some(verdict) = find_focus_or_approval(facts, agent) {
        return Some(verdict);
    }
    if facts.wait_open {
        return Some(make_kept(KeptReason::WaitsOnJob));
    }
    let can_reopen = agent.kind == AgentKind::OpenCode && agent.session_id.is_some();
    if needs_reopen && !can_reopen {
        return Some(make_kept(KeptReason::CannotReopen));
    }
    None
}

/// Returns `to-close` with WHY, unless the user is in the pane or an
/// approval request waits or changed lately. git does not look,
/// since a newer hub or session owns the worktree.
fn make_close_unless_user_waits(facts: &SessionFacts, agent: &Agent, why: &str) -> Verdict {
    find_focus_or_approval(facts, agent).unwrap_or_else(|| make_verdict(Lifecycle::ToClose, why))
}

/// Returns a kept verdict when the user is in the pane, or an approval
/// request from it waits or changed in the last hour.
fn find_focus_or_approval(facts: &SessionFacts, agent: &Agent) -> Option<Verdict> {
    if agent.focused {
        return Some(make_kept(KeptReason::UserInPane));
    }
    if facts.approval_pending {
        return Some(make_kept(KeptReason::ApprovalWaits));
    }
    if facts.filed_request_recently {
        return Some(make_kept(KeptReason::RecentRequest));
    }
    None
}

/// Returns a verdict whose why is plain text.
fn make_verdict(state: Lifecycle, why: &str) -> Verdict {
    Verdict {
        state,
        why: why.to_owned(),
    }
}

/// Returns a kept verdict whose why is the reason's label.
fn make_kept(reason: KeptReason) -> Verdict {
    make_kept_why(reason, reason.label().to_owned())
}

/// Returns a kept verdict with a longer why, such as the worktree.
fn make_kept_why(reason: KeptReason, why: String) -> Verdict {
    Verdict {
        state: Lifecycle::Kept(reason),
        why,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::shared::test_values::{make_agent, make_facts, make_parked_record};

    /// One row of the state table.
    struct Row {
        id: &'static str,
        facts: SessionFacts,
        expected: Lifecycle,
    }

    /// The state table. Each row changes the base worker from
    /// `make_facts`, which gives `to-park`, in one way.
    fn make_lifecycle_rows() -> Vec<Row> {
        let worker = "demo-kb-007-impl";
        let base = make_facts(worker);
        let make_facts_with_status = |status| SessionFacts {
            agent: Some(Agent {
                session_id: Some("ses_worker".to_owned()),
                ..make_agent(worker, status)
            }),
            ..make_facts(worker)
        };
        let make_facts_with_stage = |stage| {
            let mut facts = make_facts(worker);
            facts.card.as_mut().unwrap().stage = stage;
            facts
        };
        let make_facts_with_agent = |change: fn(&mut Agent)| {
            let mut facts = make_facts(worker);
            change(facts.agent.as_mut().unwrap());
            facts
        };
        let make_active_facts_with_reports = |reports| SessionFacts {
            reports,
            ..make_facts_with_stage(Stage::Active)
        };
        let make_hub_facts = |handed_off| SessionFacts {
            name: "kanban-hub-16".to_owned(),
            agent: Some(make_agent("kanban-hub-16", AgentStatus::Idle)),
            card: None,
            handed_off,
            ..make_facts(worker)
        };
        let make_no_card_facts = |name: &str, ever_reported| SessionFacts {
            name: name.to_owned(),
            agent: Some(make_agent(name, AgentStatus::Idle)),
            card: None,
            reports: ReportFacts {
                ever_reported,
                ..ReportFacts::default()
            },
            ..make_facts(worker)
        };
        let make_parked_facts = |stage| SessionFacts {
            agent: None,
            parked: Some(make_parked_record(worker)),
            ..make_facts_with_stage(stage)
        };
        let worktree = PathBuf::from("/home/u/wt/demo-kb-007");
        vec![
            Row {
                id: "parked",
                facts: make_parked_facts(Stage::InReview),
                expected: Lifecycle::Parked,
            },
            Row {
                id: "parked-card-done",
                facts: make_parked_facts(Stage::Done),
                expected: Lifecycle::ToClose,
            },
            Row {
                id: "other-machine-idle",
                facts: make_facts_with_agent(|agent| agent.machine = Some("laptop".to_owned())),
                expected: Lifecycle::Kept(KeptReason::OtherMachine),
            },
            Row {
                id: "blocked-card-done",
                facts: SessionFacts {
                    agent: Some(make_agent(worker, AgentStatus::Blocked)),
                    ..make_facts_with_stage(Stage::Done)
                },
                expected: Lifecycle::WaitsOnUser,
            },
            Row {
                id: "marked",
                facts: make_facts_with_agent(|agent| agent.marked = true),
                expected: Lifecycle::WaitsOnUser,
            },
            Row {
                id: "open-question",
                facts: {
                    let mut facts = make_facts(worker);
                    facts.card.as_mut().unwrap().has_open_question = true;
                    facts
                },
                expected: Lifecycle::WaitsOnUser,
            },
            Row {
                id: "working",
                facts: make_facts_with_status(AgentStatus::Working),
                expected: Lifecycle::Working,
            },
            Row {
                id: "unknown",
                facts: make_facts_with_status(AgentStatus::Unknown),
                expected: Lifecycle::Working,
            },
            Row {
                id: "unread-report",
                facts: SessionFacts {
                    reports: ReportFacts {
                        unread: true,
                        ever_reported: true,
                        replied_since_report: false,
                    },
                    ..make_facts(worker)
                },
                expected: Lifecycle::WaitsOnHub,
            },
            Row {
                id: "not-sent",
                facts: SessionFacts {
                    messages: MessageCounts {
                        not_sent: 1,
                        ..MessageCounts::default()
                    },
                    ..make_facts(worker)
                },
                expected: Lifecycle::WaitsOnHub,
            },
            Row {
                id: "handed-off-hub",
                facts: make_hub_facts(true),
                expected: Lifecycle::ToClose,
            },
            Row {
                id: "handed-off-focused",
                facts: {
                    let mut facts = make_hub_facts(true);
                    facts.agent.as_mut().unwrap().focused = true;
                    facts
                },
                expected: Lifecycle::Kept(KeptReason::UserInPane),
            },
            Row {
                id: "other-hub",
                facts: make_hub_facts(false),
                expected: Lifecycle::Kept(KeptReason::AnotherHub),
            },
            Row {
                id: "handed-off",
                facts: {
                    let mut facts = make_facts_with_stage(Stage::Active);
                    facts.card.as_mut().unwrap().newer_session_live = true;
                    facts
                },
                expected: Lifecycle::ToClose,
            },
            Row {
                id: "handed-off-approval",
                facts: {
                    let mut facts = make_facts_with_stage(Stage::Active);
                    facts.card.as_mut().unwrap().newer_session_live = true;
                    facts.approval_pending = true;
                    facts
                },
                expected: Lifecycle::Kept(KeptReason::ApprovalWaits),
            },
            Row {
                id: "card-unknown",
                facts: SessionFacts {
                    card: None,
                    card_unknown: true,
                    ..make_facts(worker)
                },
                expected: Lifecycle::Kept(KeptReason::CardUnknown),
            },
            Row {
                id: "never-reported",
                facts: make_active_facts_with_reports(ReportFacts::default()),
                expected: Lifecycle::WaitsOnUser,
            },
            Row {
                id: "read-no-reply",
                facts: make_active_facts_with_reports(ReportFacts {
                    ever_reported: true,
                    ..ReportFacts::default()
                }),
                expected: Lifecycle::WaitsOnHub,
            },
            Row {
                id: "review-clean",
                facts: base,
                expected: Lifecycle::ToPark,
            },
            Row {
                id: "blocked-clean",
                facts: make_facts_with_stage(Stage::Blocked),
                expected: Lifecycle::ToPark,
            },
            Row {
                id: "review-files",
                facts: SessionFacts {
                    unsaved: Unsaved::Files(worktree.clone()),
                    ..make_facts(worker)
                },
                expected: Lifecycle::Kept(KeptReason::UnsavedFiles),
            },
            Row {
                id: "review-unpushed",
                facts: SessionFacts {
                    unsaved: Unsaved::Commits(worktree),
                    ..make_facts(worker)
                },
                expected: Lifecycle::Kept(KeptReason::Unpushed),
            },
            Row {
                id: "review-git-failed",
                facts: SessionFacts {
                    unsaved: Unsaved::GitFailed("not a git repository".to_owned()),
                    ..make_facts(worker)
                },
                expected: Lifecycle::Kept(KeptReason::GitFailed),
            },
            Row {
                id: "review-focused",
                facts: make_facts_with_agent(|agent| agent.focused = true),
                expected: Lifecycle::Kept(KeptReason::UserInPane),
            },
            Row {
                id: "review-approval",
                facts: SessionFacts {
                    approval_pending: true,
                    ..make_facts(worker)
                },
                expected: Lifecycle::Kept(KeptReason::ApprovalWaits),
            },
            Row {
                id: "review-open-wait",
                facts: SessionFacts {
                    wait_open: true,
                    ..make_facts(worker)
                },
                expected: Lifecycle::Kept(KeptReason::WaitsOnJob),
            },
            Row {
                id: "done-recent-request",
                facts: SessionFacts {
                    filed_request_recently: true,
                    ..make_facts_with_stage(Stage::Done)
                },
                expected: Lifecycle::Kept(KeptReason::RecentRequest),
            },
            Row {
                id: "review-codex",
                facts: make_facts_with_agent(|agent| {
                    agent.kind = AgentKind::Other("codex".to_owned())
                }),
                expected: Lifecycle::Kept(KeptReason::CannotReopen),
            },
            Row {
                id: "review-no-session-id",
                facts: make_facts_with_agent(|agent| agent.session_id = None),
                expected: Lifecycle::Kept(KeptReason::CannotReopen),
            },
            Row {
                id: "done-clean",
                facts: make_facts_with_stage(Stage::Done),
                expected: Lifecycle::ToClose,
            },
            Row {
                id: "done-files",
                facts: SessionFacts {
                    unsaved: Unsaved::Files(PathBuf::from("/home/u/wt/demo-kb-007")),
                    ..make_facts_with_stage(Stage::Done)
                },
                expected: Lifecycle::Kept(KeptReason::UnsavedFiles),
            },
            Row {
                id: "done-focused",
                facts: {
                    let mut facts = make_facts_with_stage(Stage::Done);
                    facts.agent.as_mut().unwrap().focused = true;
                    facts
                },
                expected: Lifecycle::Kept(KeptReason::UserInPane),
            },
            Row {
                id: "done-not-checked",
                facts: SessionFacts {
                    unsaved: Unsaved::NotChecked,
                    ..make_facts_with_stage(Stage::Done)
                },
                expected: Lifecycle::Kept(KeptReason::GitFailed),
            },
            Row {
                id: "no-card-never-reported",
                facts: make_no_card_facts("notes", false),
                expected: Lifecycle::Kept(KeptReason::Yours),
            },
            Row {
                id: "no-card-report-read",
                facts: make_no_card_facts("notes", true),
                expected: Lifecycle::ToClose,
            },
            Row {
                id: "session-review",
                facts: make_no_card_facts("review-kanban-hub-16", true),
                expected: Lifecycle::ToClose,
            },
        ]
    }

    /// The tidy pass will close every `to-close` session and park
    /// every `to-park` one. A rule out of order, or a check left out,
    /// can close a tab that holds unsaved work, a pending approval, or
    /// the user's own typing. A blocked session whose card's work is
    /// over must still reach the user.
    #[test]
    fn decide_lifecycle_matches_state_table() {
        for row in make_lifecycle_rows() {
            let verdict = decide_lifecycle(&row.facts);

            assert_eq!(verdict.state, row.expected, "{}", row.id);
        }
    }

    /// A new state or kept reason can change what the tidy pass
    /// closes. Without a row in the state table, no test catches that
    /// change. The match below has no catch-all arm, so a new variant
    /// fails to build here until someone lists it and gives it a row.
    #[test]
    fn lifecycle_table_has_row_for_every_state() {
        let rows = make_lifecycle_rows();
        let reasons = [
            KeptReason::UnsavedFiles,
            KeptReason::Unpushed,
            KeptReason::GitFailed,
            KeptReason::UserInPane,
            KeptReason::ApprovalWaits,
            KeptReason::RecentRequest,
            KeptReason::CannotReopen,
            KeptReason::AnotherHub,
            KeptReason::OtherMachine,
            KeptReason::Yours,
            KeptReason::CardUnknown,
            KeptReason::WaitsOnJob,
        ];
        let mut states = vec![
            Lifecycle::Working,
            Lifecycle::WaitsOnHub,
            Lifecycle::WaitsOnUser,
            Lifecycle::ToClose,
            Lifecycle::ToPark,
            Lifecycle::Parked,
        ];
        states.extend(reasons.map(Lifecycle::Kept));

        for state in states {
            match state {
                Lifecycle::Working
                | Lifecycle::WaitsOnHub
                | Lifecycle::WaitsOnUser
                | Lifecycle::Kept(
                    KeptReason::UnsavedFiles
                    | KeptReason::Unpushed
                    | KeptReason::GitFailed
                    | KeptReason::UserInPane
                    | KeptReason::ApprovalWaits
                    | KeptReason::RecentRequest
                    | KeptReason::CannotReopen
                    | KeptReason::AnotherHub
                    | KeptReason::OtherMachine
                    | KeptReason::Yours
                    | KeptReason::CardUnknown
                    | KeptReason::WaitsOnJob,
                )
                | Lifecycle::ToClose
                | Lifecycle::ToPark
                | Lifecycle::Parked => {}
            }
            assert!(
                rows.iter().any(|row| row.expected == state),
                "{state:?} has no row"
            );
        }
    }

    /// Hubs are found by name alone, with hand-written matching in place
    /// of a regex. A worker whose name only contains `-hub-`, if mistaken
    /// for a hub, would be moved to the top by `gather`, shown by `go`
    /// and kept open by `run_status` as another hub.
    #[test]
    fn is_hub_name_rejects_workers_and_reviews() {
        let cases = [
            ("hub", "kanban-hub-13", true),
            ("no-number", "kanban-hub-", false),
            ("text-after-number", "kanban-hub-13-notes", false),
            ("worker", "demo-kb-001-impl", false),
            ("old-hub-review", "review-kanban-hub-16", false),
        ];

        for (case_name, name, expected) in cases {
            assert_eq!(is_hub_name(name), expected, "{case_name}");
        }
    }

    /// `run_next` and `run_status` tell the hub to adopt each hub this
    /// returns, and an adopted hub gets closed and reviewed. A wrong
    /// match would close the hub that properly handed off, a newer hub,
    /// another run's hub or a running review.
    #[test]
    fn is_unadopted_hub_matches_older_unnamed_hubs() {
        let handed_off = BTreeSet::from(["kanban-hub-2".to_owned()]);
        let cases = [
            ("older-same-run", "kanban-hub-3", true),
            ("handed-off", "kanban-hub-2", false),
            ("other-run", "auto-hub-3", false),
            ("newer", "kanban-hub-5", false),
            ("review", "review-kanban-hub-3", false),
        ];

        for (case_name, name, expected) in cases {
            let unadopted = is_unadopted_hub("kanban-hub-4", name, &handed_off);

            assert_eq!(unadopted, expected, "{case_name}");
        }
    }

    /// The front hub adopts an auto hub once its run is over, since an
    /// auto hub hands off to no one and would stay open for good. A
    /// wrong match would adopt the running hub itself or adopt one
    /// hub twice, which starts a second review of it.
    #[test]
    fn can_adopt_hub_accepts_auto_hubs_once() {
        let handed_off = BTreeSet::from(["auto-hub-2".to_owned()]);
        let cases = [
            ("auto-hub", "kanban-hub-4", "auto-hub-3", true),
            ("auto-hub-adopted", "kanban-hub-4", "auto-hub-2", false),
            ("own-auto-hub", "auto-hub-3", "auto-hub-3", false),
            ("unadopted-hub", "kanban-hub-4", "kanban-hub-3", true),
        ];

        for (case_name, own, name, expected) in cases {
            let adoptable = can_adopt_hub(own, name, &handed_off);

            assert_eq!(adoptable, expected, "{case_name}");
        }
    }
}
