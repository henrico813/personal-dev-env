//! This module loads and adds the user's decisions on comments, and compares
//! each comment with them. Read the `load(dir)`, `add(dir, new_approvals)`,
//! and `Approvals::compare(comment)` functions to understand the module.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::Display;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::comments::Comment;
use super::git;
use super::{Error, ErrorCode};

const APPROVALS_FILE: &str = "code-comments.json";
const LOCK_FILE: &str = "code-comments.json.lock";
const TEMP_FILE: &str = "code-comments.json.tmp";

/// Loads the decisions, or none when the file is missing. Refuses a field
/// it does not know, so an older program cannot drop it when it adds decisions.
pub fn load(dir: &Path) -> Result<Approvals, Error> {
    load_file(&find_approvals_folder(dir)?.join(APPROVALS_FILE))
}

/// Loads the decisions from one approvals file, as `load` does.
pub(crate) fn load_file(path: &Path) -> Result<Approvals, Error> {
    Ok(read_saved_file(path)?.into_approvals())
}

/// Adds decisions after the stored ones and keeps the rest of the file.
/// Another add waits for the lock, so neither loses its decisions.
pub fn add(dir: &Path, new_approvals: &[Approval]) -> Result<(), Error> {
    let folder = find_approvals_folder(dir)?;
    let path = folder.join(APPROVALS_FILE);
    // `_lock` holds the lock until this function returns, after the write.
    let _lock = lock_approvals(&folder).map_err(|error| save_error(&path, error))?;
    let mut saved = read_saved_file(&path)?;
    for approval in new_approvals {
        saved.approved.push(SavedApproval {
            approval: approval.clone(),
            unknown_fields: BTreeMap::new(),
        });
    }
    write_saved_file(&folder, &saved).map_err(|error| save_error(&path, error))
}

impl Approvals {
    /// Checks the comment against the decisions for its file and name. A
    /// comment with no name is always undecided.
    pub fn compare(&self, comment: &Comment) -> DecisionMatch {
        let Some(name) = &comment.name else {
            return DecisionMatch::Undecided;
        };
        let decisions: Vec<&Approval> = self
            .approved
            .iter()
            .filter(|decision| decision.file == comment.file && &decision.name == name)
            .collect();
        match decisions.last() {
            None => DecisionMatch::Undecided,
            Some(newest) if newest.text.is_empty() => DecisionMatch::WasCut,
            Some(_)
                if decisions
                    .iter()
                    .any(|decision| decision.text == comment.text) =>
            {
                DecisionMatch::Approved
            }
            Some(_) => DecisionMatch::DiffersFromApproved,
        }
    }

    /// Returns each word agents should not use, with what to say instead.
    pub(crate) fn avoided_words(&self) -> &BTreeMap<String, String> {
        &self.avoid
    }

    /// Returns each file that holds a decision, approved or cut.
    pub(crate) fn files(&self) -> BTreeSet<&Path> {
        self.approved
            .iter()
            .map(|decision| decision.file.as_path())
            .collect()
    }
}

/// Holds one decision. An empty text means the user cut the comment.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct Approval {
    pub file: PathBuf,
    pub name: String,
    pub text: String,
}

/// Holds every decision in the approvals file, oldest first.
pub struct Approvals {
    approved: Vec<Approval>,
    avoid: BTreeMap<String, String>,
}

/// Says how one comment stands against the user's decisions.
#[derive(Debug, PartialEq)]
pub enum DecisionMatch {
    /// No decision has the comment's file and name.
    Undecided,
    /// An approved text equals the comment's text, and the newest decision
    /// is not a cut.
    Approved,
    /// The newest decision for the comment's file and name is a cut.
    WasCut,
    /// The comment's name has approved texts, and none equals its text.
    DiffersFromApproved,
}

/// Holds the approvals file in the format today's checker writes.
#[derive(Default, Deserialize, Serialize)]
struct SavedFile {
    #[serde(default)]
    approved: Vec<SavedApproval>,
    #[serde(default)]
    avoid: BTreeMap<String, String>,
    /// Collects fields this program does not know, so reading can refuse
    /// them.
    #[serde(flatten)]
    unknown_fields: BTreeMap<String, Value>,
}

/// Holds one stored decision and any fields this program does not know.
#[derive(Deserialize, Serialize)]
struct SavedApproval {
    #[serde(flatten)]
    approval: Approval,
    #[serde(flatten)]
    unknown_fields: BTreeMap<String, Value>,
}

