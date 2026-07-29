use crate::schema::{ExplicitFile, GatherOutput, SCHEMA_VERSION};
use crate::source;
use crate::taskfile::{read_task_file, validate_task_name, DEFAULT_TASK_FILENAME};
use std::error::Error;
use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub fn run(repo_root: &Path, task_file: &Path) -> Result<(), Box<dyn Error>> {
    let task_file = fs::canonicalize(task_file)?;
    let task_name = task_name_from_resolved(&task_file)?;
    let task = read_task_file(&task_file)?;

    let validated_explicit_files = validate_explicit_files(repo_root, &task.explicit_files)?;
    validate_search_areas(repo_root, &task.search_areas)?;

    let output = GatherOutput {
        schema_version: SCHEMA_VERSION.to_string(),
        task_name,
        repo_root: repo_root.to_string_lossy().into_owned(),
        summary: task.summary,
        explicit_files: validated_explicit_files.explicit_files,
        missing_explicit_files: validated_explicit_files.missing_explicit_files,
        skipped_explicit_files: validated_explicit_files.skipped_explicit_files,
        search_areas: task.search_areas,
        query: task.query,
        terms: task.terms,
        blockers: Vec::new(),
    };

    let stdout = io::stdout();
    let mut handle = stdout.lock();
    serde_json::to_writer(&mut handle, &output)?;
    handle.write_all(b"\n")?;
    Ok(())
}

fn task_name_from_resolved(task_file: &Path) -> io::Result<String> {
    if task_file.file_name() != Some(OsStr::new(DEFAULT_TASK_FILENAME)) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "task file must be named task.json",
        ));
    }
    let task_name = task_file
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "task.json parent must have a UTF-8 task name",
            )
        })?;
    validate_task_name(task_name)?;
    Ok(task_name.to_string())
}

#[derive(Debug, PartialEq, Eq)]
struct ValidatedExplicitFiles {
    explicit_files: Vec<ExplicitFile>,
    missing_explicit_files: Vec<String>,
    skipped_explicit_files: Vec<String>,
}

fn validate_explicit_files(
    repo_root: &Path,
    files: &[String],
) -> Result<ValidatedExplicitFiles, Box<dyn Error>> {
    let mut explicit_files = Vec::with_capacity(files.len());
    let mut missing_explicit_files = Vec::new();
    let mut skipped_explicit_files = Vec::new();

    for path in files {
        let resolved = source::resolve_path(repo_root, path);
        if source::is_skipped_path(repo_root, &resolved) {
            explicit_files.push(ExplicitFile {
                path: path.clone(),
                found: false,
            });
            skipped_explicit_files.push(path.clone());
            continue;
        }

        if resolved.is_file() {
            explicit_files.push(ExplicitFile {
                path: path.clone(),
                found: true,
            });
            continue;
        }

        if resolved.exists() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("explicit path is not a file: {path}"),
            )
            .into());
        }

        explicit_files.push(ExplicitFile {
            path: path.clone(),
            found: false,
        });
        missing_explicit_files.push(path.clone());
    }

    Ok(ValidatedExplicitFiles {
        explicit_files,
        missing_explicit_files,
        skipped_explicit_files,
    })
}

#[derive(Debug, PartialEq, Eq)]
enum SearchAreaError {
    Empty,
    Malformed(String),
    NotFound { area: String, resolved: PathBuf },
    Skipped { area: String, resolved: PathBuf },
}

impl fmt::Display for SearchAreaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("search area must not be empty"),
            Self::Malformed(area) => write!(formatter, "search area contains a NUL byte: {area:?}"),
            Self::NotFound { area, resolved } => write!(
                formatter,
                "search area {area:?} resolved as {}, but that path does not exist; relative search areas resolve against --repo and '.' selects the repository root",
                resolved.display(),
            ),
            Self::Skipped { area, resolved } => write!(
                formatter,
                "search area {area:?} resolved as {}, but that path is excluded by the Surveil skip policy",
                resolved.display(),
            ),
        }
    }
}

impl Error for SearchAreaError {}

fn validate_search_area(repo_root: &Path, area: &str) -> Result<PathBuf, SearchAreaError> {
    if area.trim().is_empty() {
        return Err(SearchAreaError::Empty);
    }
    if area.contains('\0') {
        return Err(SearchAreaError::Malformed(area.to_string()));
    }

    let resolved = source::resolve_path(repo_root, area);
    if !resolved.exists() {
        return Err(SearchAreaError::NotFound {
            area: area.to_string(),
            resolved,
        });
    }
    if source::is_skipped_path(repo_root, &resolved) {
        return Err(SearchAreaError::Skipped {
            area: area.to_string(),
            resolved,
        });
    }
    Ok(resolved)
}

