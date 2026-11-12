//! The board's list of rows. It tracks which sections are folded, which
//! row is chosen and which rows fit on screen. It draws nothing, so the
//! drawing code and taps read the same rows. Read
//! `SectionList::show_sections` and `SectionList::place` first.
//!
//! When the list starts inside a section, that section's heading is
//! pinned on the first line, so the user always sees where they are.
//! An item wider than the list wraps onto more rows, and the choice
//! moves by whole items.

use std::collections::HashMap;

use super::sections::{Item, Section, SectionId};

/// The columns before an item's text, its pointer, mark and a space.
pub(super) const ITEM_INDENT: usize = 3;

/// One row of the list, borrowed from its section.
pub(super) enum Row<'a> {
    Heading {
        section: &'a Section,
        folded: bool,
    },
    /// One wrapped line of an item. `first` is true on its first line.
    Item {
        section: &'a Section,
        item: &'a Item,
        line: &'a str,
        first: bool,
    },
}

/// Where one row comes from in `SectionList::sections`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RowRef {
    Heading(usize),
    /// The section, the item, and which of the item's wrapped lines.
    Item(usize, usize, usize),
}

/// The sections, their folds and the rows they make, the chosen row,
/// and the rows drawn on screen.
pub(super) struct SectionList {
    sections: Vec<Section>,
    /// The sections the user folded or opened. The others keep
    /// `SectionId::starts_folded`, and this outlives every read.
    folds: HashMap<SectionId, bool>,
    rows: Vec<RowRef>,
    /// Each row's wrapped text, empty for a heading.
    texts: Vec<String>,
    /// The first row of the chosen item, or the chosen heading.
    chosen: Option<usize>,
    /// The first row the list draws.
    offset: usize,
    /// False after a wheel turn, so the list stays where the user
    /// scrolled it even when the chosen row is off screen.
    follow_choice: bool,
    /// The list's height at the last draw.
    height: usize,
    /// The list's width at the last draw, which items wrap to.
    width: usize,
    /// The row drawn on each line of the list, so a tap finds its row.
    drawn: Vec<usize>,
}

impl SectionList {
    /// Returns an empty list with no row chosen.
    pub(super) fn new() -> SectionList {
        SectionList {
            sections: Vec::new(),
            folds: HashMap::new(),
            rows: Vec::new(),
            texts: Vec::new(),
            chosen: None,
            offset: 0,
            follow_choice: true,
            height: 0,
            width: 0,
            drawn: Vec::new(),
        }
    }

    /// Shows new sections. The chosen row stays chosen when it is still
    /// there, and the choice keeps its place otherwise.
    pub(super) fn show_sections(&mut self, sections: Vec<Section>) {
        let chosen_key = self.chosen_key();
        self.sections = sections;
        self.rebuild_rows(chosen_key);
    }

    /// Adds `item` under PROBLEMS in place of one with the same key,
    /// making the section when there is none.
    pub(super) fn add_problem(&mut self, item: Item) {
        let chosen_key = self.chosen_key();
        match self
            .sections
            .iter_mut()
            .find(|section| section.id == SectionId::Problems)
        {
            Some(section) => {
                section.items.retain(|old| old.key != item.key);
                section.items.push(item);
            }
            None => self
                .sections
                .insert(0, Section::new(SectionId::Problems, vec![item])),
        }
        self.rebuild_rows(chosen_key);
    }

    /// Wraps the items to `width` columns, then picks the first row to
    /// draw in `height` lines so the chosen row shows, unless the wheel
    /// moved the list, and records the row on each line.
    pub(super) fn place(&mut self, height: usize, width: usize) {
        self.height = height;
        if width == self.width {
            self.place_rows();
        } else {
            self.width = width;
            let chosen_key = self.chosen_key();
            self.rebuild_rows(chosen_key);
        }
    }

    /// Picks the first row to draw at the last height, as `place` says.
    /// A chosen item shows whole when it fits, else from its first line.
    fn place_rows(&mut self) {
        self.offset = self.offset.min(self.last_offset());
        // Before the first draw the height is 0 and no row fits, so the
        // start row stays put until a draw gives the real height.
        if self.follow_choice
            && self.height > 0
            && let Some(chosen) = self.chosen
        {
            let chosen_end = self.item_end(chosen);
            self.offset = self.offset.min(chosen);
            while self.offset < chosen && !self.rows_drawn_from(self.offset).contains(&chosen_end) {
                self.offset += 1;
            }
        }
        self.drawn = self.rows_drawn_from(self.offset);
    }

    /// Returns the rows drawn at the last `place`, each with true when
    /// it is chosen.
    pub(super) fn drawn_rows(&self) -> impl Iterator<Item = (Row<'_>, bool)> {
        self.drawn
            .iter()
            .map(|&index| (self.row(index), self.chosen == Some(self.item_start(index))))
    }

