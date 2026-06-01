use crate::schema::{ExplicitFile, GatherOutput, SCHEMA_VERSION};
use crate::source;
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

pub fn run(repo_root: &Path, task_file: &Path) -> Result<(), Box<dyn Error>> {
    let task_text = fs::read_to_string(task_file)?;
    let parsed = parse_task(&task_text)?;

    let validated_explicit_files = validate_explicit_files(repo_root, &parsed.explicit_files)?;
    validate_search_areas(repo_root, &parsed.search_areas)?;

    let output = GatherOutput {
        schema_version: SCHEMA_VERSION.to_string(),
        repo_root: repo_root.to_string_lossy().into_owned(),
        summary: parsed.summary,
        explicit_files: validated_explicit_files.explicit_files,
        missing_explicit_files: validated_explicit_files.missing_explicit_files,
        skipped_explicit_files: validated_explicit_files.skipped_explicit_files,
        search_areas: parsed.search_areas,
        query: parsed.query,
        terms: parsed.terms,
        blockers: Vec::new(),
    };

    let stdout = io::stdout();
    let mut handle = stdout.lock();
    serde_json::to_writer(&mut handle, &output)?;
    handle.write_all(b"\n")?;
    Ok(())
}

struct ParsedTask {
    summary: String,
    explicit_files: Vec<String>,
    search_areas: Vec<String>,
    query: Vec<String>,
    terms: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
struct ValidatedExplicitFiles {
    explicit_files: Vec<ExplicitFile>,
    missing_explicit_files: Vec<String>,
    skipped_explicit_files: Vec<String>,
}

fn parse_task(text: &str) -> Result<ParsedTask, Box<dyn Error>> {
    let mut sections: HashMap<String, Vec<String>> = HashMap::new();
    let mut current: Option<String> = None;

    for raw_line in text.lines() {
        let line = raw_line.trim_end();
        let trimmed = line.trim();

        if trimmed == "# Task" {
            current = None;
            continue;
        }

        if let Some(section) = heading_name(trimmed) {
            if !is_allowed_section(&section) {
                return Err(io::Error::new(io::ErrorKind::InvalidData, format!("unexpected section: {section}")).into());
            }
            if sections.contains_key(&section) {
                return Err(io::Error::new(io::ErrorKind::InvalidData, format!("duplicate section: {section}")).into());
            }
            sections.insert(section.clone(), Vec::new());
            current = Some(section);
            continue;
        }

        if trimmed.is_empty() {
            continue;
        }

        let section = current.as_ref().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "content found before any section"))?;
        sections.get_mut(section).expect("section exists").push(line.to_string());
    }

    let summary = take_text_section(&sections, "Summary")?;
    let explicit_files = take_list_section(&sections, "Explicit Files")?;
    let search_areas = take_list_section(&sections, "Search Areas")?;
    let query = take_list_section(&sections, "Query")?;
    if query.is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Query section is required and must not be empty").into());
    }
    let terms = take_optional_list_section(&sections, "Terms");

    Ok(ParsedTask {
        summary,
        explicit_files,
        search_areas,
        query,
        terms,
    })
}

fn heading_name(line: &str) -> Option<String> {
    let without_hashes = line.strip_prefix("##")?;
    let name = without_hashes.trim().to_string();
    if name.is_empty() {
        return None;
    }
    Some(name)
}

fn is_allowed_section(section: &str) -> bool {
    matches!(section, "Summary" | "Explicit Files" | "Search Areas" | "Query" | "Terms")
}

fn take_text_section(sections: &HashMap<String, Vec<String>>, name: &str) -> Result<String, Box<dyn Error>> {
    let lines = sections.get(name).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, format!("missing required section: {name}")))?;
    let text = lines.join("\n").trim().to_string();
    if text.is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidData, format!("section is empty: {name}")).into());
    }
    Ok(text)
}

fn take_list_section(sections: &HashMap<String, Vec<String>>, name: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let lines = sections.get(name).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, format!("missing required section: {name}")))?;
    Ok(parse_items(lines))
}

fn take_optional_list_section(sections: &HashMap<String, Vec<String>>, name: &str) -> Vec<String> {
    sections
        .get(name)
        .map(|lines| parse_items(lines))
        .unwrap_or_default()
}

fn parse_items(lines: &[String]) -> Vec<String> {
    lines.iter().filter_map(|line| parse_item(line)).collect()
}

fn parse_item(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    let item = trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
        .or_else(|| trimmed.strip_prefix("+ "))
        .or_else(|| strip_numbered_prefix(trimmed))
        .unwrap_or(trimmed)
        .trim();

    if item.is_empty() {
        None
    } else {
        Some(item.to_string())
    }
}

fn strip_numbered_prefix(value: &str) -> Option<&str> {
    let mut digits = 0;
    for ch in value.chars() {
        if ch.is_ascii_digit() {
            digits += 1;
        } else {
            break;
        }
    }

    if digits == 0 {
        return None;
    }

    let rest = &value[digits..];
    let rest = rest.strip_prefix('.').or_else(|| rest.strip_prefix(')'))?;
    Some(rest.trim_start())
}

fn validate_explicit_files(repo_root: &Path, files: &[String]) -> Result<ValidatedExplicitFiles, Box<dyn Error>> {
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
            return Err(io::Error::new(io::ErrorKind::InvalidInput, format!("explicit path is not a file: {path}")).into());
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

fn validate_search_areas(repo_root: &Path, search_areas: &[String]) -> Result<(), Box<dyn Error>> {
    for area in search_areas {
        let resolved = source::resolve_path(repo_root, area);
        if source::is_skipped_path(repo_root, &resolved) || !resolved.exists() {
            return Err(io::Error::new(io::ErrorKind::NotFound, format!("search area not found: {area}")).into());
        }
    }
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::{parse_task, validate_explicit_files, ValidatedExplicitFiles};
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

    #[test]
    fn parses_structured_task_query_and_terms() {
        let task = r#"
# Task

## Summary
investigate attachment points

## Explicit Files

## Search Areas
- src/

## Query
- Where should Tree-sitter attach?
- What still needs verification?

## Terms
- tree-sitter
- attach
"#;

        let parsed = parse_task(task).expect("task parses");
        assert_eq!(parsed.summary, "investigate attachment points");
        assert_eq!(parsed.search_areas, vec!["src/"]);
        assert_eq!(parsed.query, vec!["Where should Tree-sitter attach?", "What still needs verification?"]);
        assert_eq!(parsed.terms, vec!["tree-sitter", "attach"]);
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
                    assert!(err.to_string().contains(expected_fragment), "case: {}", case.name);
                }
            }

            let _ = fs::remove_dir_all(repo);
        }
    }
}
