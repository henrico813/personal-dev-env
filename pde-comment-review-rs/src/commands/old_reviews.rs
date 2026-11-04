//! This module moves each open review of the old Go review tool into its
//! worktree's Review Session, keeping the User Feedback already given. Read
//! the `read_open_reviews(worktrees)`, `match_open_review(review)`, and
//! `save_matched_review(matched, approvals)` functions to understand the
//! module.

use std::collections::BTreeMap;
use std::fmt::Display;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::shared::approvals::{Approvals, DecisionMatch};
use crate::shared::comments::{self, Comment};
use crate::shared::git;
use crate::shared::review_session::{
    self, AgentSuggestion, FeedbackRequest, FeedbackRequestList, RequestId, RequestReason,
    ReviewSession, ReviewStatus, SuggestionKind,
};
use crate::shared::user_feedback::{UserAnswer, UserFeedback, UserNote};
use crate::shared::{Error, ErrorCode};

/// Returns the old tool's reviews that still wait, split into those for one
/// of `worktrees` and the worktree folders of those for other clones.
pub(crate) fn read_open_reviews(worktrees: &[PathBuf]) -> Result<OpenReviews, Error> {
    let state_folder = review_session::find_pde_state_folder()?.join("comment-review");
    let mut open_reviews = OpenReviews::default();
    let entries = match fs::read_dir(&state_folder) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(open_reviews),
        Err(error) => return Err(read_error(&state_folder, error)),
    };
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| read_error(&state_folder, error))?;
        let is_folder = entry
            .file_type()
            .map_err(|error| read_error(&entry.path(), error))?
            .is_dir();
        if let Some(name) = entry.file_name().to_str()
            && is_folder
            && is_review_name(name)
        {
            names.push(name.to_owned());
        }
    }
    names.sort();
    let canonical_worktrees: Vec<(PathBuf, PathBuf)> = worktrees
        .iter()
        .filter_map(|worktree| Some((fs::canonicalize(worktree).ok()?, worktree.clone())))
        .collect();
    for name in names {
        let folder = state_folder.join(&name);
        let rounds = read_rounds(&folder)?;
        let applied: OldApplied = read_json_file(&folder.join("applied.json"))?.unwrap_or_default();
        let Some(first_round) = rounds.first() else {
            continue;
        };
        if applied.round == rounds.len() {
            continue;
        }
        // The old tool hides a review whose worktree is gone, since it can never be written.
        let Ok(repo) = fs::canonicalize(&first_round.repo) else {
            continue;
        };
        let Some((_, worktree)) = canonical_worktrees
            .iter()
            .find(|(canonical, _)| *canonical == repo)
        else {
            open_reviews.in_other_clones.push(first_round.repo.clone());
            continue;
        };
        let notes: BTreeMap<u32, OldNote> =
            read_json_file(&folder.join("notes.json"))?.unwrap_or_default();
        let answers: BTreeMap<u32, OldAnswer> =
            read_json_file(&folder.join("decisions.json"))?.unwrap_or_default();
        let origin: OldOrigin = read_json_file(&folder.join("origin.json"))?.unwrap_or_default();
        open_reviews.in_this_clone.push(collect_newest_requests(
            name,
            worktree.clone(),
            origin,
            rounds,
            notes,
            answers,
        ));
    }
    Ok(open_reviews)
}

/// Finds the comment each old Feedback Request asked about in the worktree's
/// files on disk. Leaves out a request whose comment is gone or changed since
/// it was sent, with its User Feedback.
pub(crate) fn match_open_review(review: OpenReview) -> Result<MatchedReview, Error> {
    let mut files = BTreeMap::new();
    let mut matched = MatchedReview {
        name: review.name,
        worktree: review.worktree,
        agent_session: review.origin.session,
        requests: Vec::new(),
        left_out: Vec::new(),
        notes_left_behind: review.notes_left_behind,
    };
    for (old, user_feedback) in review.requests {
        let comment = if old.code_removed {
            None
        } else {
            if !files.contains_key(&old.file) {
                let found = comments::read_comments_on_disk(&matched.worktree, &old.file)?;
                files.insert(old.file.clone(), found.comments);
            }
            find_comment(&files[&old.file], &old).cloned()
        };
        match comment {
            Some(comment) => matched.requests.push(MatchedRequest {
                old,
                comment,
                user_feedback,
            }),
            None => matched.left_out.push(LeftOutRequest {
                id: old.id,
                file: old.file,
                name: (!old.name.is_empty()).then_some(old.name),
                user_feedback,
            }),
        }
    }
    Ok(matched)
}

