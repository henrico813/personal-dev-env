//! Runs `hub flag` and `hub unflag`, the sidebar's "needs you" mark.

use super::{Outcome, sidebar};
use crate::external::herdr::AgentStatus;
use crate::shared::{AgentName, HubError, Tools};

/// Shows "needs you" in place of the session's state in the sidebar.
/// With `on_this_machine`, it then sets the "go to" list again.
pub fn run_flag(
    tools: &Tools<'_>,
    name: &AgentName,
    on_this_machine: bool,
) -> Result<Outcome, HubError> {
    let agent = tools.herdr.get_agent(name.as_str())?;
    tools
        .herdr
        .mark_needs_you(&agent.pane_id, choose_marked_states(agent.status))?;
    let mut outcome = Outcome::from_line(format!("flagged {}", name.as_str()));
    outcome
        .lines
        .extend(refresh_go_to_list(tools, on_this_machine));
    Ok(outcome)
}

/// Removes the "needs you" mark from the session's pane. With
/// `on_this_machine`, it then sets the "go to" list again.
pub fn run_unflag(
    tools: &Tools<'_>,
    name: &AgentName,
    on_this_machine: bool,
) -> Result<Outcome, HubError> {
    let agent = tools.herdr.get_agent(name.as_str())?;
    tools.herdr.clear_marks(&agent.pane_id)?;
    let mut outcome = Outcome::from_line(format!("unflagged {}", name.as_str()));
    outcome
        .lines
        .extend(refresh_go_to_list(tools, on_this_machine));
    Ok(outcome)
}

/// Sets the "go to" list again, so the mark's change shows even when
/// Herdr lost the filter and no `hub watch` runs. Returns a warning
/// line when that fails, since the mark is already set. With
/// `--machine` it does nothing, since that machine's run folders, which
/// name its front hub, are out of reach.
fn refresh_go_to_list(tools: &Tools<'_>, on_this_machine: bool) -> Option<String> {
    if !on_this_machine {
        return None;
    }
    let error = sidebar::sync_go_to_list(tools).err()?;
    Some(format!("could not set the go to list: {error}"))
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
