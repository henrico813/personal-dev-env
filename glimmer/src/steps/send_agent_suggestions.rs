//! This module runs step 5, which checks the agent's Feedback Request List and
//! tries its Agent Suggestions in memory as step 7 would write them, so only a
//! list the user can answer reaches the screen. Read the
//! `send_agent_suggestions(dir, list)` and
//! `find_reword_failures(root, approvals, list)` functions to understand the
//! module.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;

use super::make_feedback_request_list::{ChangeRequest, make_feedback_request_list};
use super::write_user_feedback::check_suggestions_can_be_written;
use super::{NextStep, StepOutput};
use crate::shared::approvals::{self, Approvals};
use crate::shared::comments;
use crate::shared::git;
use crate::shared::review_session::{
    self, FeedbackRequest, FeedbackRequestList, RequestReason, ReviewStatus, SuggestionKind,
};
use crate::shared::style_rules::{Finding, Outcome, StyleCheck, TextAtBase};
use crate::shared::user_feedback::{UserAnswer, choose_new_text};
use crate::shared::{Error, ErrorCode};

/// Saves the list with its Agent Suggestions and sends it to the user, once
/// every check passes.
///
/// # Errors
///
/// Refuses with `ListChanged` when step 3 would now build another list or the
/// Review Session changed meanwhile. Refuses with `MissingSuggestion`,
/// `BadInput`, `RewordFailsRules` or `WouldNotWrite` when an Agent Suggestion
/// is missing or the user could not take it as it is. Step 3's refusals, such
/// as `WaitingForUserFeedback`, pass through.
pub fn send_agent_suggestions(
    dir: &Path,
    list: FeedbackRequestList,
) -> Result<StepOutput<SendResult>, Error> {
    // `add_list` refuses the list if another call changes the Review Session after this.
    let seen = review_session::see_review_session(dir)?;
    // `FeedbackRequestList` has no `ChangeRequest` field, so this rebuilds them from the list's `agent_wants_change` requests.
    let change_requests: Vec<ChangeRequest> = list
        .requests
        .iter()
        .filter(|request| request.reason == RequestReason::AgentWantsChange)
        .filter_map(|request| {
            Some(ChangeRequest {
                file: request.file.clone(),
                name: request.name.clone()?,
            })
        })
        .collect();
    let fresh = make_feedback_request_list(dir, Some(list.base.clone()), &change_requests)?
        .result
        .list;
    if !is_same_list(&list, &fresh) {
        return Err(list_changed_error());
    }
    for request in &list.requests {
        refuse_unusable_suggestion(request)?;
    }
    let root = git::find_root(dir)?;
    let approvals = approvals::load(dir)?;
    let failures = find_reword_failures(&root, &approvals, &list)?;
    if !failures.is_empty() {
        return Err(reword_fails_rules_error(&failures));
    }
    check_suggestions_can_be_written(dir, &list)?;
    let list_number = review_session::add_list(dir, list, seen)?;
    // The list is saved by now, so a waiting-list error must not read as nothing saved.
    review_session::add_to_waiting_list(dir).map_err(|error| Error {
        code: error.code,
        message: format!("The Feedback Request List was saved. {}", error.message),
    })?;
    Ok(StepOutput {
        // `add_list` saved this status, and only the user changes it next.
        status: Some(ReviewStatus::WaitingForUserFeedback),
        result: SendResult { list_number },
        next: NextStep::TellUserAndWait,
    })
}