/// Saves the matched review as the worktree's Review Session, and puts one
/// that waits for the user on the waiting list. A failure becomes the
/// review's outcome, so the move goes on with the next review.
pub(crate) fn save_matched_review(matched: MatchedReview, approvals: &Approvals) -> OldReviewMove {
    let name = matched.name.clone();
    let worktree = matched.worktree.clone();
    let outcome = save_as_review_session(matched, approvals)
        .unwrap_or_else(|error| OldReviewOutcome::NotSaved { error });
    OldReviewMove {
        name,
        worktree,
        outcome,
    }
}

/// Holds the old tool's reviews that still wait.
#[derive(Default)]
pub(crate) struct OpenReviews {
    pub(crate) in_this_clone: Vec<OpenReview>,
    /// Holds the worktree folder of each open review for another clone.
    pub(crate) in_other_clones: Vec<PathBuf>,
}

/// Holds one open review of the old tool, with the newest version of each of
/// its Feedback Requests and that version's User Feedback.
pub(crate) struct OpenReview {
    name: String,
    worktree: PathBuf,
    origin: OldOrigin,
    requests: Vec<(OldRequest, Option<UserFeedback>)>,
    notes_left_behind: usize,
}

/// Holds an open review whose requests were found in the code, ready to save.
pub(crate) struct MatchedReview {
    name: String,
    worktree: PathBuf,
    agent_session: String,
    requests: Vec<MatchedRequest>,
    left_out: Vec<LeftOutRequest>,
    notes_left_behind: usize,
}

/// Holds an old Feedback Request with the comment it asked about, as the code
/// holds it now.
struct MatchedRequest {
    old: OldRequest,
    comment: Comment,
    user_feedback: Option<UserFeedback>,
}

/// Says what the move did with one open review of the old tool.
#[derive(Debug, Serialize)]
pub struct OldReviewMove {
    /// Names the review's folder in the old tool.
    pub name: String,
    pub worktree: PathBuf,
    pub outcome: OldReviewOutcome,
}

/// Says whether an open review became the worktree's Review Session.
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OldReviewOutcome {
    Moved {
        status: ReviewStatus,
        requests: usize,
        answers: usize,
        notes: usize,
        /// Counts User Notes on requests that also have an answer, since a
        /// Feedback Request keeps one piece of User Feedback.
        notes_left_behind: usize,
        left_out: Vec<LeftOutRequest>,
    },
    /// Leaves the worktree's own Review Session as it was.
    AlreadyHasReviewSession,
    /// Saves nothing, since every request's comment is gone or changed.
    NothingLeftToMove { left_out: Vec<LeftOutRequest> },
    /// Saves no Review Session, because reading the worktree, adding it to
    /// the waiting list or saving its Review Session failed.
    NotSaved { error: Error },
}

/// Holds an old Feedback Request whose comment is gone or changed since it
/// was sent, with its User Feedback, so the user can give it again.
#[derive(Debug, Serialize)]
pub struct LeftOutRequest {
    pub id: u32,
    pub file: PathBuf,
    pub name: Option<String>,
    pub user_feedback: Option<UserFeedback>,
}

/// Holds one `round-N.json` of the old tool, a Feedback Request List.
#[derive(Deserialize)]
struct OldRound {
    repo: PathBuf,
    items: Vec<OldRequest>,
}

/// Holds one Feedback Request as the old tool saved it. `old` is the
/// comment's whole lines, and `new` a reword's whole lines.
#[derive(Clone, Deserialize)]
struct OldRequest {
    id: u32,
    file: PathBuf,
    old: String,
    proposal: OldSuggestion,
    #[serde(default)]
    new: String,
    #[serde(default)]
    why: String,
    #[serde(default)]
    also_at: Vec<String>,
    #[serde(default)]
    name: String,
    #[serde(default)]
    code_removed: bool,
}

/// Names the old tool's Agent Suggestion.
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum OldSuggestion {
    Keep,
    Cut,
    Reword,
}

