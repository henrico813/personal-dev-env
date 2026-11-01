//! This module runs step 3, which builds the Feedback Request List. It saves
//! nothing, so a later step that sends the Agent Suggestions can build the
//! list again to see whether the code changed. Read the
//! `make_feedback_request_list(dir, base, change_requests)`,
//! `find_requests(dir, root, base, change_requests)`, and
//! `resend_noted_requests(root, review, found)` functions to understand the
//! module.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use super::check_comments::check_comments;
use super::{NextStep, StepOutput};
use crate::shared::approvals::{self, Approvals, DecisionMatch};
use crate::shared::comments::{self, Comment, FileComments};
use crate::shared::git::{self, CommitId};
use crate::shared::review_session::{
    self, FeedbackRequest, FeedbackRequestList, RequestId, RequestReason, ReviewSession,
    ReviewStatus,
};
use crate::shared::user_feedback::UserFeedback;
use crate::shared::{Error, ErrorCode};

/// Returns the Feedback Requests for changed or conflicting comments and change
/// requests, and resends noted ones with their IDs while User Notes wait.
/// Refuses while a list waits, a write is unfinished, step 2 fails, or a
/// committed or staged comment goes unasked.
pub fn make_feedback_request_list(
    dir: &Path,
    base: Option<CommitId>,
    change_requests: &[ChangeRequest],
) -> Result<StepOutput<FeedbackRequestListResult>, Error> {
    let review = review_session::load(dir)?;
    match review.as_ref().map(|review| review.status) {
        Some(ReviewStatus::WaitingForUserFeedback) => {
            return Err(Error {
                code: ErrorCode::WaitingForUserFeedback,
                message: "A Feedback Request List waits for the user. Tell them to press Ctrl+b e, then wait."
                    .to_owned(),
            });
        }
        Some(ReviewStatus::WritingUserFeedback) => {
            return Err(Error {
                code: ErrorCode::WriteNotFinished,
                message: "A write into the code did not finish. Ask the user to check git status."
                    .to_owned(),
            });
        }
        _ => {}
    }
    let noted_review = review.filter(|review| review.status == ReviewStatus::UserNotesWaiting);
    // While User Notes wait, the Review Session keeps its task's base, so a
    // stacked branch is not checked against its parent.
    let saved_base = noted_review
        .as_ref()
        .and_then(|review| review.lists.last())
        .map(|list| list.base.clone());
    if let (Some(base), Some(saved_base)) = (&base, &saved_base)
        && base != saved_base
    {
        return Err(Error {
            code: ErrorCode::BadInput,
            message: format!(
                "Pass the base the review started from, {}, or none.",
                saved_base.as_str()
            ),
        });
    }
    let base = match base.or(saved_base) {
        Some(base) => base,
        None => git::find_branch_start(dir)?,
    };
    let check = check_comments(dir, Some(base.clone()))?;
    if !check.result.failures.is_empty() {
        return Err(Error {
            code: ErrorCode::CheckFailed,
            message: "Step 2 still fails. Fix what check_comments lists, then call again."
                .to_owned(),
        });
    }
    let root = git::find_root(dir)?;
    let found = find_requests(dir, &root, &base, change_requests)?;
    let requests = match noted_review {
        Some(review) => resend_noted_requests(&root, &review, found)?,
        // A new Review Session numbers from 1, since a finished one's User
        // Feedback never carries over.
        None => number_requests(found.into_iter().map(|found| found.request).collect(), 1),
    };
    Ok(StepOutput {
        result: FeedbackRequestListResult {
            list: FeedbackRequestList { base, requests },
            unsupported: check.result.unsupported,
        },
        next: NextStep::SendAgentSuggestions,
    })
}

/// Returns a request for each comment that is changed since `base` or goes against
/// a decision, and for each change request. Their IDs are not set yet.
fn find_requests(
    dir: &Path,
    root: &Path,
    base: &CommitId,
    change_requests: &[ChangeRequest],
) -> Result<Vec<FoundRequest>, Error> {
    let approvals = approvals::load(dir)?;
    let changed = git::list_changed_files(dir, base)?;
    let files: BTreeSet<&Path> = changed
        .iter()
        .map(PathBuf::as_path)
        .chain(approvals.files())
        .collect();
    let mut found = Vec::new();
    for file in files {
        if !comments::is_supported(file) {
            continue;
        }
        let now = comments::read_comments_on_disk(root, file)?;
        let before = comments::read_comments_at_commit(root, base, file)?;
        let mut file_found = Vec::new();
        for comment in &now.comments {
            if let Some(reason) = find_reason(&approvals, &before, comment) {
                file_found.push(FoundRequest {
                    request: to_request(comment, reason),
                    start: comment.byte_range.start,
                });
            }
        }
        // Commits and the index ship too, but User Feedback is written into the disk copy.
        for version in git::read_changed_versions(root, base, file)? {
            let place = version.place;
            let shipped = comments::read_version_comments(file, version)?;
            let is_unasked = shipped.comments.iter().any(|comment| {
                find_reason(&approvals, &before, comment).is_some()
                    && !file_found
                        .iter()
                        .any(|found| is_asked_as(&found.request, comment))
            });
            if is_unasked {
                return Err(Error {
                    code: ErrorCode::BadInput,
                    message: format!(
                        "{} {place} holds a comment that needs User Feedback, but the disk copy does not ask about it. Make the committed, staged and disk copies agree, then call again.",
                        file.display()
                    ),
                });
            }
        }
        found.extend(file_found);
    }
    for change_request in change_requests {
        let change = to_change_request(root, &approvals, change_request)?;
        // Two requests for one comment would leave two pieces of User Feedback to write into it.
        if !found.iter().any(|listed| {
            listed.request.file == change.request.file && listed.request.name == change.request.name
        }) {
            found.push(change);
        }
    }
    Ok(found)
}

