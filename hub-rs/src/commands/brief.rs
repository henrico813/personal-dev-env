//! Prints the briefing, everything that waits on the user.

use jiff::Timestamp;
use jiff::tz::TimeZone;

use super::Outcome;
use crate::external::cards::{Board, format_days};
use crate::shared::questions::{OpenQuestion, find_typed_since_asked, read_open_questions};
use crate::shared::run_folder::RunFolder;
use crate::shared::{HubError, Tools, format_span};

/// Prints the board line, one line per pending approval request,
/// read fresh at `now`, `reviews.md`, then each card's open question in
/// full, so the user never has to ask what a question was. Under a
/// question's title come `Check first:` lines for sessions on its card
/// where the user typed since it was asked, since the user may have
/// answered there. A failed
/// `Cards::read_board` becomes a `board: REASON` line, and a failed
/// question read a `questions: REASON` line. Cards waiting on people
/// outside the team come last, as news the user may chase but need not
/// act on.
pub fn run_brief(tools: &Tools<'_>, run: &RunFolder, now: Timestamp) -> Result<Outcome, HubError> {
    let questions = read_open_questions(tools.cards);
    let question_count = match &questions {
        Ok(questions) if questions.len() == 1 => "1 question".to_owned(),
        Ok(questions) => format!("{} questions", questions.len()),
        Err(_) => "questions unread".to_owned(),
    };
    let board = tools.cards.read_board();
    let mut board_lines = vec![match &board {
        Ok(board) => format!("board {}; {question_count}", board.counts),
        Err(error) => format!("board: {error}"),
    }];
    board_lines.extend(board.as_ref().ok().and_then(Board::blocked_line));
    let blocked = board.map(|board| board.blocked).unwrap_or_default();
    let mut sections = vec![board_lines.join("\n")];
    let approvals = list_pending_approvals(tools, now)?;
    if !approvals.is_empty() {
        sections.push(approvals.join("\n"));
    }
    let pull_requests = run.read_optional_file("reviews.md")?;
    if !pull_requests.trim().is_empty() {
        sections.push(pull_requests.trim().to_owned());
    }
    match &questions {
        Ok(questions) => {
            let time_zone = TimeZone::system();
            for question in questions {
                let mut section = vec![format!(
                    "Question {}: {}",
                    question.key.as_str(),
                    question.title
                )];
                section.extend(check_typed_replies(tools, question, &time_zone));
                section.extend(question.lines.iter().cloned());
                sections.push(section.join("\n"));
            }
        }
        Err(error) => sections.push(format!("questions: {error}")),
    }
    // An unread question list may hide one, so it is never all clear.
    if approvals.is_empty()
        && pull_requests.trim().is_empty()
        && questions.is_ok_and(|questions| questions.is_empty())
    {
        sections.push("Nothing waits on the user.".to_owned());
    }
    if !blocked.is_empty() {
        let mut section = vec!["Blocked (no action needed)".to_owned()];
        for card in &blocked {
            section.push(format!(
                "- {}, {}",
                card.key.as_str(),
                format_days(card.days)
            ));
            section.extend(
                [&card.title, &card.reason]
                    .into_iter()
                    .filter(|text| !text.is_empty())
                    .map(|text| format!("  {text}")),
            );
        }
        sections.push(section.join("\n"));
    }
    let text = sections.join("\n\n");
    Ok(Outcome {
        lines: text.lines().map(str::to_owned).collect(),
    })
}

/// Returns a line for each session on the question's card where the
/// user typed since the day it was asked, such as
/// `Check first: the user wrote in demo-007-impl 2026-11-21 21:14`,
/// or one line that says why OpenCode could not be read. A session
/// Herdr does not list shows its ID.
fn check_typed_replies(
    tools: &Tools<'_>,
    question: &OpenQuestion,
    time_zone: &TimeZone,
) -> Vec<String> {
    let typed = match find_typed_since_asked(tools.opencode, question, time_zone) {
        Ok(typed) if typed.is_empty() => return Vec::new(),
        Ok(typed) => typed,
        Err(error) => return vec![format!("Check first: OpenCode unread: {error}")],
    };
    let agents = tools.herdr.list_agents().unwrap_or_default();
    typed
        .into_iter()
        .map(|(session, at)| {
            let name = agents
                .iter()
                .find(|agent| agent.session_id.as_deref() == Some(session.as_str()))
                .map_or(session.as_str(), |agent| agent.name.as_str());
            let time = at.to_zoned(time_zone.clone()).strftime("%Y-%m-%d %H:%M");
            format!("Check first: the user wrote in {name} {time}")
        })
        .collect()
}

/// Returns one line per pending request, such as
/// `approval A0016cd 1h12m left personal-dev-env #515 create`, and a
/// line for request files that did not parse, since one may hold an
/// approval that waits.
fn list_pending_approvals(tools: &Tools<'_>, now: Timestamp) -> Result<Vec<String>, HubError> {
    let saved = tools
        .requests
        .list_saved_requests(now)
        .map_err(|source| HubError::File {
            path: tools.requests.folder().to_owned(),
            source,
        })?;
    let mut pending: Vec<_> = saved
        .requests
        .iter()
        .filter(|request| request.is_open && request.state == "pending")
        .collect();
    pending.sort_by_key(|request| request.changed);
    let mut lines: Vec<String> = pending
        .iter()
        .map(|request| {
            let left = request
                .time_left(now)
                .map_or_else(String::new, |left| format!(" {} left", format_span(left)));
            format!(
                "approval {}{left} {}",
                request.reference(),
                request.pull_request
            )
        })
        .collect();
    if saved.bad_files > 0 {
        lines.push(format!("requests: {} files did not parse", saved.bad_files));
    }
    Ok(lines)
}
