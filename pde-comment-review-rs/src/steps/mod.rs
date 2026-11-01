//! Holds the steps of a review, one per file.

pub mod check_comments;
pub mod make_feedback_request_list;
pub mod show_status;
pub mod write_user_feedback;

/// Holds a step's result and the step the agent runs next.
#[derive(Debug)]
pub struct StepOutput<T> {
    pub result: T,
    pub next: NextStep,
}

/// Names the step the agent runs next.
#[derive(Debug, PartialEq)]
pub enum NextStep {
    CheckComments,
    MakeFeedbackRequestList,
    SendAgentSuggestions,
    /// Tells the user to press Ctrl+b e, then waits, since only the user can answer.
    TellUserAndWait,
}
