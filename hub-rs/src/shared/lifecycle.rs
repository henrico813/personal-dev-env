//! Works out one session's lifecycle state from its facts, with no
//! external calls. Read `decide_lifecycle` first. Its rules apply in
//! order and the first match wins, so a blocked session is `you` even
//! when its card is accepted. The later tidy pass closes or parks a
//! session only in the `to-close` or `to-park` state, so each rule
//! that gives one of those decides what the hub may close.

use super::messages::MessageCounts;
use super::parking::ParkedRecord;
use crate::external::cards::{CardKey, Stage};
use crate::external::git::Unsaved;
use crate::external::herdr::{Agent, AgentKind, AgentStatus};

/// The review session that `hub handoff` creates for the old hub gets
/// a name starting with this, so `review-kanban-hub-16` is not a hub.
const REVIEW_PREFIX: &str = "review-";

/// One session's place in its life. Each `match` on it must name
/// every state. Code that forgets to handle a new state then fails to
/// build.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    /// Returns the word `hub status` shows, such as `hub` or `to-park`.
    pub(crate) fn word(self) -> &'static str {
        match self {
            Lifecycle::Working => "working",
            Lifecycle::WaitsOnHub => "hub",
            Lifecycle::WaitsOnUser => "you",
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KeptReason {
    UnsavedFiles,
    Unpushed,
    GitFailed,
    UserInPane,
    ApprovalWaits,
    CannotReopen,
    AnotherHub,
    OtherMachine,
    Yours,
    CardUnknown,
}

impl KeptReason {
    /// Returns the short words `hub status` shows, such as
    /// `unsaved files`.
    pub(crate) fn label(self) -> &'static str {
        match self {
            KeptReason::UnsavedFiles => "unsaved files",
            KeptReason::Unpushed => "unpushed",
            KeptReason::GitFailed => "git failed",
            KeptReason::UserInPane => "user in pane",
            KeptReason::ApprovalWaits => "approval waits",
            KeptReason::CannotReopen => "cannot reopen",
            KeptReason::AnotherHub => "another hub",
            KeptReason::OtherMachine => "other machine",
            KeptReason::Yours => "yours",
            KeptReason::CardUnknown => "card unknown",
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
    pub(crate) fn why_or_state(&self) -> &str {
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
    /// The card cannot be read, because `card` failed or the board
    /// has no stage for the key in the session's name.
    pub card_unknown: bool,
    pub reports: ReportFacts,
    pub messages: MessageCounts,
    pub unsaved: Unsaved,
    /// True when a pending, approved or running pde-gh-write request
    /// came from this session's pane or session ID. Such a request
    /// asks the user to approve a pull request change. A request file
    /// that does not parse sets it for every live session.
    pub approval_pending: bool,
    /// The session is the hub named in this run's last `handoff`
    /// event.
    pub is_predecessor: bool,
    pub context_tokens: Option<u64>,
}

/// What the lifecycle needs from the session's card.
#[derive(Clone, Debug)]
pub(crate) struct CardFacts {
    pub key: CardKey,
    pub stage: Stage,
    pub needs_you_open: bool,
    /// A later session on the same card is live, as after a handoff.
    pub newer_session_live: bool,
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
    // parked record. Its record goes once its card is done.
    let Some(agent) = &facts.agent else {
        return match stage {
            Some(Stage::Accepted | Stage::Dropped) => make_verdict(Lifecycle::ToClose, "card done"),
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
    if facts.card.as_ref().is_some_and(|card| card.needs_you_open) {
        return make_verdict(Lifecycle::WaitsOnUser, "needs-you note");
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
    if facts.messages.any() {
        return make_verdict(Lifecycle::WaitsOnHub, "");
    }
    // Rule 6. An old hub closes only once `hub handoff` named it, and
    // not while the user types in it or its approval waits.
    if is_hub_name(&facts.name) && !facts.name.starts_with(REVIEW_PREFIX) {
        return if facts.is_predecessor {
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
        // worktree, so closing this tab changes no file.
        (Some(card), Some(Stage::Draft | Stage::Ready | Stage::Active)) => {
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
        // Rule 8. A finished worker waits for the user's review, and
        // parking keeps a way back to it.
        (Some(_), Some(Stage::Review)) => find_kept_reason(facts, agent, true)
            .unwrap_or_else(|| make_verdict(Lifecycle::ToPark, "")),
        // Rule 9. Nothing waits on a done card's session.
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
    name.rsplit_once("-hub-").is_some_and(|(_, number)| {
        !number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit())
    })
}

/// Returns the first check that keeps a finished session open. The
/// checks look for unsaved work, then the user in its pane, then a
/// waiting approval. A session that would be parked also needs a way
/// to reopen it.
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
    let can_reopen = agent.kind == AgentKind::OpenCode && agent.session_id.is_some();
    if needs_reopen && !can_reopen {
        return Some(make_kept(KeptReason::CannotReopen));
    }
    None
}

/// Returns `to-close` with WHY, unless the user is in the pane or an
/// approval waits. git does not look, since a newer hub or session
/// owns the worktree.
fn make_close_unless_user_waits(facts: &SessionFacts, agent: &Agent, why: &str) -> Verdict {
    find_focus_or_approval(facts, agent).unwrap_or_else(|| make_verdict(Lifecycle::ToClose, why))
}

/// Returns a kept verdict when the user is in the pane or an approval
/// request from it waits.
fn find_focus_or_approval(facts: &SessionFacts, agent: &Agent) -> Option<Verdict> {
    if agent.focused {
        return Some(make_kept(KeptReason::UserInPane));
    }
    if facts.approval_pending {
        return Some(make_kept(KeptReason::ApprovalWaits));
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
        let with_status = |status| SessionFacts {
            agent: Some(Agent {
                session_id: Some("ses_worker".to_owned()),
                ..make_agent(worker, status)
            }),
            ..make_facts(worker)
        };
        let with_stage = |stage| {
            let mut facts = make_facts(worker);
            facts.card.as_mut().unwrap().stage = stage;
            facts
        };
        let with_agent = |change: fn(&mut Agent)| {
            let mut facts = make_facts(worker);
            change(facts.agent.as_mut().unwrap());
            facts
        };
        let active_with_reports = |reports| SessionFacts {
            reports,
            ..with_stage(Stage::Active)
        };
        let hub = |is_predecessor| SessionFacts {
            name: "kanban-hub-16".to_owned(),
            agent: Some(make_agent("kanban-hub-16", AgentStatus::Idle)),
            card: None,
            is_predecessor,
            ..make_facts(worker)
        };
        let no_card = |name: &str, ever_reported| SessionFacts {
            name: name.to_owned(),
            agent: Some(make_agent(name, AgentStatus::Idle)),
            card: None,
            reports: ReportFacts {
                ever_reported,
                ..ReportFacts::default()
            },
            ..make_facts(worker)
        };
        let parked = |stage| SessionFacts {
            agent: None,
            parked: Some(make_parked_record(worker)),
            ..with_stage(stage)
        };
        let worktree = PathBuf::from("/home/u/wt/demo-kb-007");
        vec![
            Row {
                id: "parked",
                facts: parked(Stage::Review),
                expected: Lifecycle::Parked,
            },
            Row {
                id: "parked-card-done",
                facts: parked(Stage::Accepted),
                expected: Lifecycle::ToClose,
            },
            Row {
                id: "other-machine-idle",
                facts: with_agent(|agent| agent.machine = Some("laptop".to_owned())),
                expected: Lifecycle::Kept(KeptReason::OtherMachine),
            },
            Row {
                id: "blocked-card-accepted",
                facts: SessionFacts {
                    agent: Some(make_agent(worker, AgentStatus::Blocked)),
                    ..with_stage(Stage::Accepted)
                },
                expected: Lifecycle::WaitsOnUser,
            },
            Row {
                id: "marked",
                facts: with_agent(|agent| agent.marked = true),
                expected: Lifecycle::WaitsOnUser,
            },
            Row {
                id: "needs-you-note",
                facts: {
                    let mut facts = make_facts(worker);
                    facts.card.as_mut().unwrap().needs_you_open = true;
                    facts
                },
                expected: Lifecycle::WaitsOnUser,
            },
            Row {
                id: "working",
                facts: with_status(AgentStatus::Working),
                expected: Lifecycle::Working,
            },
            Row {
                id: "unknown",
                facts: with_status(AgentStatus::Unknown),
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
                id: "predecessor-hub",
                facts: hub(true),
                expected: Lifecycle::ToClose,
            },
            Row {
                id: "predecessor-focused",
                facts: {
                    let mut facts = hub(true);
                    facts.agent.as_mut().unwrap().focused = true;
                    facts
                },
                expected: Lifecycle::Kept(KeptReason::UserInPane),
            },
            Row {
                id: "other-hub",
                facts: hub(false),
                expected: Lifecycle::Kept(KeptReason::AnotherHub),
            },
            Row {
                id: "handed-off",
                facts: {
                    let mut facts = with_stage(Stage::Active);
                    facts.card.as_mut().unwrap().newer_session_live = true;
                    facts
                },
                expected: Lifecycle::ToClose,
            },
            Row {
                id: "handed-off-approval",
                facts: {
                    let mut facts = with_stage(Stage::Active);
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
                facts: active_with_reports(ReportFacts::default()),
                expected: Lifecycle::WaitsOnUser,
            },
            Row {
                id: "read-no-reply",
                facts: active_with_reports(ReportFacts {
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
                facts: with_agent(|agent| agent.focused = true),
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
                id: "review-codex",
                facts: with_agent(|agent| agent.kind = AgentKind::Other("codex".to_owned())),
                expected: Lifecycle::Kept(KeptReason::CannotReopen),
            },
            Row {
                id: "review-no-session-id",
                facts: with_agent(|agent| agent.session_id = None),
                expected: Lifecycle::Kept(KeptReason::CannotReopen),
            },
            Row {
                id: "accepted-clean",
                facts: with_stage(Stage::Accepted),
                expected: Lifecycle::ToClose,
            },
            Row {
                id: "accepted-files",
                facts: SessionFacts {
                    unsaved: Unsaved::Files(PathBuf::from("/home/u/wt/demo-kb-007")),
                    ..with_stage(Stage::Accepted)
                },
                expected: Lifecycle::Kept(KeptReason::UnsavedFiles),
            },
            Row {
                id: "accepted-focused",
                facts: {
                    let mut facts = with_stage(Stage::Accepted);
                    facts.agent.as_mut().unwrap().focused = true;
                    facts
                },
                expected: Lifecycle::Kept(KeptReason::UserInPane),
            },
            Row {
                id: "accepted-not-checked",
                facts: SessionFacts {
                    unsaved: Unsaved::NotChecked,
                    ..with_stage(Stage::Accepted)
                },
                expected: Lifecycle::Kept(KeptReason::GitFailed),
            },
            Row {
                id: "no-card-never-reported",
                facts: no_card("notes", false),
                expected: Lifecycle::Kept(KeptReason::Yours),
            },
            Row {
                id: "no-card-report-read",
                facts: no_card("notes", true),
                expected: Lifecycle::ToClose,
            },
            Row {
                id: "session-review",
                facts: no_card("review-kanban-hub-16", true),
                expected: Lifecycle::ToClose,
            },
        ]
    }

    /// The tidy pass will close every `to-close` session and park
    /// every `to-park` one. A rule out of order, or a check left out,
    /// can close a tab that holds unsaved work, a pending approval, or
    /// the user's own typing. A blocked session whose card was accepted must still
    /// reach the user.
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
            KeptReason::CannotReopen,
            KeptReason::AnotherHub,
            KeptReason::OtherMachine,
            KeptReason::Yours,
            KeptReason::CardUnknown,
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
                    | KeptReason::CannotReopen
                    | KeptReason::AnotherHub
                    | KeptReason::OtherMachine
                    | KeptReason::Yours
                    | KeptReason::CardUnknown,
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
    /// and kept open by `hub status` as another hub.
    #[test]
    fn is_hub_name_needs_number_after_hub() {
        let cases = [
            ("hub", "kanban-hub-13", true),
            ("no-number", "kanban-hub-", false),
            ("text-after-number", "kanban-hub-13-notes", false),
            ("worker", "demo-kb-001-impl", false),
        ];

        for (case_name, name, expected) in cases {
            assert_eq!(is_hub_name(name), expected, "{case_name}");
        }
    }
}
