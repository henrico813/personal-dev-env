//! Runs `hub auto`, the steps of an auto run, in the order `ready`,
//! `check`, `approve`, `now`. An auto run works a list of cards the
//! user approved, without the user, until each card stops at a pull
//! request or a question. That list, `run.md`, names every ready card
//! once, and may name active or review cards the user picked. Only the
//! cards under `## Run` are worked.
//!
//! ```text
//! # Auto run 2
//!
//! ## Run
//! - DEMO-KB-007 demo: retry failed uploads
//!
//! ## Needs your hands
//! - DEMO-KB-009 demo: plug in the new disk
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use jiff::Zoned;
use regex::Regex;

use super::{Outcome, sidebar};
use crate::external::cards::{CardKey, CardNote, ListedCard, Stage};
use crate::shared::decisions;
use crate::shared::run_folder::{self, RunFolder};
use crate::shared::{HubError, Tools};

/// The title line of `run.md`, such as `# Auto run 2`.
static RUN_TITLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^# Auto run ([0-9]+)$").unwrap());
/// A card line of `run.md`, such as `- DEMO-KB-007 demo: retry`.
static LISTED_CARD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^- ([A-Z][A-Z0-9]*-KB-[0-9]{3,})\b").unwrap());

/// The user's approval, as `hub auto approve` takes it.
pub struct AutoApproval {
    pub number: u32,
    /// The user's reply, word for word.
    pub reply: String,
    /// The reply is the user's own words, which must name exactly the
    /// run's cards, rather than `approve run N`.
    pub given: bool,
}

/// `run.md` read into its number and the keys under each heading.
#[derive(Default)]
struct RunList {
    /// From the `# Auto run N` line.
    number: Option<u32>,
    sections: BTreeMap<String, Vec<CardKey>>,
}

impl RunList {
    /// Returns the keys under `## Run`, the cards the run works.
    fn run_keys(&self) -> &[CardKey] {
        self.sections.get("Run").map_or(&[], Vec::as_slice)
    }
}

/// Writes every ready card's title, project and criteria to
/// `ready.md`, and prints it.
///
/// # Errors
///
/// `HubError::Card` when `card` fails, before `ready.md` changes.
pub fn run_auto_ready(tools: &Tools<'_>, run: &RunFolder) -> Result<Outcome, HubError> {
    let mut descriptions = Vec::new();
    for key in tools.cards.list_ready()? {
        let note = tools.cards.read_card(&key)?;
        descriptions.push(describe_card(&key, &note));
    }
    let text = if descriptions.is_empty() {
        "No ready cards.\n".to_owned()
    } else {
        descriptions.join("\n\n") + "\n"
    };
    run_folder::write_file_whole(&run.path().join("ready.md"), &text)?;
    Ok(Outcome {
        lines: text.lines().map(str::to_owned).collect(),
    })
}

/// Checks `run.md` against the cards' stages, and prints how many cards
/// the run works.
///
/// # Errors
///
/// `HubError::NotDone` with one line per problem.
pub fn run_auto_check(tools: &Tools<'_>, run: &RunFolder) -> Result<Outcome, HubError> {
    let Some(text) = run.read_file_if_exists("run.md")? else {
        return Err(HubError::NotDone("run.md does not exist".to_owned()));
    };
    let list = read_run_list(&text);
    let number = check_run_list(&tools.cards.list_cards()?, &list)
        .map_err(|problems| HubError::NotDone(problems.join("\n")))?;
    Ok(Outcome::line(format!(
        "run {number}: {} cards to run",
        list.run_keys().len()
    )))
}

/// Saves the checked list with the user's reply, once. A reply other
/// than `approve run N` starts nothing, since run 1 started on a "go"
/// meant for another question. Given words pass only when they name by
/// key every card under `## Run` and no other card.
///
/// # Errors
///
/// `HubError::NotDone` when the run is already approved, the reply
/// approves nothing, or `run.md` fails its check or has another number.
pub fn run_auto_approve(
    tools: &Tools<'_>,
    run: &RunFolder,
    approval: &AutoApproval,
    now: &Zoned,
) -> Result<Outcome, HubError> {
    let phrase = format!("approve run {}", approval.number);
    if run.read_file_if_exists("approved.md")?.is_some() {
        return Err(HubError::NotDone(format!(
            "run {} is already approved",
            approval.number
        )));
    }
    if !approval.given && normalize_reply(&approval.reply) != phrase {
        return Err(HubError::NotDone(format!(
            "the reply is not '{phrase}'; nothing was approved"
        )));
    }
    let failed_check =
        || HubError::NotDone("run.md fails its check; run hub auto check".to_owned());
    let text = run
        .read_file_if_exists("run.md")?
        .ok_or_else(failed_check)?;
    let list = read_run_list(&text);
    let number = check_run_list(&tools.cards.list_cards()?, &list).map_err(|_| failed_check())?;
    if number != approval.number {
        return Err(HubError::NotDone(format!(
            "run.md is run {number}, not run {}",
            approval.number
        )));
    }
    if approval.given {
        let named: BTreeSet<CardKey> = CardKey::find_all_in_text(&approval.reply)
            .into_iter()
            .collect();
        let run_keys: BTreeSet<CardKey> = list.run_keys().iter().cloned().collect();
        if named != run_keys {
            return Err(HubError::NotDone(
                "the words do not name every card under '## Run' and no other; \
                 nothing was approved"
                    .to_owned(),
            ));
        }
    }
    let approved = format!(
        "# Auto run {number} approved\n\nApproved: {}\nThe user's words: {}\n\n{text}",
        now.strftime("%Y-%m-%d %H:%M"),
        approval.reply
    );
    run_folder::write_file_whole(&run.path().join("approved.md"), &approved)?;
    Ok(Outcome::line(format!(
        "approved run {number}: {} cards",
        list.run_keys().len()
    )))
}

