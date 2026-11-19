//! Sorts a `Queue` into the board's groups, such as one for cards
//! needing the user's review, and writes each row's words, its
//! details page and the NEXT lines. Nothing here draws. Read
//! `build_groups` and `describe_item` first.

use ratatui::style::Color;

use super::queue::{CardLine, Queue, SessionRow};
use crate::external::cards::{CardNote, Stage};
use crate::shared::lifecycle::Lifecycle;

/// The board's groups, in screen order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Group {
    /// What the read could not get, such as a failed
    /// `Cards::list_cards` call.
    Problems,
    NeedsYou,
    Active,
    InReview,
    Blocked,
    Todo,
    Draft,
}

/// The word a row starts with, which says whether anyone works on it
/// now. Each word takes at most `Status::WIDTH` columns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Status {
    /// It waits on the user.
    You,
    /// A worker session of its card is working.
    Run,
    /// Its card has a worker session, and none of them is working.
    Idle,
    /// Its card needs the user's review, and no worker runs on it.
    Rev,
    /// Its card has no worker session.
    None,
    /// A source the read could not get.
    Problem,
}

impl Group {
    pub(super) fn title(self) -> &'static str {
        match self {
            Group::Problems => "Could not read",
            Group::Active => "Active",
            Group::InReview => "In review",
            Group::Blocked => "Blocked",
            Group::NeedsYou => "Needs you",
            Group::Todo => "Todo",
            Group::Draft => "Draft",
        }
    }

    /// Returns the word that names the group in element ids, such as
    /// `group:in-review`.
    pub(super) fn slug(self) -> &'static str {
        match self {
            Group::Problems => "could-not-read",
            Group::Active => "active",
            Group::InReview => "in-review",
            Group::Blocked => "blocked",
            Group::NeedsYou => "needs-you",
            Group::Todo => "todo",
            Group::Draft => "draft",
        }
    }

    /// Returns one of the 16 colors the terminal's own theme sets, or
    /// the terminal's default for Draft, so a phone app's theme keeps
    /// them readable.
    pub(super) fn color(self) -> Color {
        match self {
            Group::Problems => Color::Red,
            Group::Active => Color::Yellow,
            Group::InReview => Color::Green,
            Group::Blocked => Color::Cyan,
            Group::NeedsYou => Color::Magenta,
            Group::Todo => Color::Blue,
            Group::Draft => Color::Reset,
        }
    }
}

impl Status {
    /// The columns the widest word takes.
    pub(super) const WIDTH: usize = 4;

    pub(super) fn word(self) -> &'static str {
        match self {
            Status::You => "you",
            Status::Run => "run",
            Status::Idle => "idle",
            Status::Rev => "rev",
            Status::None => "none",
            Status::Problem => "!",
        }
    }

    /// Returns the word's color, the same one its group has, or the
    /// terminal's default for a row nobody works on.
    pub(super) fn color(self) -> Color {
        match self {
            Status::You => Group::NeedsYou.color(),
            Status::Run => Group::Active.color(),
            Status::Rev => Group::InReview.color(),
            Status::Problem => Group::Problems.color(),
            Status::Idle | Status::None => Color::Reset,
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
    pub status: Status,
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

/// Returns every group with its items, in screen order. The
/// `Could not read` group shows only when a read failed, and the
/// group of cards waiting on people outside the team only when it
/// holds a card.
pub(super) fn build_groups(queue: &Queue) -> Vec<(Group, Vec<Item>)> {
    let cards_at = |stage: Stage| -> Vec<Item> {
        queue
            .cards
            .iter()
            .enumerate()
            .filter(|(_, card)| card.stage == stage)
            .map(|(index, card)| card_item(index, card, &queue.sessions))
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
            status: Status::You,
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
                status: Status::You,
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
            status: Status::Problem,
            target: Target::Problem(index),
        })
        .collect();

    let mut groups = Vec::new();
    if !problems.is_empty() {
        groups.push((Group::Problems, problems));
    }
    groups.extend([
        (Group::NeedsYou, needs_you),
        (Group::Active, cards_at(Stage::Active)),
        (Group::InReview, cards_at(Stage::InReview)),
        (Group::Blocked, cards_at(Stage::Blocked)),
        (Group::Todo, cards_at(Stage::Todo)),
        (Group::Draft, cards_at(Stage::Draft)),
    ]);
    // Most boards have no card waiting on people outside the team,
    // and a phone has few rows.
    groups.retain(|(group, items)| *group != Group::Blocked || !items.is_empty());
    groups
}

/// The statuses the QUEUE heading counts, in its order.
const COUNTED: [Status; 3] = [Status::Run, Status::You, Status::Idle];

/// The lines of the NEXT block and view.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct NextLines {
    /// One `you:` line per row of Needs you, or one that says none.
    pub you: Vec<String>,
    /// The `then:` line, which counts the cards needing the user's
    /// review.
    pub then: String,
    /// One `hub:` line per card ready to start, in the order
    /// `Cards::list_cards` gives, or one that says none. The hub may
    /// start another card first.
    pub hub: Vec<String>,
}

