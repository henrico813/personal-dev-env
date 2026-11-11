//! Runs `hub alive`, which tells whether an OpenCode session still
//! works. It reads the database, so it sees workers and subagents that
//! Herdr does not list.

use jiff::Zoned;

use super::Outcome;
use crate::outside::opencode::OpenCodeError;
use crate::shared::{HubError, Tools};

/// Prints the message count, the newest time, the last tool, and
/// whether the session is working, finished or stopped, such as
/// `412 messages, newest 14:03:22, last tool shell, working`.
///
/// # Errors
///
/// `HubError::OpenCode` when this machine's database has no such
/// session, so a mistyped ID never reads as a stopped session.
pub fn run_alive(tools: &Tools<'_>, session: &str, now: &Zoned) -> Result<Outcome, HubError> {
    let Some(activity) = tools.opencode.read_activity(session, now)? else {
        return Err(OpenCodeError::NoSession(session.to_owned()).into());
    };
    let newest = activity.newest.to_zoned(now.time_zone().clone());
    Ok(Outcome::line(format!(
        "{} messages, newest {}, last tool {}, {}",
        activity.message_count,
        newest.strftime("%H:%M:%S"),
        activity.last_tool.as_deref().unwrap_or("-"),
        activity.word.as_str()
    )))
}