/// Writes `brief-now.md`, one line per card of the run, and prints it.
/// A card's line is the first non-empty line of its worker's status
/// file. It says `(needs you)` while `decisions.md` holds a question
/// whose title starts with the card's key. The title of each question
/// follows the count. It then puts the hubs at the top of the sidebar,
/// and a failure there adds a warning line to the output, never to the
/// file.
///
/// # Errors
///
/// `HubError::NotDone` when the run has no `approved.md`, so a run the
/// user never approved starts nothing.
pub fn run_auto_now(tools: &Tools<'_>, run: &RunFolder) -> Result<Outcome, HubError> {
    let Some(approved) = run.read_file_if_exists("approved.md")? else {
        return Err(HubError::NotDone(
            "the run is not approved; start nothing".to_owned(),
        ));
    };
    let list = read_run_list(&approved);
    let questions = decisions::read_decisions(&run.read_optional_file("decisions.md")?);
    let asking: BTreeSet<&str> = questions
        .iter()
        .filter_map(|question| question.title.split_whitespace().next())
        .collect();
    let mut lines = vec![match list.number {
        Some(number) => format!("Auto run {number}"),
        None => "Auto run".to_owned(),
    }];
    for key in list.run_keys() {
        let status_file = format!("status-{}-auto.md", key.as_str().to_lowercase());
        let status = run.read_optional_file(&status_file)?;
        let news = status
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .unwrap_or("not started");
        let mark = if asking.contains(key.as_str()) {
            " (needs you)"
        } else {
            ""
        };
        lines.push(format!("{}{mark}: {news}", key.as_str()));
    }
    lines.push(format!("Questions for you: {}", questions.len()));
    lines.extend(
        questions
            .iter()
            .map(|question| format!("- {}", question.title)),
    );
    let text = lines.join("\n") + "\n";
    run_folder::write_file_whole(&run.path().join("brief-now.md"), &text)?;
    if let Some(warning) = sidebar::gather_after_hub_start(tools) {
        lines.push(warning);
    }
    Ok(Outcome { lines })
}

/// Returns the run's number when `run.md` can go to the user, else
/// every problem. A listed card must be ready, active or review and
/// listed only once, and every ready card must be listed, so the
/// briefing accounts for each one. The user's `approve run N`, not the
/// stage, is what lets the run work a card.
fn check_run_list(cards: &[ListedCard], list: &RunList) -> Result<u32, Vec<String>> {
    let listed: Vec<&CardKey> = list.sections.values().flatten().collect();
    let mut problems = Vec::new();
    if list.number.is_none() {
        problems.push("run.md lacks its '# Auto run N' title".to_owned());
    }
    if list.run_keys().is_empty() {
        problems.push("run.md lists no card under '## Run'".to_owned());
    }
    for key in listed.iter().copied().collect::<BTreeSet<_>>() {
        if listed.iter().filter(|other| **other == key).count() > 1 {
            problems.push(format!("{} is listed more than once", key.as_str()));
        }
        match cards.iter().find(|card| card.key == *key) {
            None => problems.push(format!("{} is not on the board", key.as_str())),
            Some(card) if matches!(card.stage, Stage::Ready | Stage::Active | Stage::Review) => {}
            Some(card) => problems.push(format!(
                "{} is a {} card; a run takes ready, active or review cards",
                key.as_str(),
                card.stage.as_str()
            )),
        }
    }
    for key in cards
        .iter()
        .filter(|card| card.stage == Stage::Ready)
        .map(|card| &card.key)
    {
        if !listed.contains(&key) {
            problems.push(format!(
                "ready card {} is missing from run.md",
                key.as_str()
            ));
        }
    }
    match list.number {
        Some(number) if problems.is_empty() => Ok(number),
        _ => Err(problems),
    }
}

