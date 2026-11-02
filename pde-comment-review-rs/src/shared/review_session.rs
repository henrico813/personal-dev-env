//! This module loads and saves each worktree's Review Session under a lock,
//! so two screens lose no User Feedback. Read the
//! `save_user_feedback(dir, shown_request, feedback)`, `lock(dir)`, and
//! `load(dir)` functions to understand the module.

use std::collections::BTreeMap;
use std::fmt::Display;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::git::{self, CommitId};
use super::user_feedback::{UserAnswer, UserFeedback};
use super::{Error, ErrorCode};

const REVIEW_FILE: &str = "review.json";
const LOCK_FILE: &str = "review.json.lock";
const TEMP_FILE: &str = "review.json.tmp";
const UNREADABLE_FILE: &str = "review.json.unreadable";

/// Saves User Feedback unless the Review Session stopped waiting for it or the
/// Feedback Request changed since the screen showed it. Sets
/// `user_notes_waiting` once every Feedback Request has User Feedback and any
/// of it is a User Note.
pub fn save_user_feedback(
    dir: &Path,
    shown_request: &FeedbackRequest,
    feedback: UserFeedback,
) -> Result<ReviewStatus, Error> {
    let mut lock = lock(dir)?;
    let Some(review_session) = lock.review_session_mut() else {
        return Err(late_error("No Review Session takes User Feedback here."));
    };
    if review_session.status != ReviewStatus::WaitingForUserFeedback {
        return Err(late_error(
            "This Review Session no longer takes User Feedback.",
        ));
    }
    // A screen left open may still show an older version of this Feedback Request.
    let newest_request = review_session
        .lists
        .iter()
        .rev()
        .flat_map(|list| &list.requests)
        .find(|request| request.id == shown_request.id);
    if newest_request != Some(shown_request) {
        return Err(Error {
            code: ErrorCode::RequestChanged,
            message: "The Feedback Request changed after the screen showed it.".to_owned(),
        });
    }
    review_session.feedback.insert(shown_request.id, feedback);
    let every_request_has_feedback = review_session
        .lists
        .iter()
        .flat_map(|list| &list.requests)
        .all(|request| review_session.feedback.contains_key(&request.id));
    let any_note = review_session
        .feedback
        .values()
        .any(|feedback| matches!(feedback, UserFeedback::Note(_)));
    if every_request_has_feedback && any_note {
        review_session.status = ReviewStatus::UserNotesWaiting;
    }
    let status = review_session.status;
    lock.save()?;
    Ok(status)
}

/// Takes the lock and reads `review.json` again under it. Waits while any
/// other holder has the lock, even one in this process.
pub fn lock(dir: &Path) -> Result<ReviewSessionLock, Error> {
    let folder = git::find_worktree_git_folder(dir)?.join("pde");
    fs::create_dir_all(&folder).map_err(|error| save_error(&folder, error))?;
    // Each save renames over review.json, so the lock sits on a file that stays.
    let lock_path = folder.join(LOCK_FILE);
    let lock_file = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
        .map_err(|error| save_error(&lock_path, error))?;
    lock_file
        .lock()
        .map_err(|error| save_error(&lock_path, error))?;
    let review_session = read_review_session(&folder)?;
    Ok(ReviewSessionLock {
        _lock_file: lock_file,
        folder,
        review_session,
    })
}

/// Returns the saved Review Session, or `None` when there is none. Moves an
/// unreadable file aside, but leaves a newer version's file in place.
pub fn load(dir: &Path) -> Result<Option<ReviewSession>, Error> {
    Ok(lock(dir)?.review_session)
}

impl ReviewSessionLock {
    /// Returns the folder that holds `review.json`, in the worktree's own Git
    /// folder.
    pub(crate) fn folder(&self) -> &Path {
        &self.folder
    }

    /// Returns the Review Session to change, or `None` when there is none.
    pub fn review_session_mut(&mut self) -> Option<&mut ReviewSession> {
        self.review_session.as_mut()
    }

    /// Saves through a temporary file, so a crash leaves the old file whole.
    /// Saves nothing when there is no Review Session.
    pub fn save(&self) -> Result<(), Error> {
        let Some(review_session) = &self.review_session else {
            return Ok(());
        };
        let path = self.folder.join(REVIEW_FILE);
        let mut text =
            serde_json::to_vec_pretty(review_session).map_err(|error| save_error(&path, error))?;
        text.push(b'\n');
        // Only the lock holder writes this file, so one fixed name is enough.
        let temp_path = self.folder.join(TEMP_FILE);
        write_and_sync(&temp_path, &text).map_err(|error| save_error(&temp_path, error))?;
        fs::rename(&temp_path, &path).map_err(|error| save_error(&path, error))
    }
}

/// Holds one worktree's Review Session, saved in its own Git folder.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewSession {
    pub status: ReviewStatus,
    pub origin: ReviewOrigin,
    pub lists: Vec<FeedbackRequestList>,
    pub feedback: BTreeMap<RequestId, UserFeedback>,
}

impl ReviewSession {
    /// Returns the newest version of each Feedback Request, in ID order.
    pub(crate) fn newest_requests(&self) -> Vec<&FeedbackRequest> {
        let mut newest = BTreeMap::new();
        // Lists are kept in the order they were sent, so a later version replaces an older one.
        for request in self.lists.iter().flat_map(|list| &list.requests) {
            newest.insert(request.id, request);
        }
        newest.into_values().collect()
    }

