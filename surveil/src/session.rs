use crate::gather;
use crate::merge;
use crate::research;
use crate::taskfile::{self, validate_task_name, DEFAULT_TASK_FILENAME};
use rustix::fs::{renameat_with, RenameFlags, CWD};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

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
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "task name must be UTF-8"))?;
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

pub(crate) fn run(repo: &Path, root: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let repo = resolve_repo(repo)?;
    let tasks = discover_tasks(root)?;
    let session_dir = root.join(SESSION_DIR);
    match fs::symlink_metadata(&session_dir) {
        Ok(_) => {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "managed root already contains .surveil-session",
            )
            .into())
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }

    let staging = create_staging_dir(root)?;
    let result = execute(&repo, &staging, &tasks);
    let receipt = match result {
        Ok(receipt) => receipt,
        Err(error) => {
            let _ = fs::remove_dir_all(&staging);
            return Err(error);
        }
    };
    if let Err(error) = renameat_with(
        CWD,
        &staging,
        CWD,
        &session_dir,
        RenameFlags::NOREPLACE,
    ) {
        let _ = fs::remove_dir_all(&staging);
        return Err(error.into());
    }
    Ok(session_dir.join(receipt))
}

fn execute(
    repo: &Path,
    staging: &Path,
    tasks: &[SessionTask],
) -> Result<&'static str, Box<dyn Error>> {
    let mut reports = Vec::with_capacity(tasks.len());
    let mut artifacts = Vec::with_capacity(tasks.len() * 3 + 1);

    for task in tasks {
        let context = gather::create_output(repo, &task.task_file)?;
        let (report, trace) = research::create_research_outputs(context.clone())?;
        let task_root = format!("{TASK_ARTIFACT_DIR}/{}", task.name);
        for (kind, file, value) in [
            (ArtifactKind::Context, "context.json", serde_json::to_value(&context)?),
            (ArtifactKind::Trace, "trace.json", serde_json::to_value(&trace)?),
            (ArtifactKind::Report, "report.json", serde_json::to_value(&report)?),
        ] {
            let relative = format!("{task_root}/{file}");
            artifacts.push(write_artifact(
                staging,
                &relative,
                kind,
                Some(&task.name),
                &value,
            )?);
        }
        reports.push(report);
    }

    let evidence = merge::merge_outputs(reports)?;
    artifacts.push(write_artifact(
        staging,
        "evidence.json",
        ArtifactKind::Evidence,
        None,
        &evidence,
    )?);
    let receipt = SessionReceipt {
        schema_version: SESSION_SCHEMA_VERSION.to_string(),
        status: SessionStatus::Complete,
        repo_root: repo.to_str().expect("validated UTF-8 repo").to_string(),
        task_names: tasks.iter().map(|task| task.name.clone()).collect(),
        artifacts,
    };
    write_new(&staging.join(RECEIPT_FILENAME), &encode_json(&receipt)?)?;
    Ok(RECEIPT_FILENAME)
}

fn create_staging_dir(root: &Path) -> io::Result<PathBuf> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos();
    for attempt in 0..10 {
        let path = root.join(format!(
            ".surveil-session-{}-{stamp}-{attempt}.tmp",
            std::process::id()
        ));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not allocate session staging directory",
    ))
}

fn write_artifact(
    root: &Path,
    relative: &str,
    kind: ArtifactKind,
    task_name: Option<&str>,
    value: &impl Serialize,
) -> Result<ArtifactRecord, Box<dyn Error>> {
    let bytes = encode_json(value)?;
    write_new(&artifact_path(root, relative)?, &bytes)?;
    Ok(ArtifactRecord {
        kind,
        task_name: task_name.map(str::to_string),
        path: relative.to_string(),
        byte_len: bytes.len() as u64,
        sha256: format!("{:x}", Sha256::digest(&bytes)),
    })
}

fn artifact_path(root: &Path, relative: &str) -> io::Result<PathBuf> {
    let relative = Path::new(relative);
    let mut components = relative.components();
    let valid = !relative.is_absolute()
        && matches!(components.next(), Some(Component::Normal(_)))
        && components.all(|part| matches!(part, Component::Normal(_)));
    if !valid {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "artifact path must contain only relative normal components",
        ));
    }
    Ok(root.join(relative))
}

fn encode_json(value: &impl Serialize) -> Result<Vec<u8>, serde_json::Error> {
    let mut bytes = serde_json::to_vec(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(bytes)?;
    file.flush()?;
    Ok(())
}
