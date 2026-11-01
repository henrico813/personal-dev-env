//! Reads and adds the user's answers in one approvals file that every
//! worktree of the clone shares. Answers are only added, so the last one
//! for a file and name is the newest.

use std::collections::BTreeMap;
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

/// Holds one answer. An empty text means the user cut the comment.
#[derive(Clone, Deserialize, Serialize)]
pub struct Approval {
    pub file: PathBuf,
    pub name: String,
    pub text: String,
}

/// Holds every answer in the approvals file, oldest first.
pub struct Approvals {
    approved: Vec<Approval>,
}

/// Says how one comment stands against the user's answers.
#[derive(Debug, PartialEq)]
pub enum DecisionMatch {
    /// No answer has the comment's file and name.
    Undecided,
    /// An approved text equals the comment's text, and the newest answer
    /// is not a cut.
    Approved,
    /// The newest answer for the comment's file and name is a cut.
    WasCut,
    /// The comment's name has approved texts, and none equals its text.
    DiffersFromApproved,
}

impl Approvals {
    /// Checks the comment against the answers for its file and name. A
    /// comment with no name is always undecided.
    pub fn compare(&self, comment: &Comment) -> DecisionMatch {
        let Some(name) = &comment.name else {
            return DecisionMatch::Undecided;
        };
        let answers: Vec<&Approval> = self
            .approved
            .iter()
            .filter(|answer| answer.file == comment.file && &answer.name == name)
            .collect();
        match answers.last() {
            None => DecisionMatch::Undecided,
            Some(newest) if newest.text.is_empty() => DecisionMatch::WasCut,
            Some(_) if answers.iter().any(|answer| answer.text == comment.text) => {
                DecisionMatch::Approved
            }
            Some(_) => DecisionMatch::DiffersFromApproved,
        }
    }
}

/// Holds the approvals file in the format today's checker writes.
#[derive(Default, Deserialize, Serialize)]
struct SavedFile {
    #[serde(default)]
    approved: Vec<SavedApproval>,
    /// Maps each word agents should not use to what to say instead.
    #[serde(default)]
    avoid: BTreeMap<String, String>,
    /// Collects fields this program does not know, so reading can refuse
    /// them.
    #[serde(flatten)]
    unknown_fields: BTreeMap<String, Value>,
}

/// Holds one saved answer and any fields this program does not know.
#[derive(Deserialize, Serialize)]
struct SavedApproval {
    #[serde(flatten)]
    approval: Approval,
    #[serde(flatten)]
    unknown_fields: BTreeMap<String, Value>,
}

/// Loads the answers, or none when the file is missing. Refuses a field
/// it does not know, so an older program cannot drop it on save.
pub fn load(dir: &Path) -> Result<Approvals, Error> {
    let path = find_approvals_folder(dir)?.join(APPROVALS_FILE);
    let saved = read_saved_file(&path)?;
    let approved = saved
        .approved
        .into_iter()
        .map(|entry| entry.approval)
        .collect();
    Ok(Approvals { approved })
}

/// Adds answers after the saved ones and keeps the rest of the file.
/// Another add waits for the lock, so neither loses its answers.
pub fn add(dir: &Path, new_approvals: &[Approval]) -> Result<(), Error> {
    let folder = find_approvals_folder(dir)?;
    let path = folder.join(APPROVALS_FILE);
    // `_lock` holds the lock until this function returns, after the save.
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

/// Returns the folder that holds the approvals, in the Git folder every
/// worktree of the clone shares.
fn find_approvals_folder(dir: &Path) -> Result<PathBuf, Error> {
    Ok(git::find_shared_git_folder(dir)?.join("pde"))
}

/// Locks a file beside the approvals, since each save replaces the
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
        message: format!("Cannot read the approvals in {}. {error}", path.display()),
    }
}

fn save_error(path: &Path, error: io::Error) -> Error {
    Error {
        code: ErrorCode::SaveFailed,
        message: format!("Cannot save the approvals in {}. {error}", path.display()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::comments::CommentStyle;

    fn approval(file: &str, name: &str, text: &str) -> Approval {
        Approval {
            file: PathBuf::from(file),
            name: name.to_owned(),
            text: text.to_owned(),
        }
    }

    fn comment(file: &str, name: &str, text: &str) -> Comment {
        Comment {
            file: PathBuf::from(file),
            name: Some(name.to_owned()),
            line: 1,
            end_line: 1,
            style: CommentStyle::Line,
            text: text.to_owned(),
        }
    }

    /// Without it, the check would fail text the user approved earlier, or pass an agent's reword.
    #[test]
    fn approved_texts_match_and_other_texts_differ() {
        let approvals = Approvals {
            approved: vec![
                approval("review_store.go", "SaveAnswer", "// Saves the answer.\n"),
                approval(
                    "review_store.go",
                    "SaveAnswer",
                    "// Saves the answer safely.\n",
                ),
            ],
        };

        let older = comment("review_store.go", "SaveAnswer", "// Saves the answer.\n");
        let newer = comment(
            "review_store.go",
            "SaveAnswer",
            "// Saves the answer safely.\n",
        );
        let reworded = comment("review_store.go", "SaveAnswer", "// Writes the answer.\n");
        assert_eq!(approvals.compare(&older), DecisionMatch::Approved);
        assert_eq!(approvals.compare(&newer), DecisionMatch::Approved);
        assert_eq!(
            approvals.compare(&reworded),
            DecisionMatch::DiffersFromApproved
        );
    }

    /// Without it, the user's latest keep or cut would be ignored, and they would be asked again.
    #[test]
    fn newest_cut_or_approval_settles_the_name() {
        let kept = approval("review_store.go", "SaveAnswer", "// Saves the answer.\n");
        let cut = approval("review_store.go", "SaveAnswer", "");
        let cases = [
            (
                "cut-after-approval",
                vec![kept.clone(), cut.clone()],
                DecisionMatch::WasCut,
            ),
            (
                "approval-after-cut",
                vec![cut, kept],
                DecisionMatch::Approved,
            ),
        ];

        for (case, approved, expected) in cases {
            let approvals = Approvals { approved };
            let saved = comment("review_store.go", "SaveAnswer", "// Saves the answer.\n");
            assert_eq!(approvals.compare(&saved), expected, "{case}");
        }
    }

    /// Without it, approving one file's module comment would fail other files' module comments.
    #[test]
    fn same_name_in_another_file_is_undecided() {
        let approvals = Approvals {
            approved: vec![approval("review.rs", "<module>", "//! Reads the review.\n")],
        };

        let other_file = comment("approvals.rs", "<module>", "//! Reads the approvals.\n");
        assert_eq!(approvals.compare(&other_file), DecisionMatch::Undecided);
    }
}
