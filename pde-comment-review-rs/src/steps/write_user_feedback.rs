//! This module runs step 7, which writes each Answered Request into its file
//! and saves a decision for each named comment. A stop signal waits until every
//! file is written or back and the Review Session is saved, so one after the
//! last file still ends with all User Feedback written. Read the
//! `write_user_feedback(dir)`,
//! `write_files_and_decisions(lock, root, backup_folder, files, decisions, signals)`,
//! and `write_every_file(backup, signals)` functions to understand the module.

use std::collections::BTreeMap;
use std::ffi::c_int;
use std::fmt::Display;
use std::fs::{self, File, Permissions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM, SIGXFSZ};
use signal_hook::{flag, low_level};

use crate::shared::approvals::{self, Approval};
use crate::shared::comments::{self, Comment, CommentEdit, RewrittenFile};
use crate::shared::git;
use crate::shared::review_session::{
    self, AnsweredRequest, FeedbackRequest, ReviewSessionLock, ReviewStatus,
};
use crate::shared::user_feedback::choose_new_text;
use crate::shared::{Error, ErrorCode};

/// Names the folder beside `review.json` that holds each file's original while
/// step 7 writes.
const BACKUP_FOLDER: &str = "write-backup";

const STOP_SIGNALS: [c_int; 3] = [SIGINT, SIGTERM, SIGHUP];

/// Writes every Answered Request into its file and saves the decisions, or puts
/// every file back and keeps the User Feedback. When a file cannot go back, it
/// refuses with `WriteNotFinished`; when only the save of `done` fails, it
/// returns `SaveFailed` with everything written.
pub fn write_user_feedback(dir: &Path) -> Result<WriteResult, Error> {
    let root = git::find_root(dir)?;
    let mut lock = review_session::lock(&root)?;
    let backup_folder = lock.folder().join(BACKUP_FOLDER);
    let Some(review_session) = lock.review_session_mut() else {
        return Err(nothing_to_write("No Review Session waits here."));
    };
    match review_session.status {
        ReviewStatus::Done => return Ok(WriteResult::AlreadyWritten),
        ReviewStatus::WritingUserFeedback => {
            return Err(Error {
                code: ErrorCode::WriteNotFinished,
                message: format!(
                    "An earlier write did not finish. Check git status, and any originals it saved in {}.",
                    backup_folder.display()
                ),
            });
        }
        // Waiting User Notes leave Feedback Requests unanswered, so the next check refuses the write.
        ReviewStatus::WaitingForUserFeedback | ReviewStatus::UserNotesWaiting => {}
    }
    let Some(answered) = review_session.answered_requests() else {
        return Err(nothing_to_write(
            "Some Feedback Requests have no answer yet.",
        ));
    };
    let files = rewrite_answered_requests(&root, &answered)?;
    let decisions = list_decisions(&answered);
    let signals = hold_stop_signals();
    let result =
        write_files_and_decisions(&mut lock, &root, backup_folder, files, &decisions, &signals);
    if let Some(signal) = signals.release() {
        // The signal waited for the files and the Review Session to settle, and now acts.
        deliver_held_signal(signal);
    }
    result
}

/// Writes the files and saves the decisions, or puts every file back.
fn write_files_and_decisions(
    lock: &mut ReviewSessionLock,
    root: &Path,
    backup_folder: PathBuf,
    files: Vec<RewrittenFile>,
    decisions: &[Approval],
    signals: &HeldSignals,
) -> Result<WriteResult, Error> {
    // A crash after this save leaves the status, so the screen tells the user to check git status.
    save_status(lock, ReviewStatus::WritingUserFeedback)?;
    let mut backup = match FileBackup::save(root, backup_folder, files) {
        Ok(backup) => backup,
        Err(error) => return refuse_after_write(lock, error),
    };
    if let Err(reason) = write_every_file(&mut backup, signals) {
        return refuse_after_write(lock, backup.undo(reason));
    }
    if let Err(error) = approvals::add(root, decisions) {
        return refuse_after_write(lock, backup.undo(error.message));
    }
    save_status(lock, ReviewStatus::Done).map_err(|error| Error {
        code: error.code,
        message: format!(
            "All User Feedback and every decision was written, but the Review Session could not be marked done. Do not put back the originals in {}. {}",
            backup.folder.display(),
            error.message
        ),
    })?;
    let files = backup
        .files
        .iter()
        .map(|file| file.rewritten.path.clone())
        .collect();
    backup.remove();
    Ok(WriteResult::Written {
        files,
        approved: decisions.len(),
    })
}

