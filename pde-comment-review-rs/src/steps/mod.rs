//! Holds the steps of a review, one per file.

use serde::Serialize;

use crate::shared::review_session::ReviewStatus;

pub mod check_comments;
pub mod make_feedback_request_list;
pub mod send_agent_suggestions;
pub mod show_status;
pub mod write_user_feedback;

/// Holds a step's result, the Review Session's status and the step the agent
/// runs next.
#[derive(Debug)]
pub struct StepOutput<T> {
    /// Holds `None` when no Review Session exists, or when step 2 cannot read
    /// it.
    pub status: Option<ReviewStatus>,
    pub result: T,
    pub next: NextStep,
}

/// Names the step the agent runs next.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NextStep {
    CheckComments,
    MakeFeedbackRequestList,
    SendAgentSuggestions,
    /// Tells the user what to do, then waits, since only the user can act.
    TellUserAndWait,
    /// Tells the agent to end the review with nothing sent, since no comment
    /// needs User Feedback and no User Notes wait.
    FinishWithoutFeedbackRequests,
}
