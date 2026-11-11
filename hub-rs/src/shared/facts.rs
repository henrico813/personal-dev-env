//! Reads facts about sessions from the external systems. So far it reads
//! only the hub's own line.

use super::Tools;

/// The hub's own session and context, for the end of each news line.
#[derive(Debug, Default)]
pub(crate) struct HubLine {
    pub session_id: Option<String>,
    pub context_tokens: Option<u64>,
}

/// Reads the hub's own agent and its context. Anything that fails
/// leaves a field `None`, so a news line still prints.
pub(crate) fn read_hub_line(tools: &Tools<'_>) -> HubLine {
    let Some(pane) = tools.own_pane else {
        return HubLine::default();
    };
    let session_id = tools
        .herdr
        .get_agent(pane)
        .ok()
        .and_then(|agent| agent.session_id);
    let context_tokens = session_id
        .as_deref()
        .and_then(|session| tools.opencode.read_context_tokens(session).ok().flatten());
    HubLine {
        session_id,
        context_tokens,
    }
}
