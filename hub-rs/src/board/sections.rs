//! Sorts a `Queue` into the board's sections, in the order the user
//! reads them, and counts what waits for the top line. Read
//! `build_sections` first.

use jiff::civil::{Date, DateTime};
use ratatui::style::Color;

use super::queue::{CardLine, PullRequest, Question, Queue, SessionLine, Urgency};
use crate::external::cards::{BoardCounts, Stage};
use crate::shared::lifecycle::{KeptReason, Lifecycle};

/// The board's sections, in the order they show.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum SectionId {
    Problems,
    NeedsYou,
    InReview,
    WorkingOnNow,
    PullRequests,
    Later,
    Optional,
    ReviewNotes,
    Hub,
    Working,
    Parked,
    SafeToClose,
    Kept,
}

impl SectionId {
    /// Returns the heading's words, in capitals so a heading reads as
    /// one on a screen with no bold.
    pub(super) fn title(self) -> &'static str {
        match self {
            SectionId::Problems => "PROBLEMS",
            SectionId::NeedsYou => "NEEDS YOU",
            SectionId::InReview => "IN REVIEW",
            SectionId::WorkingOnNow => "WORKING ON NOW",
            SectionId::PullRequests => "PULL REQUESTS",
            SectionId::Later => "LATER",
            SectionId::Optional => "OPTIONAL",
            SectionId::ReviewNotes => "PR NOTES",
            SectionId::Hub => "WAITING ON HUB",
            SectionId::Working => "WORKING SESSIONS",
            SectionId::Parked => "PARKED",
            SectionId::SafeToClose => "SAFE TO CLOSE",
            SectionId::Kept => "KEPT",
        }
    }

    /// Returns the heading's color. Only the sections the user acts on
    /// get one, from the 8 basic colors every terminal has.
    pub(super) fn color(self) -> Option<Color> {
        match self {
            SectionId::Problems => Some(Color::Red),
            SectionId::NeedsYou => Some(Color::Yellow),
            SectionId::PullRequests => Some(Color::Cyan),
            _ => None,
        }
    }

    /// Returns true for side sections, which start folded so what needs
    /// the user and the cards fit on the first screen. Sessions are
    /// side sections, since the cards say what is worked on. PROBLEMS
    /// starts folded too, since its heading alone shows that a read
    /// failed.
    pub(super) fn starts_folded(self) -> bool {
        match self {
            SectionId::NeedsYou
            | SectionId::InReview
            | SectionId::WorkingOnNow
            | SectionId::PullRequests => false,
            SectionId::Problems
            | SectionId::Later
            | SectionId::Optional
            | SectionId::ReviewNotes
            | SectionId::Hub
            | SectionId::Working
            | SectionId::Parked
            | SectionId::SafeToClose
            | SectionId::Kept => true,
        }
    }
}

/// One section and its items.
pub(super) struct Section {
    pub id: SectionId,
    /// Words after the count, such as ` from 20:00`, or empty.
    pub note: String,
    pub items: Vec<Item>,
}

impl Section {
    /// Returns a section with no words after its count.
    pub(super) fn new(id: SectionId, items: Vec<Item>) -> Section {
        Section {
            id,
            note: String::new(),
            items,
        }
    }

    /// Returns the heading's text, such as `NEEDS YOU 3`.
    pub(super) fn heading(&self) -> String {
        format!("{} {}{}", self.id.title(), self.items.len(), self.note)
    }
}

/// One thing in a section.
pub(super) struct Item {
    /// Stays the same while the item exists, so a read keeps it chosen
    /// when its text changes.
    pub key: String,
    /// One character that says what the item is with no color, such as
    /// `?` for a question.
    pub mark: char,
    pub text: String,
    /// The full text the details screen shows.
    pub detail: String,
}

/// The numbers on the top line.
#[derive(Default)]
pub(super) struct Counts {
    pub you: usize,
    /// Questions folded until later or as optional.
    pub later: usize,
    pub pull_requests: usize,
    pub hub: usize,
    /// Busy cards and the limit, or `None` when `card` failed.
    pub cards: Option<(u32, u32)>,
}

impl Counts {
    /// Counts the items in `sections`, so the top line always agrees
    /// with the list.
    pub(super) fn new(sections: &[Section], cards: Option<&BoardCounts>) -> Counts {
        let count = |id: SectionId| {
            sections
                .iter()
                .find(|section| section.id == id)
                .map_or(0, |section| section.items.len())
        };
        Counts {
            you: count(SectionId::NeedsYou),
            later: count(SectionId::Later) + count(SectionId::Optional),
            pull_requests: count(SectionId::PullRequests),
            hub: count(SectionId::Hub),
            cards: cards.map(|cards| (cards.busy, cards.limit)),
        }
    }
}

