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
//! on the line above it.

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
