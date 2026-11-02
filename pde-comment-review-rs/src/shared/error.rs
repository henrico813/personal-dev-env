//! Carries each failure or refusal as a code and a message.

use serde::{Deserialize, Serialize};

/// Carries a failure code and a message for the caller.
#[derive(Debug)]
pub struct Error {
    pub code: ErrorCode,
    pub message: String,
}

/// Identifies failures without making callers inspect messages.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
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
}