/// Reads `run.md` or `approved.md` into a `RunList`. A card line
/// before the first `## ` heading belongs to no heading and is left
/// out.
fn read_run_list(text: &str) -> RunList {
    let mut list = RunList::default();
    let mut heading: Option<&str> = None;
    for line in text.lines() {
        if let Some(title) = RUN_TITLE.captures(line) {
            list.number = title[1].parse().ok();
        } else if let Some(name) = line.strip_prefix("## ") {
            let name = name.trim();
            list.sections.entry(name.to_owned()).or_default();
            heading = Some(name);
        } else if let Some(name) = heading
            && let Some(card) = LISTED_CARD.captures(line)
            && let Ok(key) = card[1].parse()
            && let Some(keys) = list.sections.get_mut(name)
        {
            keys.push(key);
        }
    }
    list
}

/// Returns the reply in lower case with single spaces and no quotes or
/// end marks, so `Approve run 2.` reads as `approve run 2`.
fn normalize_reply(reply: &str) -> String {
    let words = reply
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    words.trim_matches(|c| "\"'.! ".contains(c)).to_owned()
}

/// Writes a card's heading, project and criteria for the AI model that
/// sorts the cards. A card with no project shows `-`.
fn describe_card(key: &CardKey, note: &CardNote) -> String {
    let project = if note.project.is_empty() {
        "-"
    } else {
        note.project.as_str()
    };
    let heading = format!("## {} {}", key.as_str(), note.title);
    let mut lines = vec![heading.trim_end().to_owned(), format!("Project: {project}")];
    lines.extend(note.criteria.iter().cloned());
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A list the user cannot run as approved must not reach the user.
    /// A draft card waits for the user's refinement, while an active or
    /// review card the user picked runs without moving back to ready. A
    /// card listed twice
    /// gets two workers. An empty Run starts nothing. A ready card left
    /// out never reaches the briefing. A list with no number cannot be
    /// approved by `approve run N`. A card under another heading counts
    /// as listed.
    #[test]
    fn check_run_list_names_each_problem() {
        struct Case {
            name: &'static str,
            run_md: &'static str,
            expected: Result<u32, Vec<&'static str>>,
        }
        let cases = [
            Case {
                name: "complete",
                run_md: "# Auto run 2\n\n## Run\n- DEMO-KB-001 demo: retry\n\n\
                         ## Needs your hands\n- DEMO-KB-002 demo: plug in a disk\n",
                expected: Ok(2),
            },
            Case {
                name: "active-and-review",
                run_md: "# Auto run 2\n\n## Run\n- DEMO-KB-001 demo: retry\n\
                         - DEMO-KB-002 demo: resume\n- DEMO-KB-004 demo: active\n\
                         - DEMO-KB-005 demo: review\n",
                expected: Ok(2),
            },
            Case {
                name: "draft-or-unknown",
                run_md: "# Auto run 2\n\n## Run\n- DEMO-KB-001 demo: retry\n\
                         - DEMO-KB-002 demo: resume\n- DEMO-KB-003 demo: draft\n\
                         - DEMO-KB-099 demo: typo\n",
                expected: Err(vec![
                    "DEMO-KB-003 is a draft card; a run takes ready, active or review cards",
                    "DEMO-KB-099 is not on the board",
                ]),
            },
            Case {
                name: "listed-twice",
                run_md: "# Auto run 2\n\n## Run\n- DEMO-KB-001 demo: retry\n\
                         - DEMO-KB-002 demo: resume\n\n## Left out\n- DEMO-KB-001 demo: retry\n",
                expected: Err(vec!["DEMO-KB-001 is listed more than once"]),
            },
            Case {
                name: "no-run-cards",
                run_md: "# Auto run 2\n\n## Run\n\n## Left out\n- DEMO-KB-001 demo: retry\n\
                         - DEMO-KB-002 demo: resume\n",
                expected: Err(vec!["run.md lists no card under '## Run'"]),
            },
            Case {
                name: "missing-ready",
                run_md: "# Auto run 2\n\n## Run\n- DEMO-KB-001 demo: retry\n",
                expected: Err(vec!["ready card DEMO-KB-002 is missing from run.md"]),
            },
            Case {
                name: "no-title",
                run_md: "## Run\n- DEMO-KB-001 demo: retry\n- DEMO-KB-002 demo: resume\n",
                expected: Err(vec!["run.md lacks its '# Auto run N' title"]),
            },
        ];
        let cards: Vec<ListedCard> = [
            ("DEMO-KB-001", Stage::Ready),
            ("DEMO-KB-002", Stage::Ready),
            ("DEMO-KB-003", Stage::Draft),
            ("DEMO-KB-004", Stage::Active),
            ("DEMO-KB-005", Stage::Review),
        ]
        .into_iter()
        .map(|(key, stage)| ListedCard {
            key: key.parse().unwrap(),
            stage,
            project: String::new(),
            title: String::new(),
        })
        .collect();

        for case in cases {
            let expected = case
                .expected
                .map_err(|problems| problems.iter().map(|problem| problem.to_string()).collect());
            assert_eq!(
                check_run_list(&cards, &read_run_list(case.run_md)),
                expected,
                "{}",
                case.name
            );
        }
    }
}
