//! Reads the entries of `decisions.md`, the questions that wait on the
//! user. An entry looks like this.
//!
//! ```text
//! ## New model overnight
//!
//! Deploy the new model tonight so you can test it in the morning?
//!
//! - A. Deploy from the branch, with the undo path ready.
//! - B. Wait until you return.
//!
//! Recommended: A, since you want to test it in the morning.
//! If no reply: A.
//! Depends on: your rule of 2026-11-09 to test it in the morning.
//! ```
//!
//! `Depends on:` is optional. A line that starts with a space carries
//! on the line above it. A `Not before: 2026-11-11 20:00` line puts the
//! question off until that local time, and `Priority: optional` marks a
//! side question.

use std::collections::BTreeMap;

use jiff::Zoned;
use jiff::civil::DateTime;

use crate::external::cards::{CardKey, Stage};

/// The form of a `Not before:` time, such as `2026-11-11 20:00`.
const NOT_BEFORE_FORMAT: &str = "%Y-%m-%d %H:%M";

/// Whether an entry waits on the user now, from its `Not before:` and
/// `Priority:` lines.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Urgency {
    Now,
    /// Not before this local time, which is still ahead. It wins over
    /// `Priority: optional`.
    Later(DateTime),
    /// The hub marked it `Priority: optional`.
    Optional,
}

/// One `## <title>` entry of `decisions.md`.
pub(crate) struct DecisionEntry {
    pub title: String,
    /// Its lines, blank lines left out.
    pub lines: Vec<String>,
}

/// Returns each entry in file order. Lines before the first title are
/// left out.
pub(crate) fn read_decisions(text: &str) -> Vec<DecisionEntry> {
    let mut entries: Vec<DecisionEntry> = Vec::new();
    for line in text.lines() {
        if let Some(title) = line.strip_prefix("## ") {
            entries.push(DecisionEntry {
                title: title.trim().to_owned(),
                lines: Vec::new(),
            });
        } else if let Some(entry) = entries.last_mut()
            && !line.trim().is_empty()
        {
            entry.lines.push(line.trim_end().to_owned());
        }
    }
    entries
}

impl DecisionEntry {
    /// Returns whether the entry waits on the user at `now`. A
    /// `Not before:` time that does not read counts as no time, so a
    /// typo never hides a question. An entry whose card is accepted or
    /// dropped in `stages` is optional, since its work is done.
    pub(crate) fn urgency(&self, now: &Zoned, stages: &BTreeMap<CardKey, Stage>) -> Urgency {
        let card_done = matches!(
            self.find_card(stages),
            Some((_, Stage::Accepted | Stage::Dropped))
        );
        let later = self
            .read_not_before()
            .ok()
            .flatten()
            .and_then(|when| when.to_zoned(now.time_zone().clone()).ok())
            .filter(|when| when > now)
            .map(|when| when.datetime());
        match later {
            Some(when) => Urgency::Later(when),
            None if card_done || self.find_field("Priority:") == Some("optional") => {
                Urgency::Optional
            }
            None => Urgency::Now,
        }
    }

    /// Returns the card the entry belongs to and its stage, when its
    /// title and lines name exactly one key and `stages` holds it.
    pub(crate) fn find_card<'a>(
        &self,
        stages: &'a BTreeMap<CardKey, Stage>,
    ) -> Option<(&'a CardKey, Stage)> {
        let text = format!("{}\n{}", self.title, self.lines.join("\n"));
        let mut keys = CardKey::find_all_in_text(&text);
        keys.sort();
        keys.dedup();
        match keys.as_slice() {
            [key] => stages.get_key_value(key).map(|(key, stage)| (key, *stage)),
            _ => None,
        }
    }

    /// Returns the `Not before:` time, or `None` when the entry has no
    /// such line. A time that does not read as `2026-11-11 20:00`, such
    /// as `22:30`, comes back as the error with the line's text.
    pub(crate) fn read_not_before(&self) -> Result<Option<DateTime>, &str> {
        let Some(when) = self.find_field("Not before:") else {
            return Ok(None);
        };
        DateTime::strptime(NOT_BEFORE_FORMAT, when)
            .map(Some)
            .map_err(|_| when)
    }

    /// Returns the text after the first line that starts with `name`.
    fn find_field(&self, name: &str) -> Option<&str> {
        self.lines
            .iter()
            .find_map(|line| line.strip_prefix(name))
            .map(str::trim)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use jiff::civil::date;

    /// The hub marks a question it must not ask yet with `Not before:`
    /// and a side item with `Priority: optional`. A question shown too
    /// early breaks a promise to the user, and one hidden after its time
    /// or by a mistyped time gets lost. A question about a card already
    /// accepted must leave the user's list without being lost.
    #[test]
    fn urgency_reads_not_before_and_priority() {
        let now: Zoned = "2026-11-11T13:00[America/Los_Angeles]".parse().unwrap();
        let tonight = date(2026, 11, 11).at(20, 0, 0, 0);
        let stages = [("DEMO-KB-001".parse().unwrap(), Stage::Accepted)].into();
        let cases = [
            (
                "ahead",
                "Not before: 2026-11-11 20:00",
                Urgency::Later(tonight),
            ),
            ("passed", "Not before: 2026-11-11 12:00", Urgency::Now),
            ("mistyped", "Not before: tonight", Urgency::Now),
            ("optional", "Priority: optional", Urgency::Optional),
            ("card done", "For DEMO-KB-001.", Urgency::Optional),
        ];

        for (case_name, mark, urgency) in cases {
            let entry = DecisionEntry {
                title: "Demo: pick a name".to_owned(),
                lines: vec!["Which name?".to_owned(), mark.to_owned()],
            };

            assert_eq!(entry.urgency(&now, &stages), urgency, "{case_name}");
        }
    }
}
