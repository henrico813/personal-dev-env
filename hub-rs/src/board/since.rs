//! Groups the events since the user's last visit by card, for the
//! SINCE YOU LEFT block and page, where each card is one row named by
//! its newest event, such as `finished`. Read `group_events_by_card`
//! first.

use super::away::{self, AwayEvent};
use super::queue::CardLine;
use crate::external::cards::CardKey;
use crate::shared::lifecycle;

/// One SINCE YOU LEFT row, the events of one card, newest first.
pub(super) struct CardEvents<'a> {
    /// `None` for the events that name no card, such as the hub's own
    /// notes.
    pub card: Option<CardKey>,
    /// For events that name no card, the auto run whose hub they name,
    /// such as 24 for `auto-hub-24 reported`.
    pub run: Option<u64>,
    /// Never empty.
    pub events: Vec<&'a AwayEvent>,
}

impl CardEvents<'_> {
    /// Returns what keeps the row open across reads, its card key,
    /// `run-24` for auto run 24's hub, or `hub`.
    pub(super) fn format_row_id(&self) -> String {
        match (&self.card, self.run) {
            (Some(key), _) => key.as_str().to_owned(),
            (None, Some(run)) => format!("run-{run}"),
            (None, None) => "hub".to_owned(),
        }
    }

    /// Returns the row's words, such as `DEMO-KB-001 Faster search`,
    /// the card key alone for a card no longer on the board, or
    /// `Hub of auto run 24` for that run's hub.
    pub(super) fn describe_card(&self, cards: &[CardLine]) -> String {
        let Some(key) = &self.card else {
            return match self.run {
                Some(run) => format!("Hub of auto run {run}"),
                None => "hub".to_owned(),
            };
        };
        let title = cards
            .iter()
            .find(|card| card.key == *key)
            .map_or("", |card| card.title.as_str());
        format!("{} {title}", key.as_str()).trim_end().to_owned()
    }
}

/// Returns one entry per card named in `events`, which come newest
/// first, so the card with the newest event comes first. Events that
/// name no card share one entry per auto run whose hub they name, and
/// one entry for the rest.
pub(super) fn group_events_by_card(events: &[AwayEvent]) -> Vec<CardEvents<'_>> {
    let mut rows: Vec<CardEvents<'_>> = Vec::new();
    for event in events {
        let card = away::find_event_card(event);
        let run = match card {
            Some(_) => None,
            None => find_event_run(event),
        };
        match rows
            .iter_mut()
            .find(|row| row.card == card && row.run == run)
        {
            Some(row) => row.events.push(event),
            None => rows.push(CardEvents {
                card,
                run,
                events: vec![event],
            }),
        }
    }
    rows
}

/// Returns the auto run whose hub `event` names, such as 24 for
/// `auto-hub-24 reported`.
fn find_event_run(event: &AwayEvent) -> Option<u64> {
    event
        .text
        .split_whitespace()
        .find_map(lifecycle::parse_auto_hub_number)
}

/// Returns an event's word, `report`, `new worker` or `finished`, from
/// the lines `away::read_events`, `hub launch` and the parking of a
/// worker write. Any other event, such as a `closed` line or the hub's
/// own note, gives its own text, since its source names no word.
pub(super) fn describe_event(event: &AwayEvent) -> &str {
    let text = event.text.as_str();
    if text.ends_with(" reported") {
        "report"
    } else if text.starts_with("started ") && text.contains(" from ") {
        "new worker"
    } else if text.starts_with("parked ") && text.ends_with(" in-review)") {
        "finished"
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An event at `time` on one day, with `text`.
    fn make_event(time: &str, text: &str) -> AwayEvent {
        AwayEvent {
            at: format!("2026-11-15T{time}:00Z").parse().unwrap(),
            time: time.to_owned(),
            text: text.to_owned(),
        }
    }

    /// SINCE YOU LEFT shows one row per card, so a card's events must
    /// share one entry, with the card of the newest event first. A
    /// card split over two rows, or put below an older card, hides
    /// what just changed.
    #[test]
    fn group_events_by_card_puts_newest_first() {
        let events = [
            make_event("10:05", "demo-kb-002-build reported"),
            make_event("10:01", "parked demo-kb-001-build (DEMO-KB-001 in-review)"),
            make_event("09:50", "answer A sent to the worker"),
            make_event("09:40", "demo-kb-002-build reported"),
        ];

        let rows = group_events_by_card(&events);

        let shown: Vec<String> = rows
            .iter()
            .map(|row| format!("{} {}", row.format_row_id(), row.events.len()))
            .collect();
        assert_eq!(shown, ["DEMO-KB-002 2", "DEMO-KB-001 1", "hub 1"]);
    }

    /// An auto run's hub reports to the front hub, and its events name
    /// no card. They must get a row named by the run, apart from the
    /// front hub's own events, or the user cannot tell which run spoke.
    #[test]
    fn group_events_by_card_splits_auto_hubs() {
        let events = [
            make_event("10:05", "auto-hub-24 reported"),
            make_event("09:50", "answer A sent to the worker"),
        ];

        let rows = group_events_by_card(&events);

        let names: Vec<String> = rows.iter().map(|row| row.describe_card(&[])).collect();
        assert_eq!(names, ["Hub of auto run 24", "hub"]);
    }

    /// A row's word must come only from a line whose source gives one.
    /// An invented word, such as `finished` on a closed session, tells
    /// the user a card is done when it is not.
    #[test]
    fn describe_event_returns_word_per_event_kind() {
        let cases = [
            ("docs-fix reported", "report"),
            ("started docs-fix (p_1) from /h/handoff.md", "new worker"),
            ("parked docs-fix (DEMO-KB-001 in-review)", "finished"),
            ("closed docs-fix (card done)", "closed docs-fix (card done)"),
        ];

        for (text, word) in cases {
            assert_eq!(describe_event(&make_event("10:00", text)), word, "{text}");
        }
    }
}
