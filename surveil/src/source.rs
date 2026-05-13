use crate::schema::ExplicitFile;
use std::collections::HashSet;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceFile {
    path: PathBuf,
    display_path: String,
    explicit: bool,
}

impl SourceFile {
    pub(crate) fn new(repo_root: &Path, path: PathBuf, explicit: bool) -> Self {
        Self {
            display_path: display_path(repo_root, &path),
            path,
            explicit,
        }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn display_path(&self) -> &str {
        &self.display_path
    }

    pub(crate) fn is_explicit(&self) -> bool {
        self.explicit
    }
}

pub fn collect_candidate_files(
    repo_root: &Path,
    search_areas: &[String],
    explicit_files: &[ExplicitFile],
    skipped_paths: &mut Vec<String>,
) -> Result<Vec<SourceFile>, Box<dyn Error>> {
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();

    let mut explicit_paths: Vec<PathBuf> = explicit_files
        .iter()
        .filter(|file| file.found)
        .map(|file| resolve_path(repo_root, &file.path))
        .collect();
    explicit_paths.sort();
    for file in explicit_paths {
        if is_skipped_path(repo_root, &file) {
            skipped_paths.push(display_path(repo_root, &file));
            continue;
        }
        if seen.insert(file.clone()) {
            candidates.push(SourceFile::new(repo_root, file, true));
        }
    }

    for area in search_areas {
        let area_path = resolve_path(repo_root, area);
        for file in collect_files(repo_root, &area_path, skipped_paths)? {
            if seen.insert(file.clone()) {
                candidates.push(SourceFile::new(repo_root, file, false));
            }
        }
    }

    Ok(candidates)
}

pub fn collect_files(
    repo_root: &Path,
    dir: &Path,
    skipped_paths: &mut Vec<String>,
) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    if is_skipped_path(repo_root, dir) {
        skipped_paths.push(display_path(repo_root, dir));
        return Ok(Vec::new());
    }

    if dir.is_file() {
        return Ok(vec![dir.to_path_buf()]);
    }

    if !dir.is_dir() {
        skipped_paths.push(display_path(repo_root, dir));
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    let read_dir = match fs::read_dir(dir) {
        Ok(read_dir) => read_dir,
        Err(_) => {
            skipped_paths.push(display_path(repo_root, dir));
            return Ok(Vec::new());
        }
    };
    for entry in read_dir {
        match entry {
            Ok(entry) => entries.push(entry.path()),
            Err(_) => skipped_paths.push(display_path(repo_root, dir)),
        }
    }
    entries.sort();

    let mut files = Vec::new();
    for path in entries {
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(_) => {
                skipped_paths.push(display_path(repo_root, &path));
                continue;
            }
        };
        if is_skipped_path(repo_root, &path) {
            skipped_paths.push(display_path(repo_root, &path));
            continue;
        }
        if metadata.is_dir() {
            files.extend(collect_files(repo_root, &path, skipped_paths)?);
        } else if metadata.is_file() {
            files.push(path);
        }
    }
    Ok(files)
}

pub fn resolve_path(repo_root: &Path, raw: &str) -> PathBuf {
    let path = Path::new(raw);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo_root.join(path)
    }
}

pub fn is_skipped_path(repo_root: &Path, path: &Path) -> bool {
    let relative = path.strip_prefix(repo_root).unwrap_or(path);
    relative.components().any(|component| {
        matches!(
            component,
            std::path::Component::Normal(name)
                if matches!(
                    name.to_string_lossy().as_ref(),
                    "target" | "node_modules" | "dist" | "build" | "pack" | ".git" | ".surveil"
                )
        )
    })
}

pub fn display_path(repo_root: &Path, path: &Path) -> String {
    path.strip_prefix(repo_root)
        .map(|relative| relative.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::{collect_candidate_files, collect_files, is_skipped_path};
    use crate::schema::ExplicitFile;
    use std::fs;
    use std::io::Write;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_repo(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("surveil-{name}-{stamp}"));
        fs::create_dir_all(&path).expect("create temp repo");
        path
    }

    fn write_file(path: &PathBuf, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent dirs");
        }
        let mut file = fs::File::create(path).expect("create file");
        file.write_all(content.as_bytes()).expect("write file");
    }

    #[test]
    fn skips_repo_local_surveil_artifacts() {
        let repo = temp_repo("source");
        write_file(&repo.join(".surveil/index.sqlite"), "cache");
        write_file(&repo.join("src/lib.rs"), "fn main() {}\n");

        let mut skipped_paths = Vec::new();
        let files = collect_files(&repo, &repo, &mut skipped_paths).expect("collect files");

        assert!(files.iter().any(|path| path.ends_with("src/lib.rs")));
        assert!(!files.iter().any(|path| path.to_string_lossy().contains(".surveil")));
        assert!(skipped_paths.iter().any(|path| path == ".surveil"));
        assert!(is_skipped_path(&repo, &repo.join(".surveil/index.sqlite")));

        let mut candidate_skips = Vec::new();
        let candidates = collect_candidate_files(
            &repo,
            &[],
            &[ExplicitFile {
                path: ".surveil/index.sqlite".into(),
                found: true,
            }],
            &mut candidate_skips,
        )
        .expect("collect candidates");
        assert!(candidates.is_empty());
        assert!(candidate_skips.iter().any(|path| path == ".surveil/index.sqlite"));

        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn candidate_files_carry_display_path_and_explicit_marker() {
        let repo = temp_repo("source-record");
        write_file(&repo.join("notes/design.md"), "design\n");
        write_file(&repo.join("src/lib.rs"), "fn main() {}\n");

        let mut skipped_paths = Vec::new();
        let candidates = collect_candidate_files(
            &repo,
            &["src/".to_string()],
            &[ExplicitFile {
                path: "notes/design.md".to_string(),
                found: true,
            }],
            &mut skipped_paths,
        )
        .expect("collect candidates");

        assert_eq!(candidates[0].display_path(), "notes/design.md");
        assert!(candidates[0].is_explicit());
        assert_eq!(candidates[1].display_path(), "src/lib.rs");
        assert!(!candidates[1].is_explicit());

        let _ = fs::remove_dir_all(repo);
    }
}