/// Returns the style findings that fail in each reword, each checked as a new
/// comment in the place it would take.
fn find_reword_failures(
    root: &Path,
    approvals: &Approvals,
    list: &FeedbackRequestList,
) -> Result<Vec<Finding>, Error> {
    // A reword is new text, so none of its sentences counts as old.
    let style = StyleCheck::new(approvals, TextAtBase::default());
    let mut requests_by_file: BTreeMap<&Path, Vec<&FeedbackRequest>> = BTreeMap::new();
    for request in &list.requests {
        requests_by_file
            .entry(&request.file)
            .or_default()
            .push(request);
    }
    let mut failures = Vec::new();
    for (file, requests) in requests_by_file {
        if !requests.iter().any(|request| is_reword(request)) {
            continue;
        }
        let now = comments::read_comments_on_disk(root, file)?;
        for (request, comment) in requests.iter().zip(now.match_requests(&requests)) {
            if !is_reword(request) {
                continue;
            }
            let Some(comment) = comment else {
                return Err(list_changed_error());
            };
            let new_text = choose_new_text(request, &UserAnswer::Accept)
                .expect("refuse_unusable_suggestion refuses a reword with no words");
            let (reworded, index) = comments::read_comments_with_reword(&now, comment, &new_text)?;
            let findings = style.find_reword_style_issues(&reworded, &reworded.comments[index]);
            failures.extend(
                findings
                    .into_iter()
                    .filter(|finding| finding.outcome == Outcome::Fails),
            );
        }
    }
    Ok(failures)
}

#[derive(Debug, Serialize)]
pub struct SendResult {
    /// Numbers this list among the lists sent in its Review Session, from 1.
    pub list_number: usize,
}

/// Returns true when `list` holds the requests step 3 just built, in order,
/// with only their Agent Suggestions added.
fn is_same_list(list: &FeedbackRequestList, fresh: &FeedbackRequestList) -> bool {
    list.base == fresh.base
        && list.requests.len() == fresh.requests.len()
        && list
            .requests
            .iter()
            .zip(&fresh.requests)
            .all(|(sent, built)| is_same_request(sent, built))
}

/// Returns true when two requests differ at most in their Agent Suggestion.
fn is_same_request(sent: &FeedbackRequest, built: &FeedbackRequest) -> bool {
    // Naming every field means a new field fails to compile here until it is compared.
    let FeedbackRequest {
        id,
        file,
        name,
        reason,
        text,
        can_reword,
        line,
        suggestion: _,
    } = sent;
    (id, file, name, reason, text, can_reword, line)
        == (
            &built.id,
            &built.file,
            &built.name,
            &built.reason,
            &built.text,
            &built.can_reword,
            &built.line,
        )
}

/// Refuses a request whose Agent Suggestion the user could not take as it is.
fn refuse_unusable_suggestion(request: &FeedbackRequest) -> Result<(), Error> {
    let place = format!(
        "Feedback Request {} in {}",
        request.id.0,
        request.file.display()
    );
    if request.suggestion.is_none() {
        return Err(Error {
            code: ErrorCode::MissingSuggestion,
            message: format!("Add an Agent Suggestion to {place}, then call again."),
        });
    }
    if !is_reword(request) {
        return Ok(());
    }
    if !request.can_reword {
        return Err(Error {
            code: ErrorCode::BadInput,
            message: format!(
                "The user can only keep or cut the comment of {place}, so suggest keep or cut."
            ),
        });
    }
    // Accepting a reword with no words would cut the comment.
    if choose_new_text(request, &UserAnswer::Accept).is_none() {
        return Err(Error {
            code: ErrorCode::BadInput,
            message: format!("Give the reword of {place} its words, or suggest a cut."),
        });
    }
    Ok(())
}

fn is_reword(request: &FeedbackRequest) -> bool {
    matches!(
        request
            .suggestion
            .as_ref()
            .map(|suggestion| &suggestion.kind),
        Some(SuggestionKind::Reword { .. })
    )
}

fn list_changed_error() -> Error {
    Error {
        code: ErrorCode::ListChanged,
        message: "The code or the list changed after make_feedback_request_list built it. Build the list again.".to_owned(),
    }
}

fn reword_fails_rules_error(failures: &[Finding]) -> Error {
    let broken: Vec<String> = failures
        .iter()
        .map(|finding| {
            let detail = finding
                .detail
                .as_ref()
                .map_or(String::new(), |detail| format!(" ({detail})"));
            format!(
                "{} line {} breaks \"{}\"{detail}",
                finding.file.display(),
                finding.line,
                finding.rule.sentence()
            )
        })
        .collect();
    Error {
        code: ErrorCode::RewordFailsRules,
        message: format!("Fix each reword, then call again. {}.", broken.join("; ")),
    }
}