    /// Returns every Answered Request, or `None` while any Feedback Request in
    /// its newest version is not answered.
    pub(crate) fn answered_requests(&self) -> Option<Vec<AnsweredRequest<'_>>> {
        self.newest_requests()
            .into_iter()
            .map(|request| match self.feedback.get(&request.id) {
                Some(UserFeedback::Answer(answer)) => Some(AnsweredRequest { request, answer }),
                _ => None,
            })
            .collect()
    }
}

/// Holds an Answered Request, a Feedback Request in its newest version with the
/// User Feedback that answers it.
pub(crate) struct AnsweredRequest<'review> {
    pub(crate) request: &'review FeedbackRequest,
    pub(crate) answer: &'review UserAnswer,
}

/// Says where the Review Session is.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStatus {
    WaitingForUserFeedback,
    UserNotesWaiting,
    /// Marks a write into the code that is running, crashed or could not be undone.
    WritingUserFeedback,
    Done,
}

/// Names the repository, branch and agent session the Review Session came from.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewOrigin {
    pub repo: String,
    pub branch: String,
    pub agent_session: String,
}

/// Holds one Feedback Request List as it was sent.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeedbackRequestList {
    /// Names the commit the list's Feedback Requests came from.
    pub base: CommitId,
    pub requests: Vec<FeedbackRequest>,
}

/// Asks the user about one comment.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeedbackRequest {
    pub id: RequestId,
    pub file: PathBuf,
    /// Holds `None` for an unnamed comment, which saves no approval.
    pub name: Option<String>,
    pub reason: RequestReason,
    pub text: String,
    pub can_reword: bool,
    /// Finds an unnamed comment again when its Feedback Request is sent back.
    pub line: usize,
    pub suggestion: Option<AgentSuggestion>,
}

/// Numbers a Feedback Request the same way in every Feedback Request List.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RequestId(pub(crate) u32);

/// Says why the comment needs User Feedback.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestReason {
    New,
    /// Uses a name whose comment the user once cut.
    WasCut,
    /// Differs from the text the user approved for this name.
    DiffersFromApproved,
    /// Asks to reword a comment the user approved.
    AgentWantsChange,
}

/// Gives the agent's advice on one comment and why.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentSuggestion {
    pub kind: SuggestionKind,
    pub reasoning: String,
    pub also_at: Vec<String>,
}

/// Names what the agent suggests doing with the comment.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum SuggestionKind {
    Keep,
    Cut,
    Reword { text: String },
}

/// Holds the lock until dropped, with the Review Session read under it.
pub struct ReviewSessionLock {
    _lock_file: File,
    folder: PathBuf,
    review_session: Option<ReviewSession>,
}

fn read_review_session(folder: &Path) -> Result<Option<ReviewSession>, Error> {
    let path = folder.join(REVIEW_FILE);
    let saved = match fs::read(&path) {
        Ok(saved) => saved,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(read_error(&path, error)),
    };
    let error = match serde_json::from_slice(&saved) {
        Ok(review_session) => return Ok(Some(review_session)),
        Err(error) => error,
    };
    // Moving a newer version's file aside would lose its Review Session.
    if is_from_newer_version(&error) {
        return Err(Error {
            code: ErrorCode::UnknownSavedField,
            message: format!(
                "{} holds a field or value this version does not know. Run pde-installer to update it. {error}",
                path.display()
            ),
        });
    }
    let unreadable_path = find_free_unreadable_path(folder);
    fs::rename(&path, &unreadable_path).map_err(|error| save_error(&path, error))?;
    Err(read_error(
        &path,
        format!("Moved it to {}. {error}", unreadable_path.display()),
    ))
}

/// Treats an unknown field or value as a newer version's file. A later version
/// may only add those, or this version moves its file aside as damaged.
fn is_from_newer_version(error: &serde_json::Error) -> bool {
    // serde_json gives these errors no kind of their own, so this reads the message.
    let message = error.to_string();
    error.is_data()
        && (message.starts_with("unknown field") || message.starts_with("unknown variant"))
}

/// Returns the first unused backup name, so an earlier backup is never replaced.
fn find_free_unreadable_path(folder: &Path) -> PathBuf {
    let mut path = folder.join(UNREADABLE_FILE);
    let mut number = 2;
    while path.exists() {
        path = folder.join(format!("{UNREADABLE_FILE}.{number}"));
        number += 1;
    }
    path
}

fn write_and_sync(path: &Path, contents: &[u8]) -> io::Result<()> {
    let mut file = File::create(path)?;
    file.write_all(contents)?;
    file.sync_all()
}

fn read_error(path: &Path, error: impl Display) -> Error {
    Error {
        code: ErrorCode::SavedFileUnreadable,
        message: format!(
            "Cannot read the Review Session in {}. {error}",
            path.display()
        ),
    }
}

fn save_error(path: &Path, error: impl Display) -> Error {
    Error {
        code: ErrorCode::SaveFailed,
        message: format!(
            "Cannot save the Review Session in {}. {error}",
            path.display()
        ),
    }
}

fn late_error(message: &str) -> Error {
    Error {
        code: ErrorCode::LateUserFeedback,
        message: message.to_owned(),
    }
}