/// Writes each file in turn, and stops at a file that changed since its
/// rewrite, a failed write or a stop signal, returning why.
fn write_every_file(backup: &mut FileBackup, signals: &HeldSignals) -> Result<(), String> {
    for file in &backup.files {
        let shown_path = file.rewritten.path.display();
        match replace_file(
            &file.path,
            file.rewritten.old_contents.as_bytes(),
            file.rewritten.new_contents.as_bytes(),
            &file.permissions,
        ) {
            Ok(true) => {}
            // Writing over a change made since the rewrite read the file would lose it.
            Ok(false) => {
                return Err(format!(
                    "{shown_path} changed while the User Feedback was written."
                ));
            }
            Err(error) => return Err(format!("Cannot write {shown_path}. {error}")),
        }
        backup.written += 1;
        if signals.was_caught() {
            return Err("A stop signal arrived.".to_owned());
        }
    }
    Ok(())
}

/// Says what step 7 did.
#[derive(Debug, PartialEq)]
pub enum WriteResult {
    /// Lists the files written, from the worktree's top folder, and counts the
    /// decisions saved.
    Written {
        files: Vec<PathBuf>,
        approved: usize,
    },
    /// Says an earlier call, such as one from a second screen, wrote the User
    /// Feedback.
    AlreadyWritten,
}

/// Holds each file's original, also saved in the backup folder, and counts the
/// files the write has replaced so far.
struct FileBackup {
    folder: PathBuf,
    files: Vec<BackedUpFile>,
    written: usize,
}

/// Holds one rewritten file with its saved permissions and its path, with links
/// resolved once before any write. A later change to a link never moves the
/// write or the put-back.
struct BackedUpFile {
    path: PathBuf,
    permissions: Permissions,
    rewritten: RewrittenFile,
}

impl FileBackup {
    /// Saves each file's original contents and permissions in `folder`, before
    /// any file is written.
    fn save(root: &Path, folder: PathBuf, files: Vec<RewrittenFile>) -> Result<Self, Error> {
        // A folder already there may hold the only copy of an earlier write's originals.
        if let Err(error) = fs::create_dir(&folder) {
            return Err(write_failed(format!(
                "Cannot make {}. If an earlier write left it, check git status, then remove it. {error}",
                folder.display()
            )));
        }
        let mut backup = FileBackup {
            folder,
            files: Vec::new(),
            written: 0,
        };
        let resolved_root = fs::canonicalize(root);
        for rewritten in files {
            let saved = match &resolved_root {
                Ok(resolved_root) => backup.save_original(resolved_root, rewritten),
                Err(error) => Err(format!("Cannot find the worktree's folder. {error}")),
            };
            match saved {
                Ok(file) => backup.files.push(file),
                Err(reason) => {
                    backup.remove();
                    return Err(write_failed(reason));
                }
            }
        }
        Ok(backup)
    }

    fn save_original(
        &self,
        resolved_root: &Path,
        rewritten: RewrittenFile,
    ) -> Result<BackedUpFile, String> {
        let shown_path = rewritten.path.display().to_string();
        let cannot_save =
            |error: io::Error| format!("Cannot save the original of {shown_path}. {error}");
        let path = fs::canonicalize(resolved_root.join(&rewritten.path)).map_err(cannot_save)?;
        // A link to a file outside the worktree would let step 7 write there.
        if !path.starts_with(resolved_root) {
            return Err(format!(
                "{shown_path} leads outside the worktree, to {}.",
                path.display()
            ));
        }
        let permissions = fs::metadata(&path).map_err(cannot_save)?.permissions();
        let backup_path = self.folder.join(&rewritten.path);
        if let Some(parent) = backup_path.parent() {
            fs::create_dir_all(parent).map_err(cannot_save)?;
        }
        create_synced_file(
            &backup_path,
            rewritten.old_contents.as_bytes(),
            &permissions,
        )
        .map_err(cannot_save)?;
        Ok(BackedUpFile {
            path,
            permissions,
            rewritten,
        })
    }

