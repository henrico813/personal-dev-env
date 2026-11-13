//! Sorts a `Queue` into the board's groups, such as In review, and
//! writes each row's words and its details page. Nothing here draws.
//! Read `build_groups` and `describe_item` first.

use ratatui::style::Color;

use super::queue::{CardLine, Queue};
use crate::external::cards::{CardNote, Stage};

/// The board's groups, in screen order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Group {
    /// What the read could not get, such as a failed `card list`.
    Problems,
    InProgress,
    InReview,
    NeedsYou,
    Ready,
    Draft,
}

impl Group {
    /// The groups the top line counts, with the word each shows.
    pub(super) const COUNTED: [(Group, &'static str); 3] = [
        (Group::InProgress, "progress"),
        (Group::InReview, "review"),
        (Group::NeedsYou, "you"),
    ];

    pub(super) fn title(self) -> &'static str {
        match self {
            Group::Problems => "Could not read",
            Group::InProgress => "In progress",
            Group::InReview => "In review",
            Group::NeedsYou => "Needs you",
            Group::Ready => "Ready",
            Group::Draft => "Draft",
        }
    }

    /// Returns the word that names the group in element ids, such as
    /// `group:in-review`.
    pub(super) fn slug(self) -> &'static str {
        match self {
            Group::Problems => "could-not-read",
            Group::InProgress => "in-progress",
            Group::InReview => "in-review",
            Group::NeedsYou => "needs-you",
            Group::Ready => "ready",
            Group::Draft => "draft",
        }
    }

    /// Returns the mark each row of the group starts with, so a row's
    /// group shows without color. Each mark takes one column.
    pub(super) fn mark(self) -> &'static str {
        match self {
            Group::Problems => "!",
            Group::InProgress => "◐",
            Group::InReview => "◕",
            Group::NeedsYou => "?",
            Group::Ready => "○",
            Group::Draft => "·",
        }
    }

    /// Returns one of the 16 colors the terminal's own theme sets, or
    /// the terminal's default for Draft, so a phone app's theme keeps
    /// them readable.
    pub(super) fn color(self) -> Color {
        match self {
            Group::Problems => Color::Red,
            Group::InProgress => Color::Yellow,
            Group::InReview => Color::Green,
            Group::NeedsYou => Color::Magenta,
            Group::Ready => Color::Blue,
            Group::Draft => Color::Reset,
        }
    }
}

/// One row of a group.
#[derive(Debug)]
pub(super) struct Item {
    /// Stays the same across reads, so the choice can follow the row.
    /// It is a card key, a question title, a session name or the
    /// problem.
    pub id: String,
    pub text: String,
    /// A short word for the row's right end, such as `DevEnv` for a
    /// card's project folder or `kanban-hub-3` for a hub's question.
    pub tag: String,
    pub target: Target,
}

/// The `Queue` entry an item comes from, by its place in that list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Target {
    Card(usize),
    Question(usize),
    Session(usize),
    Problem(usize),
}

impl Target {
    /// Returns the word before an item's id in its element id, such as
    /// `card` in `card:DEMO-KB-007`.
    pub(super) fn kind_word(self) -> &'static str {
        match self {
            Target::Card(_) => "card",
            Target::Question(_) => "question",
            Target::Session(_) => "session",
            Target::Problem(_) => "problem",
        }
    }
}

