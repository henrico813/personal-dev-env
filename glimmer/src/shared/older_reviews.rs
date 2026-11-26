//! This module copies the Review Sessions saved before glimmer kept an
//! archive, once per PC. Read the `archive_older_reviews(home)` function to
//! understand the module.

use std::fs::{self, File};
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::review_session::{find_waiting_list_folder, write_archive_copy};

const MARKER_FILE: &str = "older-reviews-archived.txt";
const LOCK_FILE: &str = "older-reviews-archived.lock";
const SKIPPED_FOLDERS: [&str; 3] = ["node_modules", "target", ".cache"];

pub fn archive_older_reviews(home: &Path) -> io::Result<()> {
    let folder = find_waiting_list_folder().map_err(|error| io::Error::other(error.message))?;
    let marker = folder.join(MARKER_FILE);
    if marker.exists() {
        return Ok(());
    }
    fs::create_dir_all(&folder)?;
    let lock_file = File::create(folder.join(LOCK_FILE))?;
    if lock_file.try_lock().is_err() || marker.exists() {
        return Ok(());
    }
    let reviews = find_review_files(home)?;
    let failures: Vec<String> = reviews
        .iter()
        .filter_map(|review| {
            let error = copy_review_file(review).err()?;
            Some(format!("{} ({error})", review.display()))
        })
        .collect();
    let copied = reviews.len() - failures.len();
    let mut summary = format!("Copied {copied} of {} older reviews.\n", reviews.len());
    if let Some(failure) = failures.first() {
        summary.push_str(&format!("Could not copy {failure}\n"));
    }
    fs::write(marker, summary)
}

fn find_review_files(home: &Path) -> io::Result<Vec<PathBuf>> {
    let device = fs::metadata(home)?.dev();
    let mut reviews = Vec::new();
    let mut folders = vec![home.to_path_buf()];
    while let Some(folder) = folders.pop() {
        let Ok(entries) = fs::read_dir(&folder) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let is_folder = entry.file_type().is_ok_and(|kind| kind.is_dir());
            let is_on_device = entry.metadata().is_ok_and(|info| info.dev() == device);
            if !is_folder || !is_on_device || SKIPPED_FOLDERS.contains(&name.as_ref()) {
                continue;
            }
            if name == ".git" {
                reviews.extend(find_reviews_in_git_folder(&entry.path()));
            } else {
                folders.push(entry.path());
            }
        }
    }
    Ok(reviews)
}

fn find_reviews_in_git_folder(git_folder: &Path) -> Vec<PathBuf> {
    let worktrees = fs::read_dir(git_folder.join("worktrees"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path());
    std::iter::once(git_folder.to_path_buf())
        .chain(worktrees)
        .map(|folder| folder.join("pde").join("review.json"))
        .filter(|review| review.is_file())
        .collect()
}

fn copy_review_file(review: &Path) -> Result<(), String> {
    #[derive(Deserialize)]
    struct SavedReview {
        origin: SavedOrigin,
    }
    #[derive(Deserialize)]
    struct SavedOrigin {
        repo: String,
        branch: String,
    }

    let text = fs::read(review).map_err(|error| error.to_string())?;
    let saved: SavedReview = serde_json::from_slice(&text).map_err(|error| error.to_string())?;
    write_archive_copy(&saved.origin.repo, &saved.origin.branch, &text)
        .map_err(|error| error.message)
}
