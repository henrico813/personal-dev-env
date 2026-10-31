//! Holds an answer or a request for the agent to try again.

/// Keeps an answer separate from a note that leaves the request unanswered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserFeedback {
    Answer(Answer),
    Note { note: String },
}

/// Records the user's choice without applying it to code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    Keep,
    Cut,
    Accept,
    Edited { text: String },
}