/// Returns every group with its items, in screen order. The Could not
/// read group shows only when a read failed.
pub(super) fn build_groups(queue: &Queue) -> Vec<(Group, Vec<Item>)> {
    let cards_at = |stage: Stage| -> Vec<Item> {
        queue
            .cards
            .iter()
            .enumerate()
            .filter(|(_, card)| card.stage == stage)
            .map(|(index, card)| card_item(index, card))
            .collect()
    };
    let mut needs_you: Vec<Item> = queue
        .questions
        .iter()
        .enumerate()
        .map(|(index, question)| Item {
            id: question.title.clone(),
            text: question.title.clone(),
            tag: question.hub.clone(),
            target: Target::Question(index),
        })
        .collect();
    needs_you.extend(
        queue
            .waiting
            .iter()
            .enumerate()
            .map(|(index, session)| Item {
                id: session.name.clone(),
                text: format!("{}  {}", session.name, session.why),
                tag: "worker".to_owned(),
                target: Target::Session(index),
            }),
    );
    let problems: Vec<Item> = queue
        .problems
        .iter()
        .enumerate()
        .map(|(index, problem)| Item {
            id: problem.clone(),
            text: problem.clone(),
            tag: String::new(),
            target: Target::Problem(index),
        })
        .collect();

    let mut groups = Vec::new();
    if !problems.is_empty() {
        groups.push((Group::Problems, problems));
    }
    groups.extend([
        (Group::InProgress, cards_at(Stage::Active)),
        (Group::InReview, cards_at(Stage::Review)),
        (Group::NeedsYou, needs_you),
        (Group::Ready, cards_at(Stage::Ready)),
        (Group::Draft, cards_at(Stage::Draft)),
    ]);
    groups
}

/// Returns the lines of an item's details page, not yet wrapped. A
/// card's page needs `note` for its project, next step, links and
/// criteria, and shows the rest without it.
pub(super) fn describe_item(queue: &Queue, target: Target, note: Option<&CardNote>) -> Vec<String> {
    match target {
        Target::Card(index) => describe_card(&queue.cards[index], note),
        Target::Question(index) => {
            let question = &queue.questions[index];
            let mut lines = vec![
                question.title.clone(),
                format!("from {}", question.hub),
                String::new(),
            ];
            lines.extend(question.lines.iter().cloned());
            lines
        }
        Target::Session(index) => {
            let session = &queue.waiting[index];
            let mut lines = vec![format!("{} waits on you: {}", session.name, session.why)];
            lines.extend(session.card.iter().map(|card| format!("card {card}")));
            lines
        }
        Target::Problem(index) => vec![queue.problems[index].clone()],
    }
}

/// Returns a card's details page lines, such as its title, then
/// `DEMO-KB-001  review  Demo`, its workers, next step, PR links and
/// criteria.
pub(super) fn describe_card(card: &CardLine, note: Option<&CardNote>) -> Vec<String> {
    let mut lines = vec![
        card.title.clone(),
        String::new(),
        format!(
            "{}  {}  {}",
            card.key.as_str(),
            card.stage.as_str(),
            card.project
        ),
    ];
    lines.push(if card.workers.is_empty() {
        "no worker".to_owned()
    } else {
        format!("by {}", card.workers.join(", "))
    });
    let Some(note) = note else {
        return lines;
    };
    lines.extend(next_step(note).map(|step| format!("next: {step}")));
    lines.extend(note.prs.iter().cloned());
    if !note.criteria.is_empty() {
        lines.push(String::new());
        lines.extend(note.criteria.iter().cloned());
    }
    lines
}

/// Returns a card's row, its title with its project folder as the tag.
fn card_item(index: usize, card: &CardLine) -> Item {
    let text = if card.title.is_empty() {
        card.key.as_str().to_owned()
    } else {
        card.title.clone()
    };
    Item {
        id: card.key.as_str().to_owned(),
        text,
        tag: card.project.clone(),
        target: Target::Card(index),
    }
}

/// Returns the card's newest `next:` Log line, else a step from its
/// newest PR link or first plan. A link alone cannot tell a merged PR
/// from an open one, so the hub's `next:` line wins.
fn next_step(note: &CardNote) -> Option<String> {
    if let Some(step) = &note.next_step {
        return Some(step.clone());
    }
    if let Some(link) = note.prs.last() {
        let number = link.rsplit('/').next().unwrap_or(link);
        return Some(format!("review PR #{number}"));
    }
    note.plans.first().map(|plan| {
        let plan_key = plan.split_whitespace().next().unwrap_or(plan);
        format!("read plan {plan_key}")
    })
}
