use std::{
    ffi::OsStr,
    fs,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub(crate) struct PreparedSkillRoot {
    pub(super) path: PathBuf,
    device: u64,
    inode: u64,
    source: SkillRootSource,
}

pub(crate) struct PreparedSkills {
    pub(crate) user: Option<PreparedSkillRoot>,
    pub(crate) repository: Option<PreparedSkillRoot>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SkillRootSource {
    User,
    Repository,
}

impl SkillRootSource {
    fn label(self) -> &'static str {
        match self {
            Self::User => "user skills",
            Self::Repository => "repository skills",
        }
    }
}

fn reject_symlink_ancestry(path: &Path, label: &str) -> Result<(), String> {
    for ancestor in path.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(format!(
                    "{label} path ancestry cannot contain a symlink: {}",
                    ancestor.display()
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(format!(
                    "read {label} path ancestry {}: {error}",
                    ancestor.display()
                ));
            }
        }
    }
    Ok(())
}

fn docker_mount_path(path: &Path, label: &str) -> Result<(), String> {
    let text = path
        .to_str()
        .ok_or_else(|| format!("{label} must be valid UTF-8 to mount skills"))?;
    if text.contains(',') || text.contains('"') || text.contains('\n') || text.contains('\r') {
        return Err(format!(
            "{label} contains syntax unsafe for Docker mounts: {}",
            path.display()
        ));
    }
    Ok(())
}

fn paths_overlap(left: &Path, right: &Path) -> bool {
    left.starts_with(right) || right.starts_with(left)
}

fn reject_writable_mount_overlap(
    skills: &Path,
    skills_label: &str,
    mount_label: &str,
    writable_mount: &Path,
) -> Result<(), String> {
    let writable_mount = fs::canonicalize(writable_mount)
        .map_err(|error| format!("resolve {mount_label} for skills validation: {error}"))?;
    if paths_overlap(skills, &writable_mount) {
        return Err(format!(
            "{skills_label} path overlaps writable Docker mount {mount_label}: {}",
            skills.display()
        ));
    }
    Ok(())
}

pub(super) fn reject_user_skills_writable_overlap(
    skills: &Path,
    worktree: &Path,
    git_common_dir: &Path,
    artifacts: &Path,
    pi_agent_dir: Option<&Path>,
) -> Result<(), String> {
    for (label, writable_mount) in [
        ("worktree", worktree),
        ("shared Git directory", git_common_dir),
        ("artifacts", artifacts),
    ] {
        reject_writable_mount_overlap(skills, "user skills", label, writable_mount)?;
    }
    if let Some(pi_agent_dir) = pi_agent_dir {
        reject_writable_mount_overlap(skills, "user skills", "Pi state directory", pi_agent_dir)?;
    }
    Ok(())
}

pub(super) fn validate_repository_skills_mount(
    skills: &Path,
    worktree: &Path,
    git_common_dir: &Path,
    artifacts: &Path,
    pi_agent_dir: Option<&Path>,
) -> Result<(), String> {
    let expected = fs::canonicalize(worktree.join(".agents/skills"))
        .map_err(|error| format!("resolve repository skills in worktree: {error}"))?;
    if skills != expected {
        return Err(format!(
            "repository skills path moved outside the managed worktree: {}",
            skills.display()
        ));
    }
    for (label, writable_mount) in [
        ("shared Git directory", git_common_dir),
        ("artifacts", artifacts),
    ] {
        reject_writable_mount_overlap(skills, "repository skills", label, writable_mount)?;
    }
    if let Some(pi_agent_dir) = pi_agent_dir {
        reject_writable_mount_overlap(
            skills,
            "repository skills",
            "Pi state directory",
            pi_agent_dir,
        )?;
    }
    Ok(())
}

fn reject_descendant_symlinks(root: &Path, label: &str) -> Result<(), String> {
    let mut directories = vec![root.to_path_buf()];
    while let Some(directory) = directories.pop() {
        let entries = fs::read_dir(&directory)
            .map_err(|error| format!("read {label} {}: {error}", directory.display()))?;
        for entry in entries {
            let entry = entry.map_err(|error| {
                format!("read {label} entry in {}: {error}", directory.display())
            })?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| format!("read {label} metadata {}: {error}", path.display()))?;
            if metadata.file_type().is_symlink() {
                return Err(format!(
                    "{label} cannot contain symlinks: {}",
                    path.display()
                ));
            }
            if metadata.is_file() && metadata.nlink() > 1 {
                return Err(format!(
                    "{label} cannot contain multiply-linked files: {}",
                    path.display()
                ));
            }
            if metadata.is_dir() {
                directories.push(path);
            }
        }
    }
    Ok(())
}

