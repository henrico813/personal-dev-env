//! Sorts a `Queue` into the board's groups, such as one for cards
//! needing the user's review, and writes each row's words and its
//! details page. Nothing here draws. Read `build_groups` and
//! `describe_item` first.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

use jiff::civil::Date;
use ratatui::style::Color;

use super::pull_requests::{CheckResult, PullRequestLine};
use super::queue::{CardLine, Queue, SessionRow};
use crate::external::cards::{CardKey, CardNote, OpenItem, Stage};
use crate::shared::lifecycle::{self, Lifecycle};

/// The board's groups. `Panel::list_groups` gives each page's groups in
/// screen order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Group {
    NeedsYou,
    /// Active cards whose worker is at work.
    Working,
    NoWorker,
    /// Active cards with a worker, none of them at work, such as one
    /// that waits on the user.
    Idle,
    InReview,
    /// Open pull requests no IN REVIEW row shows.
    OtherPrs,
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
    /// Its pull request's check failed. Page 5 draws only the red
    /// color, not the `!` word.
    Problem,
}

impl Group {
    pub(super) fn title(self) -> &'static str {
        match self {
            Group::NeedsYou => "WAITING ON YOU",
            Group::Working => "WORKING",
            Group::NoWorker => "NO WORKER",
            Group::Idle => "IDLE",
            Group::InReview => "IN REVIEW",
            Group::OtherPrs => "OTHER PULL REQUESTS",
            Group::Blocked => "BLOCKED",
            Group::Todo => "UP NEXT",
            Group::Draft => "DRAFT",
        }
    }

    /// Returns the word that names the group in element ids, such as
    /// `group:in-review`.
    pub(super) fn slug(self) -> &'static str {
        match self {
            Group::NeedsYou => "needs-you",
            Group::Working => "working",
            Group::NoWorker => "no-worker",
            Group::Idle => "idle",
            Group::InReview => "in-review",
            Group::OtherPrs => "other-prs",
            Group::Blocked => "blocked",
            Group::Todo => "todo",
            Group::Draft => "draft",
        }
    }

    /// Returns the color of the group's count, one of the 16 colors the
    /// terminal's own theme sets or its default, so a phone app's theme
    /// keeps them readable. Cyan means at work, and yellow a card held
    /// up. `draw` dims the IDLE and DRAFT counts.
    pub(super) fn color(self) -> Color {
        match self {
            Group::NeedsYou => Color::Magenta,
            Group::Working => Color::Cyan,
            Group::InReview => Color::Green,
            Group::NoWorker | Group::Blocked => Color::Yellow,
            Group::Idle | Group::OtherPrs | Group::Todo | Group::Draft => Color::Reset,
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
    /// project folder of a card, or `answer` on a WAITING ON YOU row.
    pub tag: String,
    /// The dim line under the row, such as `today: Deploy tonight?`.
    pub detail: Option<String>,
    pub status: Status,
    pub target: Target,
}

/// The `Queue` entry an item comes from, by its place in that list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Target {
    Card(usize),
    Question(usize),
    Session(usize),
    /// A pull request in `Queue::pull_requests`.
    PullRequest(usize),
    /// The folded row of passing pull requests no card links, with how
    /// many it folds.
    PassingPrs(usize),
}

impl Target {
    /// Returns the word before an item's id in its element id, such as
    /// `card` in `card:DEMO-KB-007`.
    pub(super) fn pick_kind_word(self) -> &'static str {
        match self {
            Target::Card(_) => "card",
            Target::Question(_) => "question",
            Target::Session(_) => "session",
            Target::PullRequest(_) => "pr",
            Target::PassingPrs(_) => "prs",
        }
    }
}

/// Returns every group with its items. The Blocked group shows only
/// when it holds a card, since most boards have none.
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
    let (mut working, mut no_worker, mut idle) = (Vec::new(), Vec::new(), Vec::new());
    for item in list_cards_at(Stage::Active) {
        match item.status {
            Status::Run => working.push(item),
            Status::None => no_worker.push(item),
            // A worker that waits on the user is not at work. Its card
            // already shows under WAITING ON YOU.
            _ => idle.push(item),
        }
    }
    let (in_review, other_prs) = list_review_items(queue, list_cards_at(Stage::InReview));
    let mut groups = vec![
        (Group::NeedsYou, list_waiting_items(queue)),
        (Group::Working, working),
        (Group::NoWorker, no_worker),
        (Group::Idle, idle),
        (Group::InReview, in_review),
        (Group::OtherPrs, other_prs),
        (Group::Todo, list_cards_at(Stage::Todo)),
        (Group::Draft, list_cards_at(Stage::Draft)),
    ];
    let blocked = list_cards_at(Stage::Blocked);
    if !blocked.is_empty() {
        groups.push((Group::Blocked, blocked));
    }
    groups
}