impl SavedFile {
    /// Keeps the decisions and avoided words, as the steps read them.
    fn into_approvals(self) -> Approvals {
        Approvals {
            approved: self
                .approved
                .into_iter()
                .map(|entry| entry.approval)
                .collect(),
            avoid: self.avoid,
        }
    }
}

/// Returns the folder that holds the approvals, in the Git folder every
/// worktree of the clone shares.
fn find_approvals_folder(dir: &Path) -> Result<PathBuf, Error> {
    Ok(git::find_shared_git_folder(dir)?.join("pde"))
}

/// Locks a file beside the approvals, since each add replaces the
/// approvals file and a lock on the old one guards nothing.
fn lock_approvals(folder: &Path) -> io::Result<File> {
    fs::create_dir_all(folder)?;
    let lock_file = File::create(folder.join(LOCK_FILE))?;
    lock_file.lock()?;
    Ok(lock_file)
}

/// Reads the saved file, or an empty one when it is missing.
fn read_saved_file(path: &Path) -> Result<SavedFile, Error> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(SavedFile::default()),
        Err(error) => return Err(read_error(path, error)),
    };
    let saved: SavedFile =
        serde_json::from_slice(&bytes).map_err(|error| read_error(path, error))?;
    let unknown_field = saved
        .unknown_fields
        .keys()
        .chain(
            saved
                .approved
                .iter()
                .flat_map(|entry| entry.unknown_fields.keys()),
        )
        .next();
    if let Some(field) = unknown_field {
        return Err(Error {
            code: ErrorCode::UnknownSavedField,
            message: format!(
                "{} holds the field `{field}`, which this version does not know. Run pde-installer to update it.",
                path.display()
            ),
        });
    }
    Ok(saved)
}

/// Replaces the approvals file with a synced temporary file, so a crash
/// or power cut leaves a whole file, old or new.
fn write_saved_file(folder: &Path, saved: &SavedFile) -> io::Result<()> {
    let mut text = serde_json::to_vec_pretty(saved)?;
    text.push(b'\n');
    let temp_path = folder.join(TEMP_FILE);
    let mut temp_file = File::create(&temp_path)?;
    temp_file.write_all(&text)?;
    temp_file.sync_all()?;
    fs::rename(&temp_path, folder.join(APPROVALS_FILE))
}

fn read_error(path: &Path, error: impl Display) -> Error {
    Error {
        code: ErrorCode::SavedFileUnreadable,
        message: format!(
            "Cannot read the approvals in {}. Tell the user to fix the file. {error}",
            path.display()
        ),
    }
}

fn save_error(path: &Path, error: io::Error) -> Error {
    Error {
        code: ErrorCode::SaveFailed,
        message: format!("Cannot save the approvals in {}. {error}", path.display()),
    }
}

// The rest of the file, up to the tests, serves only `move-old-approvals`,
// which moves the old checker's approvals in.
const BACKUP_FILE: &str = "code-comments.json.before-move";

/// Puts the old checker's decisions ahead of the stored ones, each once, adds
/// its avoided words, and returns the decisions as saved. Saves a copy of the
/// file it replaces first, so the user can undo the move.
pub(crate) fn add_old_approvals(
    dir: &Path,
    old_copies: &[Approvals],
) -> Result<(OldApprovalsAdded, Approvals), Error> {
    let folder = find_approvals_folder(dir)?;
    let path = folder.join(APPROVALS_FILE);
    // `_lock` holds the lock until this function returns, after the write.
    let _lock = lock_approvals(&folder).map_err(|error| save_error(&path, error))?;
    let mut saved = read_saved_file(&path)?;
    let mut added = merge_old_approvals(&mut saved, old_copies);
    if added.entries_moved > 0 || added.avoided_words_moved > 0 {
        added.backup = back_up_approvals_file(&folder).map_err(|error| save_error(&path, error))?;
        write_saved_file(&folder, &saved).map_err(|error| save_error(&path, error))?;
    }
    Ok((added, saved.into_approvals()))
}

impl Approvals {
    /// Counts the decisions, approved or cut.
    pub(crate) fn count_entries(&self) -> usize {
        self.approved.len()
    }
}

