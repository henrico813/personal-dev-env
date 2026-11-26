//! Runs `hub ask` and `hub answer`, which keep a question for the user
//! on its card. `find_problems` names the parts a question file needs.

use super::Outcome;
use crate::external::cards::{CardKey, OPEN_QUESTIONS, Stage};
use crate::shared::questions::LISTED_STAGES;
use crate::shared::{HubError, Tools};

/// The card section an answered question moves to.
const DECISIONS: &str = "Decisions";

/// Adds the question in `text` to the card's Open Questions, where
/// `card` gives it a Q number and today's date. Prints a line such as
/// `DEMO-KB-007 asks 'Deploy tonight?'`.
///
/// # Errors
///
/// `HubError::NotDone`, changing nothing, when `text` lacks a part the
/// user needs to decide, the card is in a stage the hub lists no
/// questions from, such as draft, or the card already asks a question
/// with the same title.
pub fn run_ask(tools: &Tools<'_>, key: &CardKey, text: &str) -> Result<Outcome, HubError> {
    let question = read_question(text)?;
    let note = tools.cards.read_card(key)?;
    let stage = note.stage;
    let is_listed = stage.is_some_and(|stage| LISTED_STAGES.contains(&stage));
    if !is_listed {
        return Err(HubError::NotDone(format!(
            "{} is in {}; the hub lists questions only on todo, active, in-review or blocked cards",
            key.as_str(),
            stage.map_or("an unknown stage", Stage::as_str)
        )));
    }
    // A rerun after a timeout or a handoff would otherwise add a copy
    // that stays listed after the user answers the first one.
    if let Some(open) = note
        .open_questions
        .iter()
        .find(|open| open.text == question[0])
    {
        return Err(HubError::NotDone(format!(
            "{} already asks '{}' as Q{}",
            key.as_str(),
            question[0],
            open.number
        )));
    }
    tools
        .cards
        .add_item(key, OPEN_QUESTIONS, &format_item(&question))?;
    Ok(Outcome::from_line(format!(
        "{} asks '{}'",
        key.as_str(),
        question[0]
    )))
}

/// Moves the card's open question Q`number` to Decisions, with `answer`
/// as its last line. Prints a line such as
/// `DEMO-KB-007 Q3 answered 'Deploy tonight?': A`.
///
/// # Errors
///
/// `HubError::NotDone`, changing nothing, when `answer` is empty or
/// has a line break, or the card has no open question Q`number`.
/// `HubError::PartlyDone` when the answer was added and the question
/// still shows as open, since the second read or the removal failed,
/// or Q`number` changed after the first read.
pub fn run_answer(
    tools: &Tools<'_>,
    key: &CardKey,
    number: u32,
    answer: &str,
) -> Result<Outcome, HubError> {
    // `card` would read a second line as an item or Log line of its own.
    if answer.trim().is_empty() || answer.contains(['\n', '\r']) {
        return Err(HubError::NotDone(
            "the answer must be one line of text".to_owned(),
        ));
    }
    let note = tools.cards.read_card(key)?;
    let Some(item) = note
        .open_questions
        .into_iter()
        .find(|item| item.number == number)
    else {
        return Err(HubError::NotDone(format!(
            "{} has no open question Q{number}",
            key.as_str()
        )));
    };
    // The `Q3 asked DATE:` head keeps the number taken, so `card add`
    // never gives it to a later question.
    let head = match item.asked {
        Some(day) => format!("Q{} asked {day}: {}", item.number, item.text),
        None => format!("Q{}: {}", item.number, item.text),
    };
    let mut decision = vec![head];
    decision.extend(item.lines.clone());
    decision.push(format!("Answer: {answer}"));

    // The answer goes in first, so a failed removal leaves the question
    // listed rather than losing it.
    tools
        .cards
        .add_item(key, DECISIONS, &format_item(&decision))?;
    let make_partly_done = |error: HubError| HubError::PartlyDone {
        done: format!("added the answer to {} {DECISIONS}", key.as_str()),
        error: Box::new(error),
    };
    // A hand edit can shift a hand-typed item's number, so read again
    // and remove Q`number` only while it is still the answered item.
    let now = tools
        .cards
        .read_card(key)
        .map_err(|error| make_partly_done(HubError::Card(error)))?;
    if !now.open_questions.contains(&item) {
        return Err(make_partly_done(HubError::NotDone(format!(
            "{} Q{number} changed since the hub read it, so '{}' stays open; \
             find its number in hub waiting, then run \
             card edit {} \"Open Questions\" Q<n> --remove, never hub answer",
            key.as_str(),
            item.text,
            key.as_str()
        ))));
    }
    tools
        .cards
        .remove_question(key, item.number)
        .map_err(|error| make_partly_done(HubError::Card(error)))?;
    Ok(Outcome::from_line(format!(
        "{} Q{number} answered '{}': {answer}",
        key.as_str(),
        item.text
    )))
}

/// Returns the question's title, then its other lines, blank lines
/// left out.
fn read_question(text: &str) -> Result<Vec<String>, HubError> {
    let mut lines = text
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.trim().is_empty());
    let Some(title) = lines
        .next()
        .and_then(|line| line.strip_prefix("## "))
        .map(str::trim)
        .filter(|title| !title.is_empty())
    else {
        return Err(HubError::NotDone(
            "the question must start with a ## title line".to_owned(),
        ));
    };
    let mut question = vec![title.to_owned()];
    question.extend(lines.map(str::to_owned));
    let problems = find_problems(&question[1..]);
    if !problems.is_empty() {
        return Err(HubError::NotDone(format!(
            "question '{title}': {}",
            problems.join(", ")
        )));
    }
    Ok(question)
}

/// Lists each part the user needs to decide that `lines` lack, such as
/// `no Recommended: line`. Lines that start with a space carry on the
/// line above, so they are not parts.
fn find_problems(lines: &[String]) -> Vec<String> {
    const FIELDS: [&str; 3] = ["Recommended:", "If no reply:", "Depends on:"];
    let starts: Vec<&str> = lines
        .iter()
        .map(String::as_str)
        .filter(|line| !line.starts_with(' '))
        .collect();
    let choice_count = starts.iter().filter(|line| line.starts_with("- ")).count();
    let has_question = starts
        .iter()
        .any(|line| !line.starts_with("- ") && !FIELDS.iter().any(|field| line.starts_with(field)));
    let mut problems = Vec::new();
    if !has_question {
        problems.push("no question line".to_owned());
    }
    if choice_count < 2 {
        problems.push("fewer than 2 choices".to_owned());
    }
    for field in ["Recommended:", "If no reply:"] {
        if !starts.iter().any(|line| line.starts_with(field)) {
            problems.push(format!("no {field} line"));
        }
    }
    problems
}

/// Returns `lines` as one card item. Each line after the first starts
/// with two spaces, so `card` keeps the whole question as one item.
fn format_item(lines: &[String]) -> String {
    lines.join("\n  ")
}
