//! Runs `hub flag` and `hub unflag`, the sidebar's "needs you" mark.

use super::Outcome;
use crate::outside::herdr::AgentStatus;
use crate::shared::{AgentName, HubError, Tools};

/// Shows "needs you" in place of the session's state in the sidebar.
pub fn run_flag(tools: &Tools<'_>, name: &AgentName) -> Result<Outcome, HubError> {
    let agent = tools.herdr.get_agent(name.as_str())?;
    tools
        .herdr
        .mark_needs_you(&agent.pane_id, choose_marked_states(agent.status))?;
    Ok(Outcome::line(format!("flagged {}", name.as_str())))
}

/// Removes the "needs you" mark from the session's pane.
pub fn run_unflag(tools: &Tools<'_>, name: &AgentName) -> Result<Outcome, HubError> {
    let agent = tools.herdr.get_agent(name.as_str())?;
    tools.herdr.clear_marks(&agent.pane_id)?;
    Ok(Outcome::line(format!("unflagged {}", name.as_str())))
}

/// Returns the states that show the mark. A blocked session gets it
/// on blocked alone, so the mark clears once the session works again.
/// Every other status gets it on idle and done.
fn choose_marked_states(status: AgentStatus) -> &'static [AgentStatus] {
    match status {
        AgentStatus::Blocked => &[AgentStatus::Blocked],
        AgentStatus::Working | AgentStatus::Idle | AgentStatus::Done | AgentStatus::Unknown => {
            &[AgentStatus::Idle, AgentStatus::Done]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A blocked session marked on idle too would still show "needs
    /// you" after the user answers and work continues. A working session
    /// marked on working would show it while it still works.
    #[test]
    fn choose_marked_states_follows_status() {
        let cases = [
            (
                "idle",
                AgentStatus::Idle,
                &[AgentStatus::Idle, AgentStatus::Done][..],
            ),
            (
                "working",
                AgentStatus::Working,
                &[AgentStatus::Idle, AgentStatus::Done][..],
            ),
            ("blocked", AgentStatus::Blocked, &[AgentStatus::Blocked][..]),
        ];

        for (case_name, status, expected) in cases {
            assert_eq!(choose_marked_states(status), expected, "{case_name}");
        }
    }
}