    /// Returns the row drawn on `line` of the list, or for a wrapped
    /// item its first row.
    pub(super) fn row_on_line(&self, line: usize) -> Option<usize> {
        self.drawn.get(line).map(|&index| self.item_start(index))
    }

    pub(super) fn chosen(&self) -> Option<usize> {
        self.chosen
    }

    /// Returns the chosen row, or `None` when the list is empty.
    pub(super) fn chosen_row(&self) -> Option<Row<'_>> {
        self.chosen.map(|index| self.row(index))
    }

    pub(super) fn len(&self) -> usize {
        self.rows.len()
    }

    pub(super) fn sections(&self) -> &[Section] {
        &self.sections
    }

    /// Returns the index in `sections` of the chosen row's section.
    pub(super) fn chosen_section(&self) -> Option<usize> {
        self.chosen.map(|index| match self.rows[index] {
            RowRef::Heading(section) | RowRef::Item(section, ..) => section,
        })
    }

    /// Opens the section `id` and chooses its heading at the top of the
    /// list, so its items show below it.
    pub(super) fn go_to_section(&mut self, id: SectionId) {
        let chosen_key = self.chosen_key();
        self.folds.insert(id, false);
        self.rebuild_rows(chosen_key);
        let heading = self.rows.iter().position(
            |row| matches!(row, RowRef::Heading(section) if self.sections[*section].id == id),
        );
        if let Some(index) = heading {
            self.choose_heading_on_top(index);
        }
    }

    /// Chooses the row at `index`, or the item it is a line of, and
    /// brings it on screen.
    pub(super) fn choose(&mut self, index: usize) {
        if index < self.rows.len() {
            self.chosen = Some(self.item_start(index));
            self.follow_choice = true;
        }
    }

    /// Moves the choice `steps` rows down, or up when negative, stopping
    /// at the first and last row. A step down always reaches the next
    /// item, even when the chosen item wraps over more rows.
    pub(super) fn move_choice(&mut self, steps: isize) {
        let Some(last) = self.rows.len().checked_sub(1) else {
            return;
        };
        let current = self.chosen.unwrap_or(0);
        let mut target = self.item_start(current.saturating_add_signed(steps).min(last));
        if steps > 0 && target <= current {
            target = (current + 1..self.rows.len())
                .find(|&index| self.item_start(index) == index)
                .unwrap_or(current);
        }
        self.choose(target);
    }

    /// Returns how many rows a page key or tap moves, one less than the
    /// list's height since the first line may hold a pinned heading.
    pub(super) fn page_size(&self) -> isize {
        self.height.saturating_sub(1).max(1).cast_signed()
    }

    /// Chooses the first heading below the choice and puts it on top.
    pub(super) fn choose_next_heading(&mut self) {
        let current = self.chosen.unwrap_or(0);
        if let Some(index) = (current + 1..self.rows.len()).find(|&index| self.is_heading(index)) {
            self.choose_heading_on_top(index);
        }
    }

    /// Chooses the first heading above the choice and puts it on top.
    pub(super) fn choose_previous_heading(&mut self) {
        let current = self.chosen.unwrap_or(0);
        if let Some(index) = (0..current).rev().find(|&index| self.is_heading(index)) {
            self.choose_heading_on_top(index);
        }
    }

    /// Scrolls the list without moving the choice, so one flick of the
    /// wheel does not race the choice down the list.
    pub(super) fn scroll_by(&mut self, steps: isize) {
        self.offset = self
            .offset
            .saturating_add_signed(steps)
            .min(self.last_offset());
        self.follow_choice = false;
    }

    /// Folds or opens the chosen heading, and puts an opened heading on
    /// top so its rows show below it. Does nothing when an item is
    /// chosen.
    pub(super) fn toggle_chosen_fold(&mut self) {
        if let Some(Row::Heading { section, folded }) = self.chosen_row() {
            let id = section.id;
            self.folds.insert(id, !folded);
            let chosen_key = self.chosen_key();
            self.rebuild_rows(chosen_key);
            if folded && let Some(index) = self.chosen {
                self.choose_heading_on_top(index);
            }
        }
    }

    /// Lists the rows again from the sections and folds. The row with
    /// `chosen_key` stays chosen. When it is gone, the choice keeps its
    /// place, or goes to the first item on a first read. Callers take
    /// `chosen_key` before they change the sections, since `rows` still
    /// points into the old ones until this runs.
    fn rebuild_rows(&mut self, chosen_key: Option<String>) {
        let had_choice = self.chosen;
        self.rows.clear();
        self.texts.clear();
        let text_width = self.width.saturating_sub(ITEM_INDENT);
        for (section_index, section) in self.sections.iter().enumerate() {
            self.rows.push(RowRef::Heading(section_index));
            self.texts.push(String::new());
            if self.is_folded(section.id) {
                continue;
            }
            for (item_index, item) in section.items.iter().enumerate() {
                for (line, text) in wrap_text(&item.text, text_width).into_iter().enumerate() {
                    self.rows
                        .push(RowRef::Item(section_index, item_index, line));
                    self.texts.push(text);
                }
            }
        }
        self.chosen = self.rows.len().checked_sub(1).map(|last| {
            let same_row = chosen_key
                .and_then(|key| (0..self.rows.len()).find(|&index| self.row_key(index) == key));
            let first_item = || {
                self.rows
                    .iter()
                    .position(|row| matches!(row, RowRef::Item(..)))
                    .unwrap_or(0)
            };
            same_row
                .or(had_choice.map(|index| self.item_start(index.min(last))))
                .unwrap_or_else(first_item)
        });
        // A tap can arrive before the next draw, so the rows a tap finds
        // must match the new list now.
        self.place_rows();
    }

    /// Chooses the heading at `index` and starts the list there, so the
    /// section's rows show below it. Near the end, `place` starts the
    /// list higher so no lines are left empty.
    fn choose_heading_on_top(&mut self, index: usize) {
        self.chosen = Some(index);
        self.offset = index;
        self.follow_choice = true;
        self.place_rows();
    }

    /// Returns the first row of the item that row `index` is a line of,
    /// or `index` itself for a heading.
    fn item_start(&self, index: usize) -> usize {
        match self.rows[index] {
            RowRef::Item(_, _, line) => index - line,
            RowRef::Heading(_) => index,
        }
    }

    /// Returns the last row of the item that starts at row `start`, or
    /// `start` itself for a heading.
    fn item_end(&self, start: usize) -> usize {
        (start + 1..self.rows.len())
            .take_while(|&index| self.item_start(index) == start)
            .last()
            .unwrap_or(start)
    }

    fn is_heading(&self, index: usize) -> bool {
        matches!(self.rows[index], RowRef::Heading(_))
    }

    /// Returns true when the section shows only its heading.
    fn is_folded(&self, id: SectionId) -> bool {
        self.folds.get(&id).copied().unwrap_or(id.starts_folded())
    }

    /// Returns the row at `index`, borrowed from its section.
    fn row(&self, index: usize) -> Row<'_> {
        match self.rows[index] {
            RowRef::Heading(section) => {
                let section = &self.sections[section];
                Row::Heading {
                    section,
                    folded: self.is_folded(section.id),
                }
            }
            RowRef::Item(section, item, line) => {
                let section = &self.sections[section];
                Row::Item {
                    section,
                    item: &section.items[item],
                    line: &self.texts[index],
                    first: line == 0,
                }
            }
        }
    }

    /// Returns the key of the row at `index`, the same across reads.
    fn row_key(&self, index: usize) -> String {
        match self.row(index) {
            Row::Heading { section, .. } => format!("section {:?}", section.id),
            Row::Item { item, .. } => item.key.clone(),
        }
    }

    /// Returns the chosen row's key, or `None` when nothing is chosen.
    fn chosen_key(&self) -> Option<String> {
        self.chosen
            .filter(|&index| index < self.rows.len())
            .map(|index| self.row_key(index))
    }

    /// Returns how far the list may scroll, the lowest start row that
    /// still draws the last row, so scrolling stops with no empty lines
    /// below. The pinned heading takes a line, so this is not always the
    /// row count less the height.
    fn last_offset(&self) -> usize {
        let Some(last) = self.rows.len().checked_sub(1) else {
            return 0;
        };
        (0..last)
            .find(|&offset| self.rows_drawn_from(offset).contains(&last))
            .unwrap_or(last)
    }

    /// Returns the rows drawn when the list starts at `offset`. When
    /// that row is an item, its section's heading is drawn first.
    fn rows_drawn_from(&self, offset: usize) -> Vec<usize> {
        let mut drawn = Vec::new();
        if matches!(self.rows.get(offset), Some(RowRef::Item(..))) {
            let heading = (0..offset)
                .rev()
                .find(|&index| matches!(self.rows[index], RowRef::Heading(_)));
            drawn.extend(heading);
        }
        let room = self.height.saturating_sub(drawn.len());
        drawn.extend((offset..self.rows.len()).take(room));
        drawn.truncate(self.height);
        drawn
    }
}

/// Splits `text` into lines of at most `width` columns, at spaces and at
/// each newline. A word wider than `width` is cut. A `width` of 0, as
/// before the first draw, keeps each paragraph on one line.
fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let mut line = String::new();
        // Splitting at single spaces keeps double spaces inside a line.
        for word in paragraph.split(' ') {
            let fits = width == 0 || line.chars().count() + 1 + word.chars().count() <= width;
            if !line.is_empty() && !fits {
                lines.push(std::mem::take(&mut line));
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
            while width > 0 && line.chars().count() > width {
                let cut = line
                    .char_indices()
                    .nth(width)
                    .map_or(line.len(), |(at, _)| at);
                let rest = line.split_off(cut);
                lines.push(std::mem::replace(&mut line, rest));
            }
        }
        lines.push(line);
    }
    lines
}
