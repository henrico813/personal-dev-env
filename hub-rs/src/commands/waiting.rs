//! Runs `hub waiting`, which prints each question that waits on the
//! user and puts their count on the front hub's sidebar row. The hub
//! ends every message to the user with this output, so a question
//! never hides higher up the chat.

use super::Outcome;
use super::sidebar::find_front_hub;
use crate::shared::questions::{OpenQuestion, read_open_questions};
use crate::shared::run_folder::{RunFolder, write_file_whole};
use crate::shared::{HubError, Tools};

/// The run folder file that keeps each question's number, one
/// `N KEY TITLE` line per question.
const NUMBERS_FILE: &str = "question-numbers.txt";

/// Prints `N questions wait on you.`, then each open question oldest
/// first, as a `**QN. TITLE (KEY)**` line and the lines `hub ask`
/// saved, then a `Reply like` line. Prints `No question waits on you.`
/// when none waits. A card's Log gives only the day a question was
/// asked, so questions from one day print in card key order, and a
/// question with no needs-you date prints first.
///
/// A failed card read prints one `Questions could not be read` line
/// and changes nothing. A failed sidebar write adds a last line, since
/// the questions matter more than the count.
///
/// # Errors
///
/// `HubError::File` when the numbers file cannot be read or written.
pub fn run_waiting(tools: &Tools<'_>, run: &RunFolder) -> Result<Outcome, HubError> {
    let mut questions = match read_open_questions(tools.cards) {
        Ok(questions) => questions,
        Err(error) => {
            return Ok(Outcome::from_line(format!(
                "Questions could not be read: {error}"
            )));
        }
    };
    // A stable sort keeps card key order within one day.
    questions.sort_by_key(|question| question.asked_on);
    let numbers = assign_question_numbers(run, &questions)?;
    let mut lines = format_questions(&questions, &numbers);
    if let Err(error) = show_count_on_front_hub(tools, questions.len()) {
        lines.push(format!("Sidebar count not set: {error}"));
    }
    Ok(Outcome { lines })
}

/// Returns each question's number, in `questions` order, and saves
/// them. A saved number stays while any saved question still waits, so
/// a reply key such as `Q2A` to an older message names the same
/// question. A new question takes the number after the highest saved
/// one. Once no saved question waits, the next question is `Q1` again.
fn assign_question_numbers(
    run: &RunFolder,
    questions: &[OpenQuestion],
) -> Result<Vec<usize>, HubError> {
    let open: Vec<String> = questions.iter().map(format_question_id).collect();
    let mut saved = parse_numbers(&run.read_optional_file(NUMBERS_FILE)?);
    if !saved.iter().any(|(_, question)| open.contains(question)) {
        saved.clear();
    }
    let mut numbers = Vec::new();
    for question in open {
        let number = match saved.iter().find(|(_, known)| *known == question) {
            Some((number, _)) => *number,
            None => {
                let number = saved.iter().map(|(number, _)| number).max().unwrap_or(&0) + 1;
                saved.push((number, question));
                number
            }
        };
        numbers.push(number);
    }
    let text: String = saved
        .iter()
        .map(|(number, question)| format!("{number} {question}\n"))
        .collect();
    write_file_whole(&run.path().join(NUMBERS_FILE), &text)?;
    Ok(numbers)
}

/// Returns the text that tells one question from another, its card key
/// and title, such as `DEMO-KB-007 Deploy tonight`. A card may ask a
/// second question once its first is answered.
fn format_question_id(question: &OpenQuestion) -> String {
    format!("{} {}", question.key.as_str(), question.title)
}

/// Returns the numbers file's lines as numbers and questions. A line
/// with no number is left out.
fn parse_numbers(text: &str) -> Vec<(usize, String)> {
    text.lines()
        .filter_map(|line| {
            let (number, question) = line.split_once(' ')?;
            Some((number.parse().ok()?, question.to_owned()))
        })
        .collect()
}

/// Returns the lines `run_waiting` prints for these questions and their
/// numbers.
fn format_questions(questions: &[OpenQuestion], numbers: &[usize]) -> Vec<String> {
    let Some(first) = numbers.first() else {
        return vec!["No question waits on you.".to_owned()];
    };
    let count = match questions.len() {
        1 => "1 question waits".to_owned(),
        count => format!("{count} questions wait"),
    };
    let mut lines = vec![format!("{count} on you.")];
    for (question, number) in questions.iter().zip(numbers) {
        lines.push(String::new());
        lines.push(format!(
            "**Q{number}. {} ({})**",
            question.title,
            question.key.as_str()
        ));
        lines.extend(question.lines.iter().cloned());
    }
    lines.push(String::new());
    lines.push(format!("Reply like `Q{first}A`."));
    lines
}

/// Puts `count` on the front hub's sidebar row, the hub the "go to"
/// list shows first. Does nothing when this machine runs no hub.
fn show_count_on_front_hub(tools: &Tools<'_>, count: usize) -> Result<(), HubError> {
    let agents = tools.herdr.list_agent_panes()?;
    if let Some(hub) = find_front_hub(tools.runs_folder, &agents)? {
        tools.herdr.show_waiting_count(&hub.pane_id, count)?;
    }
    Ok(())
}
