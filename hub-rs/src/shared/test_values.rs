//! Values the `mod tests` blocks in `src/` share. Tests in `tests/`
//! cannot see this file and keep their own copy in `tests/common`.

use crate::external::cards::{CardKey, Stage};
use crate::external::git::Unsaved;
use crate::external::herdr::{Agent, AgentKind, AgentStatus};
use crate::shared::lifecycle::{CardFacts, ReportFacts, SessionFacts};
use crate::shared::messages::MessageCounts;
use crate::shared::parking::ParkedRecord;

/// One unfocused OpenCode agent in workspace `w1`, with pane
/// `w1:<name>`, tab `w1:t-<name>`, state change 1, no session ID, no
/// mark, no folders and no machine.
pub(crate) fn make_agent(name: &str, status: AgentStatus) -> Agent {
    Agent {
        name: name.to_owned(),
        status,
        kind: AgentKind::OpenCode,
        session_id: None,
        pane_id: format!("w1:{name}"),
        tab_id: format!("w1:t-{name}"),
        workspace_id: "w1".to_owned(),
        focused: false,
        marked: false,
        state_seq: 1,
        folders: Vec::new(),
        machine: None,
    }
}

/// One finished worker, the state table's base row, which parks. It is
/// an idle OpenCode agent with session `ses_worker`, its card in review
/// with no needs-you note, its report read, nothing waiting, its
/// worktree clean and no approval.
pub(crate) fn make_facts(name: &str) -> SessionFacts {
    SessionFacts {
        name: name.to_owned(),
        agent: Some(Agent {
            session_id: Some("ses_worker".to_owned()),
            ..make_agent(name, AgentStatus::Idle)
        }),
        parked: None,
        card: Some(CardFacts {
            key: CardKey::find_in_name(name).unwrap_or_else(|| "DEMO-KB-007".parse().unwrap()),
            stage: Stage::Review,
            needs_you_open: false,
            newer_session_live: false,
            from_report: false,
        }),
        card_unknown: false,
        reports: ReportFacts {
            unread: false,
            ever_reported: true,
            replied_since_report: false,
        },
        messages: MessageCounts::default(),
        unsaved: Unsaved::Clean,
        approval_pending: false,
        filed_request_recently: false,
        is_predecessor: false,
        context_tokens: None,
    }
}

/// A parked record for NAME on card `DEMO-KB-007`, parked at
/// `2026-11-10T22:41:00Z`.
pub(crate) fn make_parked_record(name: &str) -> ParkedRecord {
    ParkedRecord {
        name: name.to_owned(),
        card: "DEMO-KB-007".parse().unwrap(),
        parked_at: "2026-11-10T22:41:00Z".parse().unwrap(),
    }
}
