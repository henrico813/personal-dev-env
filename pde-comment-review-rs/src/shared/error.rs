//! Carries each failure or refusal as a code and a message.

use serde::{Deserialize, Serialize};

use crate::steps::NextStep;

/// Carries a failure code and a message for the caller.
#[derive(Debug, Serialize)]
pub struct Error {
    pub code: ErrorCode,
    pub message: String,
}

/// Identifies failures without making callers inspect messages.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    BadInput,
    NotAGitCheckout,
    FileUnreadable,
    SavedFileUnreadable,
    UnknownSavedField,
    SaveFailed,
    RequestChanged,
    TextChanged,
    LateUserFeedback,
    WouldNotWrite,
    CheckFailed,
    WaitingForUserFeedback,
    WriteNotFinished,
    NothingToWrite,
    WriteFailed,
    ListChanged,
    MissingSuggestion,
    RewordFailsRules,
    /// Marks an MCP call whose `dir` is not a full path.
    DirNotAbsolute,
}

impl ErrorCode {
    /// Returns the step the agent runs after this refusal, or `None` when only
    /// the message helps.
    pub fn next_step(self) -> Option<NextStep> {
        match self {
            Self::CheckFailed => Some(NextStep::CheckComments),
            Self::WaitingForUserFeedback | Self::WriteNotFinished => {
                Some(NextStep::TellUserAndWait)
            }
            Self::ListChanged => Some(NextStep::MakeFeedbackRequestList),
            Self::MissingSuggestion | Self::RewordFailsRules | Self::WouldNotWrite => {
                Some(NextStep::SendAgentSuggestions)
            }
            // Only the user can fix some files this refusal covers, so it names no step.
            // Each of its messages says who acts instead.
            Self::SavedFileUnreadable => None,
            Self::BadInput
            | Self::NotAGitCheckout
            | Self::FileUnreadable
            | Self::UnknownSavedField
            | Self::SaveFailed
            | Self::RequestChanged
            | Self::TextChanged
            | Self::LateUserFeedback
            | Self::NothingToWrite
            | Self::WriteFailed
            | Self::DirNotAbsolute => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `next_step` must name the step the design's error table gives each refusal, and none for the others.
    /// Otherwise an agent would call a step that refuses again, or build a new list after a fixable reword.
    #[test]
    fn next_step_follows_error_table() {
        let cases = [
            (ErrorCode::CheckFailed, Some(NextStep::CheckComments)),
            (
                ErrorCode::WaitingForUserFeedback,
                Some(NextStep::TellUserAndWait),
            ),
            (ErrorCode::WriteNotFinished, Some(NextStep::TellUserAndWait)),
            (
                ErrorCode::ListChanged,
                Some(NextStep::MakeFeedbackRequestList),
            ),
            (
                ErrorCode::MissingSuggestion,
                Some(NextStep::SendAgentSuggestions),
            ),
            (
                ErrorCode::RewordFailsRules,
                Some(NextStep::SendAgentSuggestions),
            ),
            (
                ErrorCode::WouldNotWrite,
                Some(NextStep::SendAgentSuggestions),
            ),
            (ErrorCode::BadInput, None),
            (ErrorCode::NotAGitCheckout, None),
            (ErrorCode::FileUnreadable, None),
            (ErrorCode::SavedFileUnreadable, None),
            (ErrorCode::UnknownSavedField, None),
            (ErrorCode::SaveFailed, None),
            (ErrorCode::RequestChanged, None),
            (ErrorCode::TextChanged, None),
            (ErrorCode::LateUserFeedback, None),
            (ErrorCode::NothingToWrite, None),
            (ErrorCode::WriteFailed, None),
            (ErrorCode::DirNotAbsolute, None),
        ];

        for (code, next) in cases {
            assert_eq!(code.next_step(), next, "{code:?}");
        }
    }
}