/// Returns the WAITING ON YOU rows, one per card, where the card's
/// open questions and its waiting sessions share a row. A waiting
/// session with no card key has a row of its own, named by its auto
/// run, such as `Hub of auto run 24`, when it is that run's hub, else
/// by its session name. Rows whose newest
/// question was asked today come first, then `see tab` rows, then the
/// older questions, newest first.
///
/// It builds the rows from `queue.questions` and `queue.waiting`.
/// `read_open_questions` fills `queue.questions`, and `hub waiting`
/// reads its questions through it too.
pub(super) fn list_waiting_items(queue: &Queue) -> Vec<Item> {
    // Each row's id, a card key or a session name, with the places of
    // its questions and sessions in the queue.
    let mut rows: BTreeMap<String, (Vec<usize>, Vec<usize>)> = BTreeMap::new();
    for (index, question) in queue.questions.iter().enumerate() {
        let id = question.key.as_str().to_owned();
        rows.entry(id).or_default().0.push(index);
    }
    for (index, session) in queue.waiting.iter().enumerate() {
        let id = CardKey::find_in_name(&session.name)
            .map_or_else(|| session.name.clone(), |key| key.as_str().to_owned());
        rows.entry(id).or_default().1.push(index);
    }
    let mut items: Vec<Item> = rows
        .into_iter()
        .map(|(id, (questions, sessions))| build_waiting_item(queue, id, &questions, &sessions))
        .collect();
    items.sort_by_key(|item| match item.target {
        Target::Question(index) => {
            let question = &queue.questions[index];
            let rank = if question.asked_on == Some(queue.read_on) {
                0
            } else {
                2
            };
            (rank, Reverse((question.asked_on, question.number)))
        }
        _ => (1, Reverse((None, 0))),
    });
    items
}

/// Returns one WAITING ON YOU row from the places in `queue` of a
/// card's questions and waiting sessions, of which there is at least
/// one. With two or more questions, the row shows the newest one with
/// the oldest one's age and a count, such as
/// `16 days old, +3 questions: Who updates the docs?`.
fn build_waiting_item(queue: &Queue, id: String, questions: &[usize], sessions: &[usize]) -> Item {
    let order_by_age = |index: &&usize| {
        let question = &queue.questions[**index];
        (question.asked_on, question.number)
    };
    let newest = questions.iter().max_by_key(order_by_age).copied();
    let oldest = questions.iter().min_by_key(order_by_age).copied();
    let title = queue
        .cards
        .iter()
        .find(|card| card.key.as_str() == id)
        .map(|card| card.title.as_str())
        .or_else(|| newest.map(|index| queue.questions[index].title.as_str()))
        .unwrap_or_default();
    let text = match lifecycle::parse_auto_hub_number(&id) {
        Some(run) => format!("Hub of auto run {run}"),
        None => format!("{id} {title}").trim_end().to_owned(),
    };
    let (tag, detail, target) = match (newest, oldest) {
        (Some(newest), Some(oldest)) => {
            let age = format_question_age(queue.questions[oldest].asked_on, queue.read_on);
            let title = &queue.questions[newest].title;
            let detail = match questions.len() - 1 {
                0 => format!("{age}: {title}"),
                1 => format!("{age}, +1 question: {title}"),
                more => format!("{age}, +{more} questions: {title}"),
            };
            ("answer", detail, Target::Question(newest))
        }
        _ => {
            let first = sessions[0];
            let why = queue.waiting[first].why.clone();
            ("see tab", why, Target::Session(first))
        }
    };
    Item {
        id,
        text,
        tag: tag.to_owned(),
        detail: Some(detail),
        status: Status::You,
        target,
    }
}

