use crate::taskfile::{self, validate_task_name, DEFAULT_TASK_FILENAME};
use serde::Serialize;
use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const SESSION_DIR: &str = ".surveil-session";
const TASK_ARTIFACT_DIR: &str = "tasks";
const RECEIPT_FILENAME: &str = "receipt.json";
const SESSION_SCHEMA_VERSION: &str = "surveil.session.v1";

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum SessionStatus {
    Complete,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum ArtifactKind {
    Context,
    Trace,
    Report,
    Evidence,
}

#[derive(Debug, Serialize)]
struct ArtifactRecord {
    kind: ArtifactKind,
    task_name: Option<String>,
    path: String,
    byte_len: u64,
    sha256: String,
}

#[derive(Debug, Serialize)]
struct SessionReceipt {
    schema_version: String,
    status: SessionStatus,
    repo_root: String,
    task_names: Vec<String>,
    artifacts: Vec<ArtifactRecord>,
}

#[derive(Debug)]
struct SessionTask {
    name: String,
    task_file: PathBuf,
}

fn resolve_repo(repo: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let repo = fs::canonicalize(repo)?;
    if !repo.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "session repository must be a directory",
        )
        .into());
    }
    if repo.to_str().is_none() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "session repository path must be UTF-8",
        )
        .into());
    }
    Ok(repo)
}

fn discover_tasks(root: &Path) -> Result<Vec<SessionTask>, Box<dyn Error>> {
    taskfile::validate_existing_managed_root(root)?;
    if root.to_str().is_none() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "session managed root path must be UTF-8",
        )
        .into());
    }
    let mut tasks = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "managed root entries must not be symlinks",
            )
            .into());
        }
        if !file_type.is_dir() {
            continue;
        }
        let task_file = entry.path().join(DEFAULT_TASK_FILENAME);
        let metadata = match fs::symlink_metadata(&task_file) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        };
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "task.json must be a direct regular file",
            )
            .into());
        }
        let task_dir = fs::canonicalize(entry.path())?;
        let resolved_task = fs::canonicalize(&task_file)?;
        if resolved_task.parent() != Some(task_dir.as_path()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "task.json must remain inside its task directory",
            )
            .into());
        }
        let name = entry.file_name().into_string().map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "task name must be UTF-8")
        })?;
        validate_task_name(&name)?;
        if name == SESSION_DIR {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "task name .surveil-session is reserved for session output",
            )
            .into());
        }
        tasks.push(SessionTask { name, task_file });
    }
    tasks.sort_by(|left, right| left.name.cmp(&right.name));
    if tasks.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "managed root has no populated task directories",
        )
        .into());
    }
    Ok(tasks)
}

pub fn run(repo: &Path, root: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let _repo = resolve_repo(repo)?;
    let _tasks = discover_tasks(root)?;
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "session execution is not implemented",
    )
    .into())
}