    /// Puts back each written file that still holds exactly what the write put
    /// there, going on past any it cannot. Removes the backup folder only when
    /// every file went back.
    fn restore(self) -> Result<(), Error> {
        let mut left = Vec::new();
        for file in &self.files[..self.written] {
            let shown_path = file.rewritten.path.display();
            // Anything else in the file is a later edit, which a put-back would erase.
            match replace_file(
                &file.path,
                file.rewritten.new_contents.as_bytes(),
                file.rewritten.old_contents.as_bytes(),
                &file.permissions,
            ) {
                Ok(true) => {}
                Ok(false) => left.push(format!("{shown_path}, which changed after it was written")),
                Err(error) => left.push(format!("{shown_path}, since {error}")),
            }
        }
        if left.is_empty() {
            self.remove();
            return Ok(());
        }
        let originals = if left.len() == 1 {
            "its original stays"
        } else {
            "their originals stay"
        };
        Err(Error {
            code: ErrorCode::WriteNotFinished,
            message: format!(
                "Cannot put back {}, so {originals} in {}. Check git status.",
                left.join(", and "),
                self.folder.display()
            ),
        })
    }

    /// Puts every file back and returns `WriteFailed` with the reason, unless a
    /// file could not go back.
    fn undo(self, reason: impl Display) -> Error {
        match self.restore() {
            Ok(()) => write_failed(reason),
            Err(not_finished) => Error {
                code: not_finished.code,
                message: format!("{reason} {}", not_finished.message),
            },
        }
    }

    fn remove(self) {
        // A folder left behind makes the next write ask the user to check and remove it.
        let _ = fs::remove_dir_all(&self.folder);
    }
}

/// Makes Ctrl-C, SIGTERM and SIGHUP wait until released, so a stop puts every
/// file back instead of leaving the write half done.
struct HeldSignals {
    owner: &'static SignalOwner,
}

/// Owns this program's actions for the stop signals and SIGXFSZ, added on the
/// first hold and kept for the program's life. A count of holds decides when a
/// signal waits, so one write's release never lets a signal end another.
struct SignalOwner {
    holds: Mutex<usize>,
    /// Holds the number of the last stop signal during the holds, or 0.
    caught: Arc<AtomicUsize>,
    /// True while no hold runs, when a signal with its default action ends the
    /// program as it would without step 7.
    unheld: Arc<AtomicBool>,
    /// Lists the signals that had their default action before this program
    /// added its own, the only ones it ends the program for.
    had_default_action: Vec<c_int>,
}

/// Says what a signal did before this program added its own action.
#[derive(Clone, Copy, PartialEq)]
enum EarlierAction {
    Default,
    Ignored,
    /// A handler of the caller's, which signal-hook still runs first.
    Handled,
}

impl HeldSignals {
    fn was_caught(&self) -> bool {
        self.owner.caught.load(Ordering::SeqCst) != 0
    }

    /// Ends this hold, and returns the stop signal that arrived during the
    /// holds when no other hold remains.
    fn release(self) -> Option<c_int> {
        let mut holds = self
            .owner
            .holds
            .lock()
            .expect("no code panics while holding the count");
        *holds -= 1;
        if *holds > 0 {
            return None;
        }
        // Set first, so a signal that arrives now acts at once instead of being lost.
        self.owner.unheld.store(true, Ordering::SeqCst);
        match self.owner.caught.swap(0, Ordering::SeqCst) {
            0 => None,
            signal => Some(signal as c_int),
        }
    }
}

/// Returns each file the Answered Requests change, in memory, once every
/// answered comment still holds the text the user answered. A file whose User
/// Feedback keeps every text is left out.
fn rewrite_answered_requests(
    root: &Path,
    answered: &[AnsweredRequest],
) -> Result<Vec<RewrittenFile>, Error> {
    let mut answered_by_file: BTreeMap<&Path, Vec<&AnsweredRequest>> = BTreeMap::new();
    for answered_request in answered {
        answered_by_file
            .entry(&answered_request.request.file)
            .or_default()
            .push(answered_request);
    }
    let mut edits = Vec::new();
    for (file, answered_requests) in answered_by_file {
        let mut comments = read_comments_as_written(root, file)?;
        for answered_request in answered_requests {
            let request = answered_request.request;
            let Some(comment) = take_answered_comment(&mut comments, request) else {
                return Err(Error {
                    code: ErrorCode::TextChanged,
                    message: format!(
                        "A comment in {} changed after the user answered it.",
                        file.display()
                    ),
                });
            };
            let new_text = choose_new_text(request, answered_request.answer)
                .map(|new_text| match_line_ends(new_text, request, &comment));
            edits.push(CommentEdit { comment, new_text });
        }
    }
    comments::rewrite(root, &edits)
}