/// Counts what `add_old_approvals` found and moved, so the user can compare
/// counts before and after the move.
#[derive(Debug, Default, Serialize)]
pub struct OldApprovalsAdded {
    /// Counts the entries in the approvals file before the move.
    pub entries_before: usize,
    /// Counts the old entries now ahead of the stored ones.
    pub entries_moved: usize,
    /// Counts old entries the approvals file already held.
    pub entries_already_there: usize,
    /// Counts old entries that another old file held too.
    pub entries_repeated: usize,
    /// Counts old entries that a later entry for the same comment in the same
    /// file replaced. The old checker reads only that later entry too.
    pub entries_replaced_in_own_file: usize,
    pub entries_after: usize,
    pub avoided_words_moved: usize,
    /// Counts old avoided words the approvals file already held with the same
    /// advice.
    pub avoided_words_already_there: usize,
    /// Counts old avoided words whose advice differs from the advice kept.
    pub avoided_words_differing: usize,
    /// Lists each moved cut that went behind a decision the approvals file
    /// already held for its comment, so the cut does not count. The user can
    /// cut those comments again.
    pub cuts_behind_newer_decisions: Vec<CutComment>,
    pub backup: ApprovalsBackup,
}

/// Names the comment of one moved cut by its file and name.
#[derive(Debug, Serialize)]
pub struct CutComment {
    pub file: PathBuf,
    pub name: String,
}

/// Says what undoes the move's change to the approvals file.
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalsBackup {
    /// The move wrote nothing, so the undo leaves the file alone.
    #[default]
    NothingWritten,
    /// The move made the file, so the undo deletes it.
    NoFileBefore,
    /// The move copied the file here before writing, so the undo copies it
    /// back.
    SavedAt(PathBuf),
}

/// Puts each old decision the saved file lacks ahead of the saved ones, once,
/// and adds each avoided word the saved file lacks.
fn merge_old_approvals(saved: &mut SavedFile, old_copies: &[Approvals]) -> OldApprovalsAdded {
    let mut added = OldApprovalsAdded {
        entries_before: saved.approved.len(),
        ..OldApprovalsAdded::default()
    };
    let stored: HashSet<&Approval> = saved.approved.iter().map(|entry| &entry.approval).collect();
    let stored_comments: HashSet<(&PathBuf, &String)> = saved
        .approved
        .iter()
        .map(|entry| (&entry.approval.file, &entry.approval.name))
        .collect();
    let mut chosen = HashSet::new();
    let mut moved = Vec::new();
    for copy in old_copies {
        // The old checker reads each file as a map, so a comment's last entry
        // replaces its earlier ones.
        let mut last_index = HashMap::new();
        for (index, approval) in copy.approved.iter().enumerate() {
            last_index.insert((&approval.file, &approval.name), index);
        }
        for (index, approval) in copy.approved.iter().enumerate() {
            if last_index[&(&approval.file, &approval.name)] != index {
                added.entries_replaced_in_own_file += 1;
            } else if stored.contains(approval) {
                added.entries_already_there += 1;
            } else if !chosen.insert(approval) {
                added.entries_repeated += 1;
            } else {
                moved.push(approval.clone());
            }
        }
    }
    // A decision the file already holds stays newer than a moved cut of its
    // comment, so the reply names that cut.
    added.cuts_behind_newer_decisions = moved
        .iter()
        .filter(|approval| approval.text.is_empty())
        .filter(|cut| stored_comments.contains(&(&cut.file, &cut.name)))
        .map(|cut| CutComment {
            file: cut.file.clone(),
            name: cut.name.clone(),
        })
        .collect();
    // The old files hold no time, so a cut goes after the approvals of its name.
    // The user is then asked about the comment, rather than losing the cut.
    moved.sort_by_key(|approval| approval.text.is_empty());
    added.entries_moved = moved.len();
    let moved = moved.into_iter().map(|approval| SavedApproval {
        approval,
        unknown_fields: BTreeMap::new(),
    });
    saved.approved.splice(0..0, moved);
    added.entries_after = saved.approved.len();
    for (word, advice) in old_copies.iter().flat_map(|copy| &copy.avoid) {
        if let Some(kept) = saved.avoid.get(word) {
            if kept == advice {
                added.avoided_words_already_there += 1;
            } else {
                added.avoided_words_differing += 1;
            }
        } else {
            saved.avoid.insert(word.clone(), advice.clone());
            added.avoided_words_moved += 1;
        }
    }
    added
}

