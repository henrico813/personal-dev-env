//! Tells the agent what the review waits on and which step to run next. An
//! agent can call it at any time. Read the `show_status(dir)` function to
//! understand the module.

use std::collections::BTreeMap;
use std::path::Path;

use super::{NextStep, StepOutput};
use crate::shared::Error;
use crate::shared::review_session::{self, RequestId, ReviewStatus};
use crate::shared::user_feedback::{UserFeedback, UserNote};

/// Returns what this worktree's review waits on and the step the agent runs
/// next.
pub fn show_status(dir: &Path) -> Result<StepOutput<StatusResult>, Error> {
    let (waiting, next) = match review_session::load(dir)? {
        None => (Waiting::Nothing, NextStep::CheckComments),
        Some(review) => match review.status {
            ReviewStatus::WaitingForUserFeedback => {
                let unanswered = review
                    .newest_requests()
                    .into_iter()
                    .filter(|request| !review.feedback.contains_key(&request.id))
                    .map(|request| request.id)
                    .collect();
                (
                    Waiting::FeedbackRequests(unanswered),
                    NextStep::TellUserAndWait,
                )
            }
            ReviewStatus::UserNotesWaiting => {
                let notes = review
                    .feedback
                    .into_iter()
                    .filter_map(|(id, feedback)| match feedback {
                        UserFeedback::Note(note) => Some((id, note)),
                        UserFeedback::Answer(_) => None,
                    })
                    .collect();
                (Waiting::UserNotes(notes), NextStep::MakeFeedbackRequestList)
            }
            // The user checks git status before any step runs again.
            ReviewStatus::WritingUserFeedback => (Waiting::Nothing, NextStep::TellUserAndWait),
            ReviewStatus::Done => (Waiting::Nothing, NextStep::CheckComments),
        },
    };
    Ok(StepOutput {
        result: StatusResult { waiting },
        next,
    })
}

/// Holds what the review waits on.
#[derive(Debug)]
pub struct StatusResult {
    pub waiting: Waiting,
}

/// Says who the review waits on, and for which requests.
#[derive(Debug)]
pub enum Waiting {
    Nothing,
    /// Lists the requests the user has not answered yet.
    FeedbackRequests(Vec<RequestId>),
    /// Holds the User Notes the agent answers next, by request.
    UserNotes(BTreeMap<RequestId, UserNote>),
}