/// Returns the comments in the file as it is on disk, with its own line ends
/// so their byte ranges fit the file, or none when the file is gone.
fn read_comments_as_written(root: &Path, file: &Path) -> Result<Vec<Comment>, Error> {
    let Some(source) = git::read_file_on_disk(root, file)?.text else {
        // A gone file holds no answered comment, so its requests count as changed.
        return Ok(Vec::new());
    };
    comments::find_comments(file, &source).map_err(|error| Error {
        code: ErrorCode::FileUnreadable,
        message: format!(
            "Cannot read the comments in {}, since {error}.",
            file.display()
        ),
    })
}

/// Takes out the comment a request asked about, if it still holds the text the
/// user answered. A named comment is found by its name, an unnamed one by its
/// line.
fn take_answered_comment(
    comments: &mut Vec<Comment>,
    request: &FeedbackRequest,
) -> Option<Comment> {
    let index = comments.iter().position(|comment| {
        let same_comment = match &request.name {
            Some(_) => comment.name == request.name,
            None => comment.name.is_none() && comment.line == request.line,
        };
        // Requests hold `\n` line ends, so a file with `\r\n` ones still matches.
        same_comment && comment.text.replace("\r\n", "\n") == request.text
    })?;
    Some(comments.swap_remove(index))
}

/// Returns the new text with `\r\n` line ends when the old comment had any, or
/// the old text as it is when the words did not change.
fn match_line_ends(new_text: String, request: &FeedbackRequest, comment: &Comment) -> String {
    if new_text == request.text {
        comment.text.clone()
    } else if comment.text.contains("\r\n") {
        new_text.replace('\n', "\r\n")
    } else {
        new_text
    }
}

/// Returns the decision each named Answered Request saves, with an empty text
/// for a cut. A comment with no name has nothing to save it under.
fn list_decisions(answered: &[AnsweredRequest]) -> Vec<Approval> {
    answered
        .iter()
        .filter_map(|answered_request| {
            let request = answered_request.request;
            let name = request.name.clone()?;
            let text = choose_new_text(request, answered_request.answer).unwrap_or_default();
            Some(Approval {
                file: request.file.clone(),
                name,
                text,
            })
        })
        .collect()
}

/// Returns the write's error, and lets the user try again when every file went
/// back.
fn refuse_after_write(lock: &mut ReviewSessionLock, error: Error) -> Result<WriteResult, Error> {
    // The status only goes back to waiting once every file is restored.
    if error.code == ErrorCode::WriteFailed {
        save_status(lock, ReviewStatus::WaitingForUserFeedback).map_err(|save_error| Error {
            code: save_error.code,
            message: format!("{} {}", error.message, save_error.message),
        })?;
    }
    Err(error)
}

fn save_status(lock: &mut ReviewSessionLock, status: ReviewStatus) -> Result<(), Error> {
    if let Some(review_session) = lock.review_session_mut() {
        review_session.status = status;
    }
    lock.save()
}

/// Replaces the file through a synced temporary file beside it, so a crash
/// leaves either the old or the new contents whole. Returns false, replacing
/// nothing, when the file is no longer a regular file holding `expected`.
fn replace_file(
    path: &Path,
    expected: &[u8],
    contents: &[u8],
    permissions: &Permissions,
) -> io::Result<bool> {
    let mut temp_name = path
        .file_name()
        .expect("a file's resolved path ends in its name")
        .to_owned();
    temp_name.push(".comment-review.tmp");
    let temp_path = path.with_file_name(temp_name);
    create_synced_file(&temp_path, contents, permissions).map_err(|error| {
        if error.kind() == io::ErrorKind::AlreadyExists {
            io::Error::new(
                error.kind(),
                format!("{} is in the way.", temp_path.display()),
            )
        } else {
            error
        }
    })?;
    // Checking after the slow sync leaves only the rename for an edit to slip into.
    if !holds_exactly(path, expected) {
        let _ = fs::remove_file(&temp_path);
        return Ok(false);
    }
    fs::rename(&temp_path, path).inspect_err(|_| {
        // Removing the temporary file is best effort, since the write has already failed.
        let _ = fs::remove_file(&temp_path);
    })?;
    Ok(true)
}

