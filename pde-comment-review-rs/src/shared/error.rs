//! Carries Git and saved-file failures as a code and a message.

/// Carries a failure code and a message for the caller.
#[derive(Debug)]
pub struct Error {
    pub code: ErrorCode,
    pub message: String,
}

/// Identifies failures without making callers inspect messages.
#[derive(Debug, PartialEq)]
pub enum ErrorCode {
    BadInput,
    NotAGitCheckout,
    FileUnreadable,
    SavedFileUnreadable,
    UnknownSavedField,
    SaveFailed,
}