fn inspect_skill_root_with<F>(
    skills_dir: &Path,
    source: SkillRootSource,
    read_dir: F,
) -> Result<Option<PreparedSkillRoot>, String>
where
    F: FnOnce(&Path) -> std::io::Result<()>,
{
    let label = source.label();
    let metadata = match fs::symlink_metadata(skills_dir) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "read {label} metadata {}: {error}",
                skills_dir.display()
            ))
        }
    };
    reject_symlink_ancestry(skills_dir, label)?;
    match metadata {
        metadata if metadata.file_type().is_symlink() => Err(format!(
            "{label} path cannot be a symlink: {}",
            skills_dir.display()
        )),
        metadata if metadata.is_dir() => {
            let resolved = match fs::canonicalize(skills_dir) {
                Ok(resolved) => resolved,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(error) => {
                    return Err(format!("resolve {label} {}: {error}", skills_dir.display()))
                }
            };
            let resolved_metadata = match fs::metadata(&resolved) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(error) => {
                    return Err(format!(
                        "read {label} metadata {}: {error}",
                        resolved.display()
                    ))
                }
            };
            docker_mount_path(&resolved, label)?;
            if metadata.dev() != resolved_metadata.dev()
                || metadata.ino() != resolved_metadata.ino()
            {
                return Err(format!(
                    "{label} path changed during validation: {}",
                    skills_dir.display()
                ));
            }
            match read_dir(&resolved) {
                Ok(()) => {
                    if source == SkillRootSource::Repository {
                        reject_descendant_symlinks(&resolved, label)?;
                    }
                    Ok(Some(PreparedSkillRoot {
                        path: resolved,
                        device: metadata.dev(),
                        inode: metadata.ino(),
                        source,
                    }))
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(error) => Err(format!("read {label} {}: {error}", resolved.display())),
            }
        }
        _ => Err(format!(
            "{label} path is not a directory: {}",
            skills_dir.display()
        )),
    }
}

pub(super) fn prepare_user_skills_with<F>(
    home: Option<&OsStr>,
    read_dir: F,
) -> Result<Option<PreparedSkillRoot>, String>
where
    F: FnOnce(&Path) -> std::io::Result<()>,
{
    let Some(home) = home else {
        return Ok(None);
    };
    let home_path = PathBuf::from(home);
    let skills_dir = home_path.join(".agents/skills");
    let prepared = inspect_skill_root_with(&skills_dir, SkillRootSource::User, read_dir)?;
    if prepared.is_none() {
        return Ok(None);
    }

    let home = home
        .to_str()
        .ok_or_else(|| "HOME must be valid UTF-8 to mount user skills".to_string())?;
    if home.is_empty() || !home_path.is_absolute() {
        return Err("HOME must be an absolute path to mount user skills".to_string());
    }
    docker_mount_path(&home_path, "HOME")?;
    Ok(prepared)
}

pub(crate) fn prepare_user_skills(
    home: Option<&OsStr>,
) -> Result<Option<PreparedSkillRoot>, String> {
    prepare_user_skills_with(home, |path| fs::read_dir(path).map(|_| ()))
}

pub(crate) fn prepare_repository_skills(
    worktree: &Path,
) -> Result<Option<PreparedSkillRoot>, String> {
    inspect_skill_root_with(
        &worktree.join(".agents/skills"),
        SkillRootSource::Repository,
        |path| fs::read_dir(path).map(|_| ()),
    )
}

pub(super) fn revalidate_skill_root(
    prepared: &PreparedSkillRoot,
) -> Result<Option<PathBuf>, String> {
    let Some(current) = inspect_skill_root_with(&prepared.path, prepared.source, |path| {
        fs::read_dir(path).map(|_| ())
    })?
    else {
        return Ok(None);
    };
    if current.device != prepared.device || current.inode != prepared.inode {
        return Err(format!(
            "{} path changed after validation: {}",
            prepared.source.label(),
            prepared.path.display()
        ));
    }
    Ok(Some(current.path))
}
