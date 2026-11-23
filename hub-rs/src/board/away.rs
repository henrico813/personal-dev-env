//! Reads what happened in the hubs' run folders for the board's AWAY
//! block, which lists worker reports and the hub's `notes.md` lines,
//! each with its time. Read `read_events` first.

use jiff::Timestamp;
use jiff::civil::{Date, Time};
use jiff::tz::TimeZone;

use super::machine::FeedLine;
use crate::external::cards::CardKey;
use crate::shared::HubError;
use crate::shared::run_folder::RunFolder;

/// One thing that happened, such as a report or a `notes.md` line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AwayEvent {
    pub at: Timestamp,
    /// `at` in the board's time zone, such as `10:42`.
    pub time: String,
    /// What happened, such as `api-build reported`.
    pub text: String,
}

/// Returns every report in `feed` and every `notes.md` line of `runs`,
/// newest first, with times in `time_zone`.
///
/// # Errors
///
/// `HubError::File` when a `notes.md` cannot be read.
pub(crate) fn read_events(
    runs: &[&RunFolder],
    feed: &[FeedLine],
    time_zone: &TimeZone,
) -> Result<Vec<AwayEvent>, HubError> {
    let mut events: Vec<AwayEvent> = feed
        .iter()
        .map(|report| AwayEvent {
            at: report.stamp,
            time: report.at.clone(),
            text: format!("{} reported", report.name),
        })
        .collect();
    for run in runs {
        let notes = run.read_optional_file("notes.md")?;
        events.extend(parse_note_lines(&notes, time_zone));
    }
    events.sort_by_key(|event| std::cmp::Reverse(event.at));
    Ok(events)
}

/// Returns the card key in an event's text, such as `DEMO-KB-001` from
/// `demo-kb-001-build reported`. An event has no card field, so the
/// card key comes from the first word that holds one. An event whose
/// words hold no card key gives `None`.
pub(crate) fn find_event_card(event: &AwayEvent) -> Option<CardKey> {
    event.text.split_whitespace().find_map(|word| {
        // Skip a bracket before the card key, since a hub's note can hold
        // `(DEMO-KB-010 in-review)`.
        CardKey::find_in_name(word.trim_start_matches(|letter: char| !letter.is_alphanumeric()))
    })
}

/// Returns the events after `since`, the start of the user's visit, or
/// every event when no time was saved. `events` come newest first.
pub(crate) fn find_events_since(events: &[AwayEvent], since: Option<Timestamp>) -> &[AwayEvent] {
    let count = since.map_or(events.len(), |since| {
        events.iter().take_while(|event| event.at > since).count()
    });
    &events[..count]
}

/// Returns the words after `AWAY` in its heading, such as `3 new`, or
/// `newest` when no time was saved and AWAY shows the newest events.
pub(crate) fn describe_new_count(since: Option<Timestamp>, count: usize) -> String {
    match since {
        Some(_) => format!("{count} new"),
        None => "newest".to_owned(),
    }
}

/// Returns the `- HHMM TEXT` lines of a `notes.md`, each dated by the
/// `## YYYY-MM-DD` heading above it. A line before any heading, or in
/// another form, is left out.
fn parse_note_lines(text: &str, time_zone: &TimeZone) -> Vec<AwayEvent> {
    let mut day: Option<Date> = None;
    let mut events = Vec::new();
    for line in text.lines() {
        if let Some(heading) = line.strip_prefix("## ") {
            day = heading.trim().parse().ok();
            continue;
        }
        let Some((stamp, words)) = line
            .strip_prefix("- ")
            .and_then(|rest| rest.split_once(' '))
        else {
            continue;
        };
        if let Some(at) = day.and_then(|day| find_note_time(day, stamp, time_zone)) {
            events.push(AwayEvent {
                at,
                time: format!("{}:{}", &stamp[..2], &stamp[2..]),
                text: words.to_owned(),
            });
        }
    }
    events
}

/// Returns `day` at `stamp`, such as `0907`, in `time_zone`, or `None`
/// when `stamp` is not four digits of a time.
fn find_note_time(day: Date, stamp: &str, time_zone: &TimeZone) -> Option<Timestamp> {
    if stamp.len() != 4 || !stamp.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let time = Time::new(stamp[..2].parse().ok()?, stamp[2..].parse().ok()?, 0, 0).ok()?;
    let zoned = day.to_datetime(time).to_zoned(time_zone.clone()).ok()?;
    Some(zoned.timestamp())
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs;

    /// AWAY tells the user what happened while they were away, so the
    /// reports and the hub's notes must come in one list, newest first,
    /// each at its own time. A note dated by the wrong heading, or a
    /// stray line read as an event, puts the wrong news on top.
    #[test]
    fn read_events_lists_reports_notes_newest_first() {
        let path = std::env::temp_dir().join(format!("hub-read-events-{}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        let notes = "\
- 0700 a line before any heading
## 2026-11-14
- 2350 closed old-search (card done)
## 2026-11-15
- 0905 launched api-build on DEMO-KB-001
- not a timed line
";
        fs::write(path.join("notes.md"), notes).unwrap();
        let run = RunFolder::open(&path).unwrap();
        let report = FeedLine {
            at: "09:01".to_owned(),
            stamp: "2026-11-15T09:01:30Z".parse().unwrap(),
            unread: true,
            name: "docs-fix".to_owned(),
        };

        let events = read_events(&[&run], &[report], &TimeZone::UTC).unwrap();

        fs::remove_dir_all(&path).unwrap();
        let shown: Vec<(&str, &str)> = events
            .iter()
            .map(|event| (event.time.as_str(), event.text.as_str()))
            .collect();
        assert_eq!(
            shown,
            [
                ("09:05", "launched api-build on DEMO-KB-001"),
                ("09:01", "docs-fix reported"),
                ("23:50", "closed old-search (card done)"),
            ]
        );
    }
}