/// Returns true when `path` is a regular file, not a link, holding exactly
/// `expected`.
fn holds_exactly(path: &Path, expected: &[u8]) -> bool {
    fs::symlink_metadata(path).is_ok_and(|found| found.is_file())
        && fs::read(path).is_ok_and(|current| current == expected)
}

/// Creates a file with the contents and permissions, synced to disk. A file
/// already at `path` is left alone, since it may be the user's.
fn create_synced_file(path: &Path, contents: &[u8], permissions: &Permissions) -> io::Result<()> {
    let mut file = File::create_new(path)?;
    file.write_all(contents)
        .and_then(|()| file.sync_all())
        .and_then(|()| fs::set_permissions(path, permissions.clone()))
        .inspect_err(|_| {
            // The half-written file is ours, and leaving it would only mislead.
            let _ = fs::remove_file(path);
        })
}

fn write_failed(reason: impl Display) -> Error {
    Error {
        code: ErrorCode::WriteFailed,
        message: format!("Every file is back as it was. {reason}"),
    }
}

fn signal_owner() -> &'static SignalOwner {
    static OWNER: OnceLock<SignalOwner> = OnceLock::new();
    OWNER.get_or_init(|| {
        let mut owner = SignalOwner {
            holds: Mutex::new(0),
            caught: Arc::new(AtomicUsize::new(0)),
            unheld: Arc::new(AtomicBool::new(true)),
            had_default_action: Vec::new(),
        };
        // SIGXFSZ waits too, so a file-size limit fails the write with EFBIG and the files go back.
        for signal in [SIGINT, SIGTERM, SIGHUP, SIGXFSZ] {
            let earlier = read_earlier_action(signal);
            if earlier == EarlierAction::Ignored {
                continue;
            }
            if STOP_SIGNALS.contains(&signal) {
                let caught = Arc::clone(&owner.caught);
                flag::register_usize(signal, caught, signal as usize)
                    .expect("signal-hook refuses only signals a program cannot catch");
            }
            if earlier == EarlierAction::Default {
                flag::register_conditional_default(signal, Arc::clone(&owner.unheld))
                    .expect("signal-hook knows the default action of these signals");
                owner.had_default_action.push(signal);
            }
        }
        owner
    })
}

fn read_earlier_action(signal: c_int) -> EarlierAction {
    // SAFETY: a zeroed sigaction is valid, and a null new action only reads the current one.
    let (read, current) = unsafe {
        let mut current: libc::sigaction = std::mem::zeroed();
        let read = libc::sigaction(signal, std::ptr::null(), &mut current);
        (read, current)
    };
    match current.sa_sigaction {
        // An action that cannot be read is treated as the caller's, so it never ends the program.
        _ if read != 0 => EarlierAction::Handled,
        libc::SIG_DFL => EarlierAction::Default,
        libc::SIG_IGN => EarlierAction::Ignored,
        _ => EarlierAction::Handled,
    }
}

/// Starts holding stop signals, adding their actions on the first call.
fn hold_stop_signals() -> HeldSignals {
    let owner = signal_owner();
    let mut holds = owner
        .holds
        .lock()
        .expect("no code panics while holding the count");
    if *holds == 0 {
        owner.caught.store(0, Ordering::SeqCst);
        owner.unheld.store(false, Ordering::SeqCst);
    }
    *holds += 1;
    HeldSignals { owner }
}

/// Ends the program with a held signal's default action, unless the signal
/// had a handler of the caller's, which already ran.
fn deliver_held_signal(signal: c_int) {
    if signal_owner().had_default_action.contains(&signal) {
        low_level::emulate_default_handler(signal)
            .expect("signal-hook knows the default action of SIGINT, SIGTERM and SIGHUP");
    }
}