/// Returns the sections that have items, in the order the user reads
/// them.
pub(super) fn build_sections(queue: &Queue) -> Vec<Section> {
    let sections = [
        Section::new(
            SectionId::Problems,
            line_items(&queue.problems, 'X', "problem", str::to_owned),
        ),
        Section::new(SectionId::NeedsYou, needs_you_items(queue)),
        Section::new(SectionId::InReview, review_card_items(queue)),
        Section::new(SectionId::WorkingOnNow, worked_card_items(queue)),
        Section::new(
            SectionId::PullRequests,
            queue.pull_requests.iter().map(pull_request_item).collect(),
        ),
        later_section(queue),
        Section::new(
            SectionId::Optional,
            questions_with(queue, Urgency::Optional),
        ),
        Section::new(
            SectionId::ReviewNotes,
            line_items(&queue.review_notes, '.', "note", str::to_owned),
        ),
        Section::new(SectionId::Hub, hub_items(queue)),
        Section::new(
            SectionId::Working,
            session_items(queue, '*', |state| state == Lifecycle::Working),
        ),
        Section::new(
            SectionId::Parked,
            session_items(queue, 'P', |state| {
                matches!(state, Lifecycle::ToPark | Lifecycle::Parked)
            }),
        ),
        Section::new(
            SectionId::SafeToClose,
            session_items(queue, 'C', |state| state == Lifecycle::ToClose),
        ),
        Section::new(
            SectionId::Kept,
            session_items(
                queue,
                'K',
                |state| matches!(state, Lifecycle::Kept(reason) if reason != KeptReason::ApprovalWaits),
            ),
        ),
    ];
    sections
        .into_iter()
        .filter(|section| !section.items.is_empty())
        .collect()
}

/// Returns the item a failed read adds under PROBLEMS. Its key stays
/// the same, so a second failure replaces the first.
pub(super) fn read_failure_item(reason: &str) -> Item {
    let text = format!("read failed: {reason}");
    Item {
        key: "problem read".to_owned(),
        mark: 'X',
        detail: text.clone(),
        text,
    }
}

/// Returns approval waits first, since they expire, then blocked
/// sessions, then questions in file order, oldest first.
fn needs_you_items(queue: &Queue) -> Vec<Item> {
    let mut items = session_items(queue, 'A', |state| {
        state == Lifecycle::Kept(KeptReason::ApprovalWaits)
    });
    items.extend(session_items(queue, 'B', |state| {
        state == Lifecycle::WaitsOnUser
    }));
    items.extend(questions_with(queue, Urgency::Now));
    items
}

/// Returns the cards in review, each with the one step the user takes
/// next.
fn review_card_items(queue: &Queue) -> Vec<Item> {
    queue
        .cards
        .iter()
        .filter(|card| card.stage == Stage::Review)
        .map(|card| card_item(card, 'V', &card.next_step))
        .collect()
}

/// Returns the cards worked on now. A draft or ready card with a worker
/// comes first, marked `!`, since its stage is out of date. Then come
/// active cards, `-` when no session works on them.
fn worked_card_items(queue: &Queue) -> Vec<Item> {
    let workers_words = |card: &CardLine| match card.workers.as_slice() {
        [] => "no worker".to_owned(),
        names => format!("by {}", names.join(", ")),
    };
    let out_of_date = queue
        .cards
        .iter()
        .filter(|card| matches!(card.stage, Stage::Draft | Stage::Ready))
        .map(|card| {
            let words = format!("in {}, {}", card.stage.as_str(), workers_words(card));
            card_item(card, '!', &words)
        });
    let active = queue
        .cards
        .iter()
        .filter(|card| card.stage == Stage::Active)
        .map(|card| {
            let mark = if card.workers.is_empty() { '-' } else { '*' };
            card_item(card, mark, &workers_words(card))
        });
    out_of_date.chain(active).collect()
}

/// Returns a card's item, its project and title on top and its key and
/// `status` on a line of their own, such as `DEMO-KB-010  review PR
/// #301`.
fn card_item(card: &CardLine, mark: char, status: &str) -> Item {
    let key = card.key.as_str();
    let about = match (card.project.as_str(), card.title.as_str()) {
        (_, "") => String::new(),
        ("", title) => format!("{title}\n"),
        (project, title) => format!("{project}: {title}\n"),
    };
    Item {
        key: format!("card {key}"),
        mark,
        text: format!("{about}{key}  {status}"),
        detail: format!(
            "{key} {}\n\nProject: {}\nStage: {}\n{status}",
            card.title,
            card.project,
            card.stage.as_str()
        ),
    }
}