fn validate_search_areas(repo_root: &Path, search_areas: &[String]) -> Result<(), SearchAreaError> {
    for area in search_areas {
        validate_search_area(repo_root, area)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        task_name_from_resolved, validate_explicit_files, validate_search_area, SearchAreaError,
        ValidatedExplicitFiles,
    };
    use crate::schema::ExplicitFile;
    use std::fs;
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_repo(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("surveil-gather-{name}-{stamp}"));
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

    struct ValidationCase {
        name: &'static str,
        dirs: Vec<&'static str>,
        files: Vec<(&'static str, &'static str)>,
        explicit_files: Vec<String>,
        expected: Result<ValidatedExplicitFiles, &'static str>,
    }

    #[test]
    fn explicit_file_validation_case_tables() {
        let cases = vec![
            ValidationCase {
                name: "keeps-missing-and-skipped-paths",
                dirs: vec![],
                files: vec![("src/lib.rs", "fn main() {}\n")],
                explicit_files: vec![
                    "docs/future.md".to_string(),
                    ".surveil/index.sqlite".to_string(),
                    "src/lib.rs".to_string(),
                ],
                expected: Ok(ValidatedExplicitFiles {
                    explicit_files: vec![
                        ExplicitFile {
                            path: "docs/future.md".to_string(),
                            found: false,
                        },
                        ExplicitFile {
                            path: ".surveil/index.sqlite".to_string(),
                            found: false,
                        },
                        ExplicitFile {
                            path: "src/lib.rs".to_string(),
                            found: true,
                        },
                    ],
                    missing_explicit_files: vec!["docs/future.md".to_string()],
                    skipped_explicit_files: vec![".surveil/index.sqlite".to_string()],
                }),
            },
            ValidationCase {
                name: "rejects-existing-directory",
                dirs: vec!["src"],
                files: vec![],
                explicit_files: vec!["src".to_string()],
                expected: Err("not a file"),
            },
        ];

        for case in &cases {
            let repo = temp_repo(case.name);
            for dir in &case.dirs {
                fs::create_dir_all(repo.join(dir)).expect("create dir");
            }
            for (path, content) in &case.files {
                write_file(&repo.join(path), content);
            }

            match &case.expected {
                Ok(expected) => {
                    let validated = validate_explicit_files(&repo, &case.explicit_files)
                        .expect("validate explicit files");
                    assert_eq!(&validated, expected, "case: {}", case.name);
                }
                Err(expected_fragment) => {
                    let err = validate_explicit_files(&repo, &case.explicit_files)
                        .expect_err("reject invalid explicit path");
                    assert!(
                        err.to_string().contains(expected_fragment),
                        "case: {}",
                        case.name
                    );
                }
            }

            let _ = fs::remove_dir_all(repo);
        }
    }

    #[test]
    fn derives_task_names_from_task_json() {
        let repo = temp_repo("task-name");
        let task_file = repo.join("architecture/task.json");
        write_file(
            &task_file,
            r#"{"summary":"summary","explicit_files":[],"search_areas":["."],"query":["Where?"],"terms":[]}"#,
        );
        let resolved = fs::canonicalize(&task_file).expect("resolve task file");
        assert_eq!(
            task_name_from_resolved(&resolved).expect("task name"),
            "architecture"
        );
        assert!(task_name_from_resolved(Path::new("/task.json")).is_err());
        assert!(task_name_from_resolved(Path::new("/line\nbreak/task.json")).is_err());
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn test_validate_search_area() {
        let repo = temp_repo("search-area");
        fs::create_dir_all(repo.join("src")).expect("create relative area");
        fs::create_dir_all(repo.join(".surveil")).expect("create skipped area");

        assert_eq!(
            validate_search_area(&repo, "src").expect("relative path"),
            repo.join("src"),
        );
        let absolute = repo.join("src");
        assert_eq!(
            validate_search_area(&repo, absolute.to_str().expect("UTF-8 path"))
                .expect("absolute path"),
            absolute,
        );
        assert_eq!(
            validate_search_area(&repo, ".").expect("repo root"),
            repo.join("."),
        );
        for area in ["", "   "] {
            assert_eq!(
                validate_search_area(&repo, area).expect_err("empty path"),
                SearchAreaError::Empty,
            );
        }
        assert!(matches!(
            validate_search_area(&repo, "bad\0path").expect_err("malformed path"),
            SearchAreaError::Malformed(_),
        ));
        assert!(matches!(
            validate_search_area(&repo, "missing").expect_err("missing path"),
            SearchAreaError::NotFound { .. },
        ));
        assert!(matches!(
            validate_search_area(&repo, ".surveil").expect_err("skipped path"),
            SearchAreaError::Skipped { .. },
        ));

        let _ = fs::remove_dir_all(repo);
    }
}
