//! Values the `mod tests` blocks in `src/` share. Tests in `tests/`
//! cannot see this file and keep their own copy in `tests/common`.

use crate::outside::herdr::{Agent, AgentKind, AgentStatus};

/// One unfocused OpenCode agent in workspace `w1`, with pane
/// `w1:<name>`, tab `w1:t-<name>`, state change 1 and no session ID.
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
        state_seq: 1,
    }
}