/// Returns each noted request read again from the code, keeping its ID, and
/// each found request whose comment no request in the Review Session holds,
/// under new IDs.
fn resend_noted_requests(
    root: &Path,
    review: &ReviewSession,
    found: Vec<FoundRequest>,
) -> Result<Vec<FeedbackRequest>, Error> {
    let sent = review.newest_requests();
    let files: BTreeSet<&Path> = sent.iter().map(|request| request.file.as_path()).collect();
    let mut requests = Vec::new();
    let mut held = BTreeSet::new();
    for file in files {
        let in_file: Vec<&FeedbackRequest> = sent
            .iter()
            .copied()
            .filter(|request| request.file == file)
            .collect();
        let now = comments::read_comments_on_disk(root, file)?;
        for (request, comment) in in_file.iter().zip(now.match_requests(&in_file)) {
            // A request whose comment is gone has nothing left to ask about.
            let Some(comment) = comment else {
                continue;
            };
            let is_noted = matches!(
                review.feedback.get(&request.id),
                Some(UserFeedback::Note(_))
            );
            if is_noted {
                requests.push(FeedbackRequest {
                    id: request.id,
                    ..to_request(comment, request.reason)
                });
            }
            // User Feedback covers only the text the user saw, so a comment
            // reworded since is asked again.
            if is_noted || comment.text == request.text {
                held.insert((file, comment.byte_range.start));
            }
        }
    }
    // A comment the agent wrote while answering User Notes must still reach the user.
    let unsent: Vec<FeedbackRequest> = found
        .into_iter()
        .filter(|found| !held.contains(&(found.request.file.as_path(), found.start)))
        .map(|found| found.request)
        .collect();
    let next_id = sent.iter().map(|request| request.id.0).max().unwrap_or(0) + 1;
    requests.extend(number_requests(unsent, next_id));
    requests.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
    Ok(requests)
}

/// Names an approved comment the agent wants the user to reword. It holds no
/// text, so the request always shows the approved text from disk.
#[derive(Debug)]
pub struct ChangeRequest {
    pub file: PathBuf,
    pub name: String,
}

/// Holds step 3's Feedback Request List and the changed files no language
/// rule reads.
#[derive(Debug)]
pub struct FeedbackRequestListResult {
    pub list: FeedbackRequestList,
    pub unsupported: Vec<PathBuf>,
}

/// Holds a request and the byte where its comment starts on disk, which tells
/// two comments on one line apart.
struct FoundRequest {
    request: FeedbackRequest,
    start: usize,
}

/// Returns true when the request asks about the comment's name, or about its
/// text when it has no name.
fn is_asked_as(request: &FeedbackRequest, comment: &Comment) -> bool {
    request.name == comment.name && (comment.name.is_some() || request.text == comment.text)
}

/// Returns why the comment needs User Feedback, or `None` when it does not.
fn find_reason(
    approvals: &Approvals,
    before: &FileComments,
    comment: &Comment,
) -> Option<RequestReason> {
    match approvals.compare(comment) {
        DecisionMatch::Undecided if !before.has_same(comment) => Some(RequestReason::New),
        DecisionMatch::Undecided | DecisionMatch::Approved => None,
        DecisionMatch::WasCut => Some(RequestReason::WasCut),
        // A changed comment would already have failed step 2, so this one has not changed.
        DecisionMatch::DiffersFromApproved => Some(RequestReason::DiffersFromApproved),
    }
}

/// Returns the request for an approved comment the user can reword, or refuses with `BadInput`.
fn to_change_request(
    root: &Path,
    approvals: &Approvals,
    change_request: &ChangeRequest,
) -> Result<FoundRequest, Error> {
    let now = comments::read_comments_on_disk(root, &change_request.file)?;
    let comment = now
        .comments
        .iter()
        .find(|comment| comment.name.as_ref() == Some(&change_request.name));
    match comment {
        Some(comment)
            if approvals.compare(comment) == DecisionMatch::Approved
                && comment.style.can_reword() =>
        {
            Ok(FoundRequest {
                request: to_request(comment, RequestReason::AgentWantsChange),
                start: comment.byte_range.start,
            })
        }
        _ => Err(Error {
            code: ErrorCode::BadInput,
            message: format!(
                "Ask to change only an approved comment the user can reword. {} in {} is not one.",
                change_request.name,
                change_request.file.display()
            ),
        }),
    }
}

/// Sorts requests in file and line order and numbers them from `first`, so
/// the same code always gets the same IDs.
fn number_requests(mut requests: Vec<FeedbackRequest>, first: u32) -> Vec<FeedbackRequest> {
    requests.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
    for (request, id) in requests.iter_mut().zip(first..) {
        request.id = RequestId(id);
    }
    requests
}

fn to_request(comment: &Comment, reason: RequestReason) -> FeedbackRequest {
    FeedbackRequest {
        // Callers set the real ID, which depends on the whole list.
        id: RequestId(0),
        file: comment.file.clone(),
        name: comment.name.clone(),
        reason,
        text: comment.text.clone(),
        can_reword: comment.style.can_reword(),
        line: comment.line,
        suggestion: None,
    }
}
