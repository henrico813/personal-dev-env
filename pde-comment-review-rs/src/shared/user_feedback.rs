//! Holds the kinds of User Feedback, an answer or a User Note.

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
