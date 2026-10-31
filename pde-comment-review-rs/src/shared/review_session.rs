//! Holds review data without reading or saving it.

use std::collections::BTreeMap;
use std::path::PathBuf;

use super::ErrorCode;
use super::approvals::Conflict;
use super::git::CommitId;
use super::user_feedback::{Answer, UserFeedback};

/// Keeps every sent list and the latest feedback for each request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewSession {
    pub status: Status,
    pub origin: Origin,
    pub lists: Vec<FeedbackRequestList>,
    pub feedback: BTreeMap<RequestId, UserFeedback>,
    pub write_error: Option<WriteError>,
}

/// Keeps the failed write's code separate from its display detail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteError {
    pub code: ErrorCode,
    pub detail: String,
}

/// Identifies who acts next or whether the review is complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    WaitingForUserFeedback,
    UserNotesWaiting,
    WritingUserFeedback,
    Done,
}

/// Records where a review started without holding a live connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Origin {
    pub repo: String,
    pub branch: String,
    pub agent_session: String,
}

/// Retains the task start used for one sent group of requests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedbackRequestList {
    pub base: CommitId,
    pub requests: Vec<FeedbackRequest>,
}

/// Keeps the comment shown to the user beside the agent's suggestion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedbackRequest {
    pub id: RequestId,
    pub file: PathBuf,
    /// Leaves unnamed comments outside saved approvals.
    pub name: Option<String>,
    pub reason: Reason,
    pub text: String,
    pub can_reword: bool,
    /// Counts from one and helps locate an unnamed comment again.
    pub line: usize,
    pub suggestion: Option<AgentSuggestion>,
}

/// Identifies one request across every list in the same review.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RequestId(pub u32);

/// Carries extra context only for reasons that need it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reason {
    New,
    Renamed { old_name: String },
    ConflictsWithReview { conflict: Conflict },
    AgentWantsChange,
}

/// Keeps the agent's reasoning and other locations beside its advice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSuggestion {
    pub kind: SuggestionKind,
    pub reasoning: String,
    pub also_at: Vec<String>,
}

/// Carries proposed wording only when the agent suggests a reword.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SuggestionKind {
    Keep,
    Cut,
    Reword { text: String },
}

/// Links the old and new names without guessing from changed text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rename {
    pub old: String,
    pub new: String,
}

/// Names an approved comment without supplying replacement source text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeRequest {
    pub file: PathBuf,
    pub name: String,
}

/// Holds optional requests used when building a feedback list.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ListInput {
    pub renames: Vec<Rename>,
    pub change_requests: Vec<ChangeRequest>,
}

/// Pairs the newest request with an answer rather than a note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnsweredRequest {
    pub request: FeedbackRequest,
    pub answer: Answer,
}