/// Copies the approvals file to the first free `code-comments.json.before-move`
/// name, such as `code-comments.json.before-move.2`, and syncs it.
fn back_up_approvals_file(folder: &Path) -> io::Result<ApprovalsBackup> {
    let contents = match fs::read(folder.join(APPROVALS_FILE)) {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(ApprovalsBackup::NoFileBefore);
        }
        Err(error) => return Err(error),
    };
    let mut number = 1;
    loop {
        let name = match number {
            1 => BACKUP_FILE.to_owned(),
            _ => format!("{BACKUP_FILE}.{number}"),
        };
        let path = folder.join(name);
        match File::create_new(&path) {
            Ok(mut backup) => {
                backup.write_all(&contents)?;
                backup.sync_all()?;
                return Ok(ApprovalsBackup::SavedAt(path));
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => number += 1,
            Err(error) => return Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::comments::CommentStyle;

    /// Builds one decision the user made, with an empty text for a cut.
    fn approval(file: &str, name: &str, text: &str) -> Approval {
        Approval {
            file: PathBuf::from(file),
            name: name.to_owned(),
            text: text.to_owned(),
        }
    }

    /// Builds a comment as the code holds it now.
    fn comment(file: &str, name: &str, text: &str) -> Comment {
        Comment {
            file: PathBuf::from(file),
            name: Some(name.to_owned()),
            line: 1,
            end_line: 1,
            style: CommentStyle::Line,
            text: text.to_owned(),
            byte_range: 0..text.len(),
            can_cut: true,
        }
    }

    /// `Approvals::compare` must pass every text the user approved for a comment and fail any other text.
    /// Otherwise an agent could quietly rewrite what the user approved.
    #[test]
    fn approved_texts_pass_and_other_texts_fail() {
        // The user approved one text, then a second one later.
        let approvals = Approvals {
            approved: vec![
                approval("review_store.go", "SaveAnswer", "// Saves the answer.\n"),
                approval(
                    "review_store.go",
                    "SaveAnswer",
                    "// Saves the answer safely.\n",
                ),
            ],
            avoid: BTreeMap::new(),
        };

        // Either approved text in the code passes.
        let older = comment("review_store.go", "SaveAnswer", "// Saves the answer.\n");
        let newer = comment(
            "review_store.go",
            "SaveAnswer",
            "// Saves the answer safely.\n",
        );
        assert_eq!(approvals.compare(&older), DecisionMatch::Approved);
        assert_eq!(approvals.compare(&newer), DecisionMatch::Approved);

        // A text the user never approved is caught.
        let reworded = comment("review_store.go", "SaveAnswer", "// Writes the answer.\n");
        assert_eq!(
            approvals.compare(&reworded),
            DecisionMatch::DiffersFromApproved
        );
    }

    /// `Approvals::compare` must count only the newest decision for a comment.
    /// Otherwise a comment the user cut would still pass, or one they approved again would still count as cut.
    #[test]
    fn the_newest_decision_wins() {
        let approved = approval("review_store.go", "SaveAnswer", "// Saves the answer.\n");
        let cut = approval("review_store.go", "SaveAnswer", "");
        let cases = [
            (
                "cut-after-approval",
                vec![approved.clone(), cut.clone()],
                DecisionMatch::WasCut,
            ),
            (
                "approval-after-cut",
                vec![cut, approved],
                DecisionMatch::Approved,
            ),
        ];

        for (case, decisions, expected) in cases {
            let approvals = Approvals {
                approved: decisions,
                avoid: BTreeMap::new(),
            };
            let in_code = comment("review_store.go", "SaveAnswer", "// Saves the answer.\n");
            assert_eq!(approvals.compare(&in_code), expected, "{case}");
        }
    }

    /// `Approvals::compare` must count a decision only in its own file.
    /// Otherwise approving one file's module doc would fail the module docs of every other file.
    #[test]
    fn a_decision_counts_only_in_its_own_file() {
        let approvals = Approvals {
            approved: vec![approval("review.rs", "<module>", "//! Reads the review.\n")],
            avoid: BTreeMap::new(),
        };

        let other_file = comment("approvals.rs", "<module>", "//! Reads the approvals.\n");
        assert_eq!(approvals.compare(&other_file), DecisionMatch::Undecided);
    }

    /// Builds one old checker's file with these decisions and no avoided words.
    fn old_copy(decisions: Vec<Approval>) -> Approvals {
        Approvals {
            approved: decisions,
            avoid: BTreeMap::new(),
        }
    }

    /// Builds a saved approvals file with these decisions, as this program keeps it.
    fn saved_file(decisions: Vec<Approval>) -> SavedFile {
        SavedFile {
            approved: decisions
                .into_iter()
                .map(|approval| SavedApproval {
                    approval,
                    unknown_fields: BTreeMap::new(),
                })
                .collect(),
            ..SavedFile::default()
        }
    }

    /// `merge_old_approvals` must put old decisions before the answers this program already saved.
    /// Otherwise an old approval would undo a cut the user made in the new program.
    #[test]
    fn merge_old_approvals_keeps_newer_answers_newest() {
        let mut saved = saved_file(vec![approval("review_store.go", "SaveAnswer", "")]);
        let old = old_copy(vec![approval(
            "review_store.go",
            "SaveAnswer",
            "// Saves the answer.\n",
        )]);

        merge_old_approvals(&mut saved, &[old]);

        let in_code = comment("review_store.go", "SaveAnswer", "// Saves the answer.\n");
        assert_eq!(
            saved.into_approvals().compare(&in_code),
            DecisionMatch::WasCut
        );
    }

    /// `merge_old_approvals` must put an old cut after old approvals of its name, whichever file held it.
    /// Otherwise a cut made in one worktree would quietly stop counting, and the comment could come back unasked.
    #[test]
    fn merge_old_approvals_puts_cuts_after_approvals() {
        let approved = approval("review_store.go", "SaveAnswer", "// Saves the answer.\n");
        let cut = approval("review_store.go", "SaveAnswer", "");
        let cases = [
            ("cut-in-main", vec![cut.clone()], vec![approved.clone()]),
            ("cut-in-worktree", vec![approved], vec![cut]),
        ];

        for (case, main_copy, worktree_copy) in cases {
            let mut saved = SavedFile::default();
            merge_old_approvals(&mut saved, &[old_copy(main_copy), old_copy(worktree_copy)]);

            let in_code = comment("review_store.go", "SaveAnswer", "// Saves the answer.\n");
            let decision = saved.into_approvals().compare(&in_code);
            assert_eq!(decision, DecisionMatch::WasCut, "{case}");
        }
    }

    /// `merge_old_approvals` must keep only the last decision for a comment in each old file, as the old checker reads it.
    /// Otherwise a text the user replaced in the old checker would count as approved again.
    #[test]
    fn merge_old_approvals_ignores_replaced_old_texts() {
        let old = old_copy(vec![
            approval("review_store.go", "SaveAnswer", "// Saves the answer.\n"),
            approval(
                "review_store.go",
                "SaveAnswer",
                "// Saves the answer safely.\n",
            ),
        ]);
        let mut saved = SavedFile::default();

        merge_old_approvals(&mut saved, &[old]);

        let replaced = comment("review_store.go", "SaveAnswer", "// Saves the answer.\n");
        assert_eq!(
            saved.into_approvals().compare(&replaced),
            DecisionMatch::DiffersFromApproved
        );
    }

    /// `merge_old_approvals` must list an old cut that goes behind a decision the approvals file held for its comment.
    /// Otherwise that cut would stop counting without the user knowing, and the comment could come back unasked.
    #[test]
    fn merge_old_approvals_lists_cuts_behind_decisions() {
        let mut saved = saved_file(vec![approval(
            "review_store.go",
            "SaveAnswer",
            "// Saves the answer.\n",
        )]);
        let old = old_copy(vec![approval("review_store.go", "SaveAnswer", "")]);

        let added = merge_old_approvals(&mut saved, &[old]);

        let names: Vec<&str> = added
            .cuts_behind_newer_decisions
            .iter()
            .map(|cut| cut.name.as_str())
            .collect();
        assert_eq!(names, ["SaveAnswer"]);
    }

    /// `merge_old_approvals` must add the old files' avoided words, keep the saved ones, and count each old word as moved, already there or differing.
    /// Otherwise agents would start using words the user asked them to avoid, or the user could not check by counting that none was lost.
    #[test]
    fn merge_old_approvals_keeps_avoided_words_counted() {
        let mut saved = SavedFile::default();
        saved
            .avoid
            .insert("utilize".to_owned(), "say use".to_owned());
        saved
            .avoid
            .insert("leverage".to_owned(), "say use".to_owned());
        let mut old = old_copy(Vec::new());
        old.avoid
            .insert("client".to_owned(), "say customer".to_owned());
        old.avoid.insert("utilize".to_owned(), "say use".to_owned());
        old.avoid
            .insert("leverage".to_owned(), "say rely on".to_owned());

        let added = merge_old_approvals(&mut saved, &[old]);

        let words: Vec<&str> = saved.avoid.keys().map(String::as_str).collect();
        assert_eq!(words, ["client", "leverage", "utilize"]);
        assert_eq!(saved.avoid["leverage"], "say use");
        assert_eq!(added.avoided_words_moved, 1);
        assert_eq!(added.avoided_words_already_there, 1);
        assert_eq!(added.avoided_words_differing, 1);
    }
}