fn nothing_to_write(reason: &str) -> Error {
    Error {
        code: ErrorCode::NothingToWrite,
        message: format!("Nothing to write. {reason}"),
    }
}

/// Lets one test at a time hold stop signals, since every hold in the process
/// shares one count.
#[cfg(test)]
static SIGNAL_TESTS: Mutex<()> = Mutex::new(());

#[cfg(test)]
mod write_every_file {
    use std::process::Command;

    use super::*;

    /// `write_every_file` must stop at a stop signal, so that every written file can go back.
    /// Otherwise a Ctrl-C halfway through would leave some files with the new User Feedback and others without.
    #[test]
    fn puts_every_file_back_after_stop_signal() {
        let _one_signal_test = SIGNAL_TESTS.lock().unwrap();
        let output = Command::new("mktemp").arg("-d").output().unwrap();
        let folder = PathBuf::from(String::from_utf8(output.stdout).unwrap().trim_end());
        // Two files hold the old comment, and the backup saves both.
        let files = ["a.go", "b.go"].map(|name| {
            fs::write(folder.join(name), "// Old.\n").unwrap();
            RewrittenFile {
                path: PathBuf::from(name),
                old_contents: "// Old.\n".to_owned(),
                new_contents: "// New.\n".to_owned(),
            }
        });
        let mut backup = FileBackup::save(&folder, folder.join("backup"), files.into()).unwrap();
        // Raise SIGINT during the hold, so the write stops after its first file.
        let signals = hold_stop_signals();
        low_level::raise(SIGINT).unwrap();

        let written = write_every_file(&mut backup, &signals);

        // Put back what was written, as step 7 does after a failed write.
        let caught = signals.release();
        let error = backup.undo(written.unwrap_err());
        let contents = ["a.go", "b.go"].map(|name| fs::read_to_string(folder.join(name)).unwrap());
        fs::remove_dir_all(&folder).unwrap();
        assert_eq!(caught, Some(SIGINT));
        assert_eq!(error.code, ErrorCode::WriteFailed);
        assert_eq!(contents, ["// Old.\n", "// Old.\n"]);
    }

    /// `write_every_file` must stop at a file that changed since its rewrite and leave it as it is.
    /// Otherwise one file would miss its User Feedback while the other files and every decision are saved.
    #[test]
    fn stops_at_file_changed_since_rewrite() {
        let _one_signal_test = SIGNAL_TESTS.lock().unwrap();
        let output = Command::new("mktemp").arg("-d").output().unwrap();
        let folder = PathBuf::from(String::from_utf8(output.stdout).unwrap().trim_end());
        // Two files hold the old comment, and the backup saves both.
        let files = ["a.go", "b.go"].map(|name| {
            fs::write(folder.join(name), "// Old.\n").unwrap();
            RewrittenFile {
                path: PathBuf::from(name),
                old_contents: "// Old.\n".to_owned(),
                new_contents: "// New.\n".to_owned(),
            }
        });
        let mut backup = FileBackup::save(&folder, folder.join("backup"), files.into()).unwrap();
        fs::write(folder.join("b.go"), "// Edited.\n").unwrap();
        let signals = hold_stop_signals();

        let written = write_every_file(&mut backup, &signals);

        signals.release();
        let b_contents = fs::read_to_string(folder.join("b.go")).unwrap();
        fs::remove_dir_all(&folder).unwrap();
        assert!(written.is_err());
        assert_eq!(backup.written, 1);
        assert_eq!(b_contents, "// Edited.\n");
    }
}

#[cfg(test)]
mod hold_stop_signals {
    use super::*;

    /// `hold_stop_signals` must keep a stop signal waiting until the last of two holds ends.
    /// Otherwise one write's release would let a Ctrl-C end the program halfway through another write.
    #[test]
    fn keeps_signal_until_last_hold_ends() {
        let _one_signal_test = SIGNAL_TESTS.lock().unwrap();
        let first = hold_stop_signals();
        let second = hold_stop_signals();

        let first_release = first.release();
        low_level::raise(SIGINT).unwrap();
        let caught_by_second = second.was_caught();
        let second_release = second.release();

        assert_eq!(first_release, None);
        assert!(caught_by_second);
        assert_eq!(second_release, Some(SIGINT));
    }
}