/// Returns the IN REVIEW rows, each card with its pull request's line
/// under it, and the OTHER PULL REQUESTS rows, failing first. A pull
/// request belongs to a card only when the card's `prs:` link names it.
/// Passing pull requests of no card fold into one last row.
fn list_review_items(queue: &Queue, mut cards: Vec<Item>) -> (Vec<Item>, Vec<Item>) {
    let pull_requests = list_pull_requests(queue);
    let mut shown = BTreeSet::new();
    for item in &mut cards {
        let Target::Card(index) = item.target else {
            continue;
        };
        let links = queue
            .notes
            .get(&queue.cards[index].key)
            .map_or(&[][..], |note| note.prs.as_slice());
        let found = links.iter().find_map(|link| {
            pull_requests
                .iter()
                .position(|pull_request| pull_request.is_named_by(link))
        });
        if let Some(at) = found {
            let pull_request = &pull_requests[at];
            let line = format!(
                "{} #{} {}",
                find_project_label(queue, pull_request),
                pull_request.number,
                pull_request.check.as_word()
            );
            item.detail = Some(line.trim_end().to_owned());
            shown.insert(at);
        }
    }
    let mut others: Vec<usize> = (0..pull_requests.len())
        .filter(|at| !shown.contains(at))
        .collect();
    others.sort_by_key(|&at| pull_requests[at].check);
    let (folded, listed): (Vec<usize>, Vec<usize>) = others
        .into_iter()
        .partition(|&at| is_folded(queue, &pull_requests[at]));
    let mut rows: Vec<Item> = listed
        .into_iter()
        .map(|at| build_pr_item(queue, pull_requests, at))
        .collect();
    if !folded.is_empty() {
        let numbers: Vec<String> = folded
            .iter()
            .map(|&at| format!("#{}", pull_requests[at].number))
            .collect();
        rows.push(Item {
            id: "passing-prs".to_owned(),
            text: format!("+ {} pass, no card: {}", folded.len(), numbers.join(" ")),
            tag: String::new(),
            detail: None,
            status: Status::Rev,
            target: Target::PassingPrs(folded.len()),
        });
    }
    (cards, rows)
}

/// Returns an OTHER PULL REQUESTS row, such as
/// `DevEnv #60 DEMO-KB-002 Faster board`, with its check word as the
/// tag.
/// Its status word takes the check's color, red for a failed check.
fn build_pr_item(queue: &Queue, pull_requests: &[PullRequestLine], at: usize) -> Item {
    let pull_request = &pull_requests[at];
    let card = find_pr_card(queue, pull_request);
    Item {
        id: format!("{}#{}", pull_request.name_with_owner, pull_request.number),
        text: format!(
            "{} #{} {}",
            find_project_label(queue, pull_request),
            pull_request.number,
            put_card_key_first(card, &pull_request.title)
        ),
        tag: pull_request.check.as_word().to_owned(),
        detail: None,
        status: match pull_request.check {
            CheckResult::Fails => Status::Problem,
            CheckResult::Running => Status::Run,
            CheckResult::Pass | CheckResult::None => Status::Rev,
        },
        target: Target::PullRequest(at),
    }
}

/// Returns the project label of a pull request's row, such as
/// `DevEnv`, from a card whose `prs:` link names the same repository.
/// The `prs` list names no project, so without such a card in
/// `Queue::notes` the label is the repository name, such as `app`.
fn find_project_label<'a>(queue: &'a Queue, pull_request: &'a PullRequestLine) -> &'a str {
    queue
        .notes
        .values()
        .find(|note| {
            !note.project.is_empty()
                && note
                    .prs
                    .iter()
                    .any(|link| pull_request.shares_repo_with(link))
        })
        .map_or_else(
            || pull_request.find_repo_name(),
            |note| note.project.as_str(),
        )
}

/// Returns the pull requests the read got, empty without the `prs`
/// list.
fn list_pull_requests(queue: &Queue) -> &[PullRequestLine] {
    queue.pull_requests.as_deref().unwrap_or_default()
}

/// Tells whether `pull_request` folds into the `+ N pass, no card` row.
fn is_folded(queue: &Queue, pull_request: &PullRequestLine) -> bool {
    pull_request.check == CheckResult::Pass && find_pr_card(queue, pull_request).is_none()
}

/// Returns the card whose `prs:` link names `pull_request`. Only the
/// cards in `Queue::notes` are known.
fn find_pr_card<'a>(queue: &'a Queue, pull_request: &PullRequestLine) -> Option<&'a CardKey> {
    queue
        .notes
        .iter()
        .find(|(_, note)| note.prs.iter().any(|link| pull_request.is_named_by(link)))
        .map(|(key, _)| key)
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
        Target::PullRequest(at) => {
            let pull_request = &list_pull_requests(queue)[at];
            let mut lines = vec![
                pull_request.title.clone(),
                format!(
                    "{} #{}  {}",
                    pull_request.name_with_owner,
                    pull_request.number,
                    pull_request.check.as_word()
                ),
            ];
            let card = find_pr_card(queue, pull_request);
            lines.extend(card.map(|key| format!("card {}", key.as_str())));
            lines
        }
        Target::PassingPrs(_) => list_pull_requests(queue)
            .iter()
            .filter(|pull_request| is_folded(queue, pull_request))
            .map(|pull_request| format!("#{} {}", pull_request.number, pull_request.title))
            .collect(),
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
    match count_days_between(asked_on, read_on) {
        0 => "asked today".to_owned(),
        1 => "asked 1 day ago".to_owned(),
        days => format!("asked {days} days ago"),
    }
}

