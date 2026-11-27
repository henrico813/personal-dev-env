//! Sorts a `Queue` into the board's groups, such as one for cards
//! needing the user's review, and writes each row's words, its
//! details page and the NEXT lines. Nothing here draws. Read
//! `build_groups` and `describe_item` first.

use jiff::civil::Date;
use ratatui::style::Color;

use super::queue::{CardLine, Queue, SessionRow};
use crate::external::cards::{CardKey, CardNote, OpenItem, Stage};
use crate::shared::lifecycle::Lifecycle;

/// The board's groups, in screen order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Group {
    /// Faults, such as a stopped `hub watch`, then what the read could
    /// not get, such as a failed `Cards::list_cards` call.
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
            Group::Problems => "Problems",
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
            Group::Problems => "problems",
            Group::Active => "active",
            Group::InReview => "in-review",
            Group::Blocked => "blocked",
            Group::NeedsYou => "needs-you",
            Group::Todo => "todo",
            Group::Draft => "draft",
        }
    }

    /// Returns the color of the group's count, one of the 16 colors the
    /// terminal's own theme sets or its default, so a phone app's theme
    /// keeps them readable. Cyan means at work, and no group count
    /// counts work, so none takes it.
    pub(super) fn color(self) -> Color {
        match self {
            Group::Problems => Color::Red,
            Group::NeedsYou => Color::Magenta,
            Group::InReview => Color::Green,
            Group::Blocked => Color::Yellow,
            Group::Active | Group::Todo | Group::Draft => Color::Reset,
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

    /// Returns the word's color. Yellow marks a card held up with no
    /// worker, as it marks the Blocked group.
    pub(super) fn color(self) -> Color {
        match self {
            Status::You => Color::Magenta,
            Status::Run => Color::Cyan,
            Status::Rev => Color::Green,
            Status::None => Color::Yellow,
            Status::Problem => Color::Red,
            Status::Idle => Color::Reset,
        }
    }
}

/// One line of the board's queue, the list of what waits on the user
/// and what runs, such as ` run  DEMO-KB-001 Faster search  DevEnv`.
/// A line is about a card, a question an agent asked the user, a
/// waiting session or a problem.
#[derive(Debug)]
pub(super) struct Item {
    /// Stays the same across reads, so the choice can follow the row.
    /// It is a card key, a question title, a session name or the
    /// problem.
    pub id: String,
    /// The words in the middle of the line, such as
    /// `DEMO-KB-001 Faster search`.
    pub text: String,
    /// A short word for the row's right end, such as `DevEnv` for the
    /// project folder of a card or of a question's card.
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
    pub(super) fn pick_kind_word(self) -> &'static str {
        match self {
            Target::Card(_) => "card",
            Target::Question(_) => "question",
            Target::Session(_) => "session",
            Target::Problem(_) => "problem",
        }
    }
}