/// Returns LATER, soonest first, with its first time in the heading,
/// such as `LATER 2 from 20:00`.
fn later_section(queue: &Queue) -> Section {
    let today = queue.read_at.date();
    let mut later: Vec<(&Question, DateTime)> = queue
        .questions
        .iter()
        .filter_map(|question| match question.urgency {
            Urgency::Later(at) => Some((question, at)),
            Urgency::Now | Urgency::Optional => None,
        })
        .collect();
    later.sort_by_key(|&(_, at)| at);
    let note = later.first().map_or(String::new(), |&(_, at)| {
        format!(" from {}", short_time(at, today))
    });
    let items = later
        .iter()
        .map(|&(question, at)| question_item(question, format!("{} ", short_time(at, today))))
        .collect();
    Section {
        id: SectionId::Later,
        note,
        items,
    }
}

/// Returns sessions waiting on the hub, then unread reports from
/// sessions not among them, such as a closed worker's. An unread
/// report makes an idle session wait on the hub, so its row already
/// says so.
fn hub_items(queue: &Queue) -> Vec<Item> {
    let mut items = session_items(queue, 'H', |state| state == Lifecycle::WaitsOnHub);
    let waits_on_hub = |name: &String| {
        queue
            .sessions
            .iter()
            .any(|session| &session.name == name && session.state == Lifecycle::WaitsOnHub)
    };
    items.extend(
        queue
            .unread
            .iter()
            .filter(|name| !waits_on_hub(name))
            .map(|name| Item {
                key: format!("report {name}"),
                mark: 'R',
                text: format!("report from {name}"),
                detail: format!("{name} wrote a report the hub has not read."),
            }),
    );
    items
}

/// Returns the questions with `urgency`, in file order.
fn questions_with(queue: &Queue, urgency: Urgency) -> Vec<Item> {
    queue
        .questions
        .iter()
        .filter(|question| question.urgency == urgency)
        .map(|question| question_item(question, String::new()))
        .collect()
}

/// Returns a question's item, its title after `prefix`. Its details
/// show the whole entry, choices and `Recommended:` included.
fn question_item(question: &Question, prefix: String) -> Item {
    Item {
        key: format!("ask {}", question.title),
        mark: '?',
        text: format!("{prefix}{}", question.title),
        detail: format!("{}\n\n{}", question.title, question.lines.join("\n")),
    }
}

/// Returns a pull request's item, such as `301 uploads: retry`. Its
/// details show every cell of its row.
fn pull_request_item(pull_request: &PullRequest) -> Item {
    Item {
        key: format!("pr {}", pull_request.number),
        mark: '#',
        text: format!(
            "{} {}",
            pull_request.number.trim_start_matches('#'),
            pull_request.what
        ),
        detail: pull_request.cells.join("\n"),
    }
}

/// Returns an item per line of text, with `detail` making its details.
fn line_items(
    lines: &[String],
    mark: char,
    kind: &str,
    detail: impl Fn(&str) -> String,
) -> Vec<Item> {
    lines
        .iter()
        .map(|line| Item {
            key: format!("{kind} {line}"),
            mark,
            text: line.clone(),
            detail: detail(line),
        })
        .collect()
}

/// Returns an item for each session whose state is `wanted`, in the
/// queue's order.
fn session_items(queue: &Queue, mark: char, wanted: impl Fn(Lifecycle) -> bool) -> Vec<Item> {
    queue
        .sessions
        .iter()
        .filter(|session| wanted(session.state))
        .map(|session| session_item(session, mark))
        .collect()
}

/// Returns a session's item, such as `docs-fix  unread report`. A why
/// of `-` says nothing, so the row shows the name alone.
fn session_item(session: &SessionLine, mark: char) -> Item {
    let why = (session.why != "-").then_some(session.why.as_str());
    let card = session
        .card
        .as_ref()
        .map_or(String::new(), |card| format!(", card {card}"));
    let state = match session.state {
        Lifecycle::Working => "is working",
        Lifecycle::WaitsOnHub => "waits on the hub",
        Lifecycle::WaitsOnUser => "waits on you",
        Lifecycle::Kept(_) => "is kept open",
        Lifecycle::ToClose => "is safe to close",
        Lifecycle::ToPark => "is ready to park",
        Lifecycle::Parked => "is parked",
    };
    Item {
        key: format!("session {}", session.name),
        mark,
        text: match why {
            Some(why) => format!("{}  {why}", session.name),
            None => session.name.clone(),
        },
        detail: match why {
            Some(why) => format!("{} {state}, {why}{card}", session.name),
            None => format!("{} {state}{card}", session.name),
        },
    }
}

/// Returns `20:00` for a time today, else `11-12 09:30`.
fn short_time(at: DateTime, today: Date) -> String {
    let format = if at.date() == today {
        "%H:%M"
    } else {
        "%m-%d %H:%M"
    };
    at.strftime(format).to_string()
}