/// Returns a question's age on its WAITING ON YOU row, such as `today`,
/// `2 days old` or `undated`.
fn format_question_age(asked_on: Option<Date>, read_on: Date) -> String {
    let Some(asked_on) = asked_on else {
        return "undated".to_owned();
    };
    match count_days_between(asked_on, read_on) {
        0 => "today".to_owned(),
        1 => "1 day old".to_owned(),
        days => format!("{days} days old"),
    }
}

/// Returns the whole days from `asked_on` to `read_on`, never below 0.
fn count_days_between(asked_on: Date, read_on: Date) -> i32 {
    asked_on
        .until(read_on)
        .map_or(0, |span| span.get_days())
        .max(0)
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
        detail: None,
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

    use jiff::civil::date;

    use crate::board::queue::{SessionLine, WorkerStatus};
    use crate::board::view::tests::make_sample_queue;
    use crate::shared::questions::OpenQuestion;

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

    /// A card with a question and a waiting tab must take one WAITING
    /// ON YOU row, or the user counts two things to do where there is
    /// one. A tab with no question comes before older questions.
    #[test]
    fn list_waiting_items_merges_question_and_tab() {
        let mut queue = make_sample_queue();
        queue.waiting.push(SessionLine {
            name: "demo-kb-001-build".to_owned(),
            why: "needs you".to_owned(),
            card: None,
        });

        let items = list_waiting_items(&queue);

        let rows: Vec<(&str, &str, Option<&str>)> = items
            .iter()
            .map(|item| {
                (
                    item.text.as_str(),
                    item.tag.as_str(),
                    item.detail.as_deref(),
                )
            })
            .collect();
        assert_eq!(
            rows,
            [
                ("blocked-fix", "see tab", Some("needs you")),
                (
                    "DEMO-KB-001 Faster search",
                    "answer",
                    Some("undated: Deploy tonight?")
                ),
            ]
        );
    }

    /// An auto run's hub has no card, so its row must name the run it
    /// leads. A bare session name such as `auto-hub-24` makes the user
    /// work out which run waits on them.
    #[test]
    fn list_waiting_items_names_auto_run_hub() {
        let mut queue = make_sample_queue();
        queue.waiting = vec![SessionLine {
            name: "auto-hub-24".to_owned(),
            why: "needs you".to_owned(),
            card: None,
        }];

        let items = list_waiting_items(&queue);

        let texts: Vec<&str> = items.iter().map(|item| item.text.as_str()).collect();
        assert_eq!(texts, ["Hub of auto run 24", "DEMO-KB-001 Faster search"]);
    }

    /// A card with several questions keeps one row that says how many
    /// more wait, how old the oldest is and what the newest asks, so
    /// the user sees both the backlog and the next thing to answer.
    #[test]
    fn list_waiting_items_counts_extra_questions() {
        let mut queue = make_sample_queue();
        queue.questions[0].asked_on = Some(date(2026, 11, 12));
        let make_question = |number, title: &str, day| OpenQuestion {
            key: "DEMO-KB-001".parse().unwrap(),
            project: "Demo".to_owned(),
            number,
            title: title.to_owned(),
            lines: Vec::new(),
            sessions: Vec::new(),
            asked_on: Some(date(2026, 11, day)),
        };
        queue.questions.push(make_question(2, "Ship the fix?", 13));
        queue.questions.push(make_question(3, "Tag a release?", 14));

        let items = list_waiting_items(&queue);

        let row = items.iter().find(|item| item.id == "DEMO-KB-001").unwrap();
        assert_eq!(
            row.detail.as_deref(),
            Some("3 days old, +2 questions: Tag a release?")
        );
    }

    /// Page 5 lists the pull requests of no card failing first and
    /// folds the passing ones into one row, so a failing one is never
    /// lost below them on a phone.
    #[test]
    fn build_groups_folds_passing_prs_after_failing() {
        let mut queue = make_sample_queue();
        let make_pr = |number, title: &str, check| PullRequestLine {
            name_with_owner: "demo/app".to_owned(),
            number,
            title: title.to_owned(),
            check,
        };
        queue.pull_requests = Some(vec![
            make_pr(48, "Bump the linter", CheckResult::Pass),
            make_pr(301, "Retry uploads", CheckResult::Fails),
            make_pr(52, "Fix a typo", CheckResult::Pass),
            make_pr(60, "Cache builds", CheckResult::Running),
            make_pr(54, "Lint docs", CheckResult::Fails),
        ]);

        let groups = build_groups(&queue);

        let (_, other_prs) = groups
            .iter()
            .find(|(group, _)| *group == Group::OtherPrs)
            .unwrap();
        let others: Vec<&str> = other_prs.iter().map(|item| item.text.as_str()).collect();
        assert_eq!(
            others,
            [
                "Demo #54 Lint docs",
                "Demo #60 Cache builds",
                "+ 2 pass, no card: #48 #52"
            ]
        );
    }
}