/// Returns every group with its items, in screen order. The
/// `Problems` group shows only when it holds a row, and the
/// group of cards waiting on people outside the team only when it
/// holds a card.
pub(super) fn build_groups(queue: &Queue) -> Vec<(Group, Vec<Item>)> {
    let list_cards_at = |stage: Stage| -> Vec<Item> {
        queue
            .cards
            .iter()
            .enumerate()
            .filter(|(_, card)| card.stage == stage)
            .map(|(index, card)| build_card_item(index, card, &queue.sessions))
            .collect()
    };
    let mut needs_you: Vec<Item> = queue
        .questions
        .iter()
        .enumerate()
        .map(|(index, question)| Item {
            id: question.format_id(),
            text: format!("{} {}", question.format_id(), question.title),
            tag: question.project.clone(),
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
                text: put_card_key_first(
                    CardKey::find_in_name(&session.name).as_ref(),
                    &format!("{}  {}", session.name, session.why),
                ),
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
        (Group::Active, list_cards_at(Stage::Active)),
        (Group::InReview, list_cards_at(Stage::InReview)),
        (Group::Blocked, list_cards_at(Stage::Blocked)),
        (Group::Todo, list_cards_at(Stage::Todo)),
        (Group::Draft, list_cards_at(Stage::Draft)),
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
pub(super) fn pick_glance_items(groups: &[(Group, Vec<Item>)]) -> Vec<(Group, &Item)> {
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
    let shown = pick_glance_items(groups);
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
/// ran at the read.
pub(super) fn write_next_lines(groups: &[(Group, Vec<Item>)], no_hub: bool) -> NextLines {
    let find_items = |wanted: Group| {
        groups
            .iter()
            .find(|(group, _)| *group == wanted)
            .map_or(&[][..], |(_, items)| items)
    };
    let mut you: Vec<String> = find_items(Group::NeedsYou)
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
    let then = match find_items(Group::InReview).len() {
        0 => "then: no reviews".to_owned(),
        1 => "then: review 1 card".to_owned(),
        count => format!("then: review {count} cards"),
    };
    let mut hub: Vec<String> = find_items(Group::Todo)
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
        Target::Card(index) => describe_card(&queue.cards[index], &queue.sessions, note),
        Target::Question(index) => {
            let question = &queue.questions[index];
            let mut lines = vec![question.title.clone(), question.format_label()];
            lines.extend(
                question
                    .asked_on
                    .map(|asked_on| describe_question_age(asked_on, queue.read_on)),
            );
            lines.push(String::new());
            lines.extend(question.lines.iter().cloned());
            lines
        }
        Target::Session(index) => {
            let session = &queue.waiting[index];
            let mut lines = vec![format!("{} waits on you: {}", session.name, session.why)];
            lines.extend(session.card.iter().map(|card| format!("card {card}")));
            lines.extend(describe_worker_status(&queue.sessions, &session.name));
            lines
        }
        Target::Problem(index) => vec![queue.problems[index].clone()],
    }
}

/// Returns a card's details page lines, such as its title and open
/// questions, then `DEMO-KB-001  in-review  Demo`, its workers and their
/// status lines, next step, PR links and criteria.
pub(super) fn describe_card(
    card: &CardLine,
    sessions: &[SessionRow],
    note: Option<&CardNote>,
) -> Vec<String> {
    let mut lines = vec![card.title.clone()];
    if let Some(note) = note {
        lines.extend(note.open_questions.iter().map(format_open_item));
    }
    lines.extend([
        String::new(),
        format!(
            "{}  {}  {}",
            card.key.as_str(),
            card.stage.as_str(),
            card.project
        ),
    ]);
    lines.push(if card.workers.is_empty() {
        "no worker".to_owned()
    } else {
        format!("by {}", card.workers.join(", "))
    });
    for name in &card.workers {
        lines.extend(describe_worker_status(sessions, name));
    }
    let Some(note) = note else {
        return lines;
    };
    lines.extend(find_next_step(note).map(|step| format!("next: {step}")));
    lines.extend(note.prs.iter().cloned());
    if !note.criteria.is_empty() {
        lines.push(String::new());
        lines.extend(note.criteria.iter().cloned());
    }
    lines
}

/// Returns an open question's line on its card's details page, such as
/// `Q3, asked 2026-11-25: Deploy tonight?` or `Q2, undated: Which bucket?`.
fn format_open_item(item: &OpenItem) -> String {
    match item.asked {
        Some(date) => format!("Q{}, asked {date}: {}", item.number, item.text),
        None => format!("Q{}, undated: {}", item.number, item.text),
    }
}

/// Returns a worker's status line with its age, such as
/// `api-build 12m ago: tests pass`, or `None` with no status line.
fn describe_worker_status(sessions: &[SessionRow], name: &str) -> Option<String> {
    let status = sessions
        .iter()
        .find(|row| row.name == name)?
        .status
        .as_ref()?;
    (!status.line.is_empty()).then(|| format!("{name} {} ago: {}", status.age, status.line))
}

/// Returns how long ago a question was asked, in whole days, such as
/// `asked 2 days ago`.
fn describe_question_age(asked_on: Date, read_on: Date) -> String {
    match asked_on
        .until(read_on)
        .map_or(0, |span| span.get_days())
        .max(0)
    {
        0 => "asked today".to_owned(),
        1 => "asked 1 day ago".to_owned(),
        days => format!("asked {days} days ago"),
    }
}

/// Returns a board line's words with the card key in front, such as
/// `DEMO-KB-001 Faster search` from `Faster search`. A card key is the
/// ID at the start of a card's name. Words with no card, or that
/// already start with the card key, come back unchanged. Empty words
/// give the card key alone.
pub(super) fn put_card_key_first(key: Option<&CardKey>, words: &str) -> String {
    match key {
        Some(key) if words.is_empty() => key.as_str().to_owned(),
        Some(key) if words.starts_with(key.as_str()) => words.to_owned(),
        Some(key) => format!("{} {words}", key.as_str()),
        None => words.to_owned(),
    }
}

/// Returns the queue line of one card, such as
/// ` run  DEMO-KB-001 Faster search  DevEnv`, where `DevEnv` is the
/// card's project folder. `sessions` give the states of its workers.
fn build_card_item(index: usize, card: &CardLine, sessions: &[SessionRow]) -> Item {
    Item {
        id: card.key.as_str().to_owned(),
        text: put_card_key_first(Some(&card.key), &card.title),
        tag: card.project.clone(),
        status: decide_card_status(card, sessions),
        target: Target::Card(index),
    }
}

/// Returns a card's status from its worker sessions. A worker that
/// waits on the user wins over one at work, and one at work wins over
/// the card needing the user's review.
fn decide_card_status(card: &CardLine, sessions: &[SessionRow]) -> Status {
    let rows: Vec<&SessionRow> = card
        .workers
        .iter()
        .filter_map(|name| sessions.iter().find(|row| row.name == *name))
        .collect();
    if rows.iter().any(|row| row.state == Lifecycle::WaitsOnUser) {
        Status::You
    } else if rows.iter().any(|row| row.working) {
        Status::Run
    } else if card.stage == Stage::InReview {
        Status::Rev
    } else if rows.is_empty() {
        Status::None
    } else {
        Status::Idle
    }
}

/// Returns the card's newest `next:` Log line, else a step from its
/// newest PR link or first plan. A link alone cannot tell a merged PR
/// from an open one, so the hub's `next:` line wins.
fn find_next_step(note: &CardNote) -> Option<String> {
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

    use crate::board::queue::WorkerStatus;
    use crate::board::view::tests::make_sample_queue;

    /// A card's open questions show under its title, with their Q
    /// numbers, so the user sees what waits and how to answer it
    /// without opening the card note.
    #[test]
    fn describe_card_lists_open_questions_under_title() {
        let queue = make_sample_queue();
        let card = &queue.cards[0];
        let note = CardNote {
            open_questions: vec![
                OpenItem {
                    number: 3,
                    asked: Some("2026-11-25".parse().unwrap()),
                    text: "Deploy tonight?".to_owned(),
                    lines: vec!["- A. Yes.".to_owned()],
                },
                OpenItem {
                    number: 4,
                    asked: None,
                    text: "Which bucket?".to_owned(),
                    lines: Vec::new(),
                },
            ],
            ..CardNote::default()
        };

        let lines = describe_card(card, &queue.sessions, Some(&note));

        assert_eq!(
            lines[..4],
            [
                card.title.clone(),
                "Q3, asked 2026-11-25: Deploy tonight?".to_owned(),
                "Q4, undated: Which bucket?".to_owned(),
                String::new(),
            ]
        );
    }

    /// A question's details page names its card, Q number and date
    /// under the title, so the user knows which reply key answers it.
    #[test]
    fn describe_item_shows_question_label_under_title() {
        let queue = make_sample_queue();

        let lines = describe_item(&queue, Target::Question(0), None);

        assert_eq!(
            lines[..3],
            [
                "Deploy tonight?".to_owned(),
                "DEMO-KB-001 Q1, undated".to_owned(),
                String::new(),
            ]
        );
    }

    /// A worker that runs on a card needing the user's review, such as
    /// one fixing review comments, means the card is in work now. Shown
    /// as `rev`, it would look like it waits for the user's review.
    #[test]
    fn build_groups_marks_working_review_card_run() {
        let mut queue = make_sample_queue();
        let docs_fix = queue.sessions.iter_mut().find(|row| row.name == "docs-fix");
        docs_fix.unwrap().working = true;

        let groups = build_groups(&queue);

        let (_, in_review) = groups
            .iter()
            .find(|(group, _)| *group == Group::InReview)
            .unwrap();
        assert_eq!(in_review[0].status, Status::Run);
    }

    /// The user opens a card to see how far its work got. Its page must
    /// show each worker's status line with its age, or the user has to
    /// find the worker's pane to learn it.
    #[test]
    fn describe_card_shows_worker_status_line() {
        let mut queue = make_sample_queue();
        let api_build = queue
            .sessions
            .iter_mut()
            .find(|row| row.name == "api-build");
        api_build.unwrap().status = Some(WorkerStatus {
            line: "search tests pass".to_owned(),
            age: "12m".to_owned(),
        });

        let lines = describe_card(&queue.cards[0], &queue.sessions, None);

        let expected = "api-build 12m ago: search tests pass".to_owned();
        assert!(lines.contains(&expected), "{lines:?}");
    }

    /// An agent's question often starts with its card key already, such
    /// as `DEMO-KB-001: Deploy tonight?`, and a 40-column phone line has
    /// no room for the card key twice. A session name, such as
    /// `demo-kb-001-build`, is in lower case, so it still gets the card
    /// key.
    #[test]
    fn put_card_key_first_keeps_existing_key() {
        let key: CardKey = "DEMO-KB-001".parse().unwrap();
        let cases = [
            (
                "DEMO-KB-001: Deploy tonight?",
                "DEMO-KB-001: Deploy tonight?",
            ),
            (
                "demo-kb-001-build reported",
                "DEMO-KB-001 demo-kb-001-build reported",
            ),
        ];

        for (words, expected) in cases {
            let text = put_card_key_first(Some(&key), words);

            assert_eq!(text, expected, "{words}");
        }
    }
}
