//! Holds the user's answers and notes on Feedback Requests.

use serde::{Deserialize, Serialize};

/// Holds either an answer or a User Note for one Feedback Request.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum UserFeedback {
    Answer(UserAnswer),
    Note(UserNote),
}

/// Says what to do with the comment.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum UserAnswer {
    Keep,
    Cut,
    /// Takes the Agent Suggestion.
    Accept,
    /// Uses the user's own wording.
    Edited {
        text: String,
    },
}

/// Asks the agent to try again instead of answering.
#[derive(Debug, Serialize, Deserialize)]
pub struct UserNote(pub String);