/// Holds a User Note from the old tool's `notes.json`, for one round.
#[derive(Deserialize)]
struct OldNote {
    text: String,
    round: usize,
}

/// Holds an answer from the old tool's `decisions.json`, for one round. An
/// edited answer's `text` is the comment's whole lines.
#[derive(Deserialize)]
struct OldAnswer {
    action: OldAction,
    #[serde(default)]
    text: String,
    round: usize,
}

/// Names the old tool's answer.
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum OldAction {
    Keep,
    Cut,
    Accept,
    Edited,
}

/// Holds the old tool's `applied.json`, the round last written into the code.
#[derive(Default, Deserialize)]
struct OldApplied {
    round: usize,
}

/// Holds the old tool's `origin.json`, of which the move keeps the agent session.
#[derive(Default, Deserialize)]
struct OldOrigin {
    #[serde(default)]
    session: String,
}

/// Returns true for a folder name the old tool reads as a review, such as
/// `vibe-add-tax`.
fn is_review_name(name: &str) -> bool {
    let mut characters = name.chars();
    let is_allowed = |character: char| character.is_ascii_lowercase() || character.is_ascii_digit();
    characters.next().is_some_and(is_allowed)
        && characters.all(|character| is_allowed(character) || "._-".contains(character))
}

/// Reads `round-1.json`, `round-2.json` and so on, up to the first one missing.
fn read_rounds(folder: &Path) -> Result<Vec<OldRound>, Error> {
    let mut rounds = Vec::new();
    while let Some(round) =
        read_json_file(&folder.join(format!("round-{}.json", rounds.len() + 1)))?
    {
        rounds.push(round);
    }
    Ok(rounds)
}

/// Returns the newest version of each Feedback Request, in ID order, with the
/// User Feedback given on that version. An answer wins over a User Note, as the
/// old tool writes the answer.
fn collect_newest_requests(
    name: String,
    worktree: PathBuf,
    origin: OldOrigin,
    rounds: Vec<OldRound>,
    notes: BTreeMap<u32, OldNote>,
    answers: BTreeMap<u32, OldAnswer>,
) -> OpenReview {
    let mut newest = BTreeMap::new();
    for (index, round) in rounds.into_iter().enumerate() {
        for request in round.items {
            newest.insert(request.id, (index + 1, request));
        }
    }
    let mut notes_left_behind = 0;
    let mut requests = Vec::new();
    for (id, (round, request)) in newest {
        let answer = answers.get(&id).filter(|answer| answer.round == round);
        let note = notes.get(&id).filter(|note| note.round == round);
        let user_feedback = match (answer, note) {
            (Some(answer), note) => {
                notes_left_behind += usize::from(note.is_some());
                Some(UserFeedback::Answer(to_user_answer(&request.file, answer)))
            }
            (None, Some(note)) => Some(UserFeedback::Note(UserNote(note.text.clone()))),
            (None, None) => None,
        };
        requests.push((request, user_feedback));
    }
    OpenReview {
        name,
        worktree,
        origin,
        requests,
        notes_left_behind,
    }
}

/// Returns the comment the old request asked about, by name, or by its text
/// when it has no name, and only while its text is the one the user saw.
fn find_comment<'a>(comments: &'a [Comment], old: &OldRequest) -> Option<&'a Comment> {
    let old_text = old.old.replace("\r\n", "\n");
    let has_text = |comment: &Comment| comment.text.replace("\r\n", "\n") == old_text;
    let mut candidates = comments.iter().filter(|comment| {
        if old.name.is_empty() {
            has_text(comment)
        } else {
            comment.name.as_deref() == Some(old.name.as_str())
        }
    });
    let comment = candidates.next()?;
    // Two comments with the same text leave no way to tell which one the user saw.
    if candidates.next().is_some() {
        return None;
    }
    has_text(comment).then_some(comment)
}

