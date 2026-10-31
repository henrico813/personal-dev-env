//! Carries failures that later commands turn into replies.

use std::fmt;

/// Carries a failure code and a message for the caller.
#[derive(Debug)]
pub struct Error {
    pub code: ErrorCode,
    pub message: String,
    source: Option<Box<dyn std::error::Error>>,
}

impl Error {
    /// Creates a failure with no underlying cause.
    pub fn new(code: ErrorCode, message: String) -> Self {
        Self {
            code,
            message,
            source: None,
        }
    }

    /// Keeps the underlying cause available for diagnostics.
    pub fn with_source(
        code: ErrorCode,
        message: String,
        source: impl std::error::Error + 'static,
    ) -> Self {
        Self {
            code,
            message,
            source: Some(Box::new(source)),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.as_deref()
    }
}

/// Identifies failures without making callers inspect messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    CheckFailed,
    WaitingForUserFeedback,
    ListChanged,
    MissingSuggestion,
    BadInput,
    RewordFailsRules,
    WouldNotWrite,
    TextChanged,
    NothingToWrite,
    WriteFailed,
    NotAGitCheckout,
    FileUnreadable,
    DirNotAbsolute,
    WriteNotFinished,
    SavedFileUnreadable,
    UnknownSavedField,
}