/// Returns the rows of the glance page's QUEUE block, which are what
/// could not be read, what waits on the user, then cards that run,
/// idle cards and cards needing the user's review. A card whose worker
/// waits on the user is left out, since that worker is already a row
/// of the `Needs you` group.
pub(super) fn glance_items(groups: &[(Group, Vec<Item>)]) -> Vec<(Group, &Item)> {
    let order = [
        Status::Problem,
        Status::You,
        Status::Run,
        Status::Idle,
        Status::Rev,
    ];
    let mut shown = Vec::new();
    for status in order {
        for (group, items) in groups {
            if status == Status::You && *group != Group::NeedsYou {
                continue;
            }
            let matching = items.iter().filter(|item| item.status == status);
            shown.extend(matching.map(|item| (*group, item)));
        }
    }
    shown
}

/// Returns the QUEUE heading's counts, such as 2 for `Status::Run`,
/// each with the group that holds its first row, where a tap on it
/// jumps. A status with no row is left out.
pub(super) fn count_statuses(groups: &[(Group, Vec<Item>)]) -> Vec<(Status, usize, Group)> {
    let shown = glance_items(groups);
    COUNTED
        .into_iter()
        .filter_map(|status| {
            let mut rows = shown.iter().filter(|(_, item)| item.status == status);
            let (group, _) = rows.next()?;
            Some((status, rows.count() + 1, *group))
        })
        .collect()
}

/// Returns the NEXT lines from the groups. `no_hub` is true when no hub
/// ran at the read, so nothing could wait on the user.
pub(super) fn write_next_lines(groups: &[(Group, Vec<Item>)], no_hub: bool) -> NextLines {
    let items = |wanted: Group| {
        groups
            .iter()
            .find(|(group, _)| *group == wanted)
            .map_or(&[][..], |(_, items)| items)
    };
    let mut you: Vec<String> = items(Group::NeedsYou)
        .iter()
        .map(|item| match item.target {
            Target::Question(_) => format!("you: answer {}", item.text),
            _ => format!("you: {}", item.text),
        })
        .collect();
    if you.is_empty() {
        let none = if no_hub {
            "no hub running"
        } else {
            "nothing waits on you"
        };
        you.push(format!("you: {none}"));
    }
    let then = match items(Group::InReview).len() {
        0 => "then: no reviews".to_owned(),
        1 => "then: review 1 card".to_owned(),
        count => format!("then: review {count} cards"),
    };
    let mut hub: Vec<String> = items(Group::Todo)
        .iter()
        .map(|item| format!("hub: start {}", item.text))
        .collect();
    if hub.is_empty() {
        hub.push(format!("hub: no {} card", Stage::Todo.as_str()));
    }
    NextLines { you, then, hub }
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
/// `DEMO-KB-001  in-review  Demo`, its workers, next step, PR links and
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
/// `sessions` give the states of its workers.
fn card_item(index: usize, card: &CardLine, sessions: &[SessionRow]) -> Item {
    let text = if card.title.is_empty() {
        card.key.as_str().to_owned()
    } else {
        card.title.clone()
    };
    Item {
        id: card.key.as_str().to_owned(),
        text,
        tag: card.project.clone(),
        status: card_status(card, sessions),
        target: Target::Card(index),
    }
}

/// Returns a card's status from its workers' states. A worker that
/// waits on the user wins over one at work, and one at work wins over
/// the card needing the user's review.
fn card_status(card: &CardLine, sessions: &[SessionRow]) -> Status {
    let states: Vec<Lifecycle> = card
        .workers
        .iter()
        .filter_map(|name| sessions.iter().find(|row| row.name == *name))
        .map(|row| row.state)
        .collect();
    if states.contains(&Lifecycle::WaitsOnUser) {
        Status::You
    } else if states.contains(&Lifecycle::Working) {
        Status::Run
    } else if card.stage == Stage::InReview {
        Status::Rev
    } else if states.is_empty() {
        Status::None
    } else {
        Status::Idle
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

#[cfg(test)]
mod tests {
    use super::*;

    use crate::board::view::tests::sample_queue;

    /// A worker that runs on a card needing the user's review, such as
    /// one fixing review comments, means the card is in work now. Shown
    /// as `rev`, it would look like it waits for the user's review.
    #[test]
    fn build_groups_marks_working_review_card_run() {
        let mut queue = sample_queue();
        let docs_fix = queue.sessions.iter_mut().find(|row| row.name == "docs-fix");
        docs_fix.unwrap().state = Lifecycle::Working;

        let groups = build_groups(&queue);

        let (_, in_review) = groups
            .iter()
            .find(|(group, _)| *group == Group::InReview)
            .unwrap();
        assert_eq!(in_review[0].status, Status::Run);
    }
}