/// Saves the matched requests as the worktree's Review Session, each with its
/// reason from `approvals`, unless the worktree already has one.
fn save_as_review_session(
    matched: MatchedReview,
    approvals: &Approvals,
) -> Result<OldReviewOutcome, Error> {
    let MatchedReview {
        worktree,
        agent_session,
        requests: matched_requests,
        left_out,
        notes_left_behind,
        ..
    } = matched;
    if matched_requests.is_empty() {
        return Ok(OldReviewOutcome::NothingLeftToMove { left_out });
    }
    let mut requests = Vec::new();
    let mut feedback = BTreeMap::new();
    for MatchedRequest {
        old,
        comment,
        user_feedback,
    } in matched_requests
    {
        let id = RequestId(old.id);
        requests.push(FeedbackRequest {
            id,
            file: comment.file.clone(),
            name: comment.name.clone(),
            reason: find_reason(approvals.compare(&comment)),
            text: comment.text.replace("\r\n", "\n"),
            can_reword: comment.style.can_reword(),
            line: comment.line,
            suggestion: Some(AgentSuggestion {
                kind: to_suggestion_kind(&old),
                reasoning: old.why,
                also_at: old.also_at,
            }),
        });
        if let Some(user_feedback) = user_feedback {
            feedback.insert(id, user_feedback);
        }
    }
    let notes = feedback
        .values()
        .filter(|user_feedback| matches!(user_feedback, UserFeedback::Note(_)))
        .count();
    let answers = feedback.len() - notes;
    let status = if feedback.len() == requests.len() && notes > 0 {
        ReviewStatus::UserNotesWaiting
    } else {
        ReviewStatus::WaitingForUserFeedback
    };
    let base =
        git::find_branch_start(&worktree).or_else(|_| git::resolve_commit(&worktree, "HEAD"))?;
    let origin = review_session::ReviewOrigin {
        agent_session,
        ..review_session::find_review_origin(&worktree)?
    };
    let request_count = requests.len();
    let review = ReviewSession {
        status,
        origin,
        lists: vec![FeedbackRequestList { base, requests }],
        feedback,
        write_error: None,
    };
    // Ctrl+b e skips a listed worktree with no Review Session, so listing it
    // first means a move stopped between the two never hides the review.
    if status == ReviewStatus::WaitingForUserFeedback {
        review_session::add_to_waiting_list(&worktree)?;
    }
    if !review_session::add_moved_review_session(&worktree, review)? {
        return Ok(OldReviewOutcome::AlreadyHasReviewSession);
    }
    Ok(OldReviewOutcome::Moved {
        status,
        requests: request_count,
        answers,
        notes,
        notes_left_behind,
        left_out,
    })
}

/// Returns this program's answer for an old answer, with an edited text turned
/// into plain words.
fn to_user_answer(file: &Path, answer: &OldAnswer) -> UserAnswer {
    match answer.action {
        OldAction::Keep => UserAnswer::Keep,
        OldAction::Cut => UserAnswer::Cut,
        OldAction::Accept => UserAnswer::Accept,
        OldAction::Edited => UserAnswer::Edited {
            text: comments::to_plain_words(file, &answer.text),
        },
    }
}

/// Returns this program's Agent Suggestion for an old one, with a reword turned
/// into plain words.
fn to_suggestion_kind(old: &OldRequest) -> SuggestionKind {
    match old.proposal {
        OldSuggestion::Keep => SuggestionKind::Keep,
        OldSuggestion::Cut => SuggestionKind::Cut,
        OldSuggestion::Reword => SuggestionKind::Reword {
            text: comments::to_plain_words(&old.file, &old.new),
        },
    }
}

/// Returns the reason step 3 would give for a comment with this decision. An
/// approved comment was asked about because the agent wanted to change it.
fn find_reason(decision: DecisionMatch) -> RequestReason {
    match decision {
        DecisionMatch::Undecided => RequestReason::New,
        DecisionMatch::Approved => RequestReason::AgentWantsChange,
        DecisionMatch::WasCut => RequestReason::WasCut,
        DecisionMatch::DiffersFromApproved => RequestReason::DiffersFromApproved,
    }
}

/// Reads a JSON file of the old tool, or `None` when it is missing.
fn read_json_file<T: DeserializeOwned>(path: &Path) -> Result<Option<T>, Error> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(read_error(path, error)),
    };
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|error| read_error(path, error))
}

fn read_error(path: &Path, error: impl Display) -> Error {
    Error {
        code: ErrorCode::SavedFileUnreadable,
        message: format!(
            "Cannot read the old review tool's {}. Fix or move the file, then run the move again. {error}",
            path.display()
        ),
    }
}
