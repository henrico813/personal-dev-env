use crate::schema::{Answer, Finding, GatherOutput, ResearchOutput, TraceOutput};
use std::collections::BTreeSet;
use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub fn run(context: &Path, trace_out: &Path) -> Result<(), Box<dyn Error>> {
    let context_text = fs::read_to_string(context)?;
    let gather: GatherOutput = serde_json::from_str(&context_text)?;

    let repo_root = Path::new(&gather.repo_root).to_path_buf();
    let mut trace = TraceState::default();
    let mut answers = Vec::with_capacity(gather.questions.len());

    for question in &gather.questions {
        let (findings, negative_evidence) = answer_question(&repo_root, question, &gather.search_areas, &mut trace)?;
        if findings.is_empty() {
            trace.unmatched_questions.push(question.clone());
        }
        answers.push(Answer {
            question: question.clone(),
            findings,
            negative_evidence,
        });
    }

    let open_questions = trace.unmatched_questions.clone();
    let report = ResearchOutput {
        schema_version: gather.schema_version.clone(),
        summary: gather.summary,
        answers,
        blockers: gather.blockers,
        open_questions,
    };

    let trace_output = TraceOutput {
        schema_version: gather.schema_version,
        searched_areas: gather.search_areas,
        skipped_paths: trace.skipped_paths,
        files_considered: trace.files_considered.len(),
        files_matched: trace.files_matched.len(),
        unmatched_questions: trace.unmatched_questions,
    };

    if let Some(parent) = trace_out.parent() {
        fs::create_dir_all(parent)?;
    }
    let trace_file = fs::File::create(trace_out)?;
    serde_json::to_writer(trace_file, &trace_output)?;

    let stdout = io::stdout();
    let mut handle = stdout.lock();
    serde_json::to_writer(&mut handle, &report)?;
    handle.write_all(b"\n")?;
    Ok(())
}

#[derive(Default)]
struct TraceState {
    files_considered: BTreeSet<PathBuf>,
    files_matched: BTreeSet<PathBuf>,
    skipped_paths: Vec<String>,
    unmatched_questions: Vec<String>,
}

fn answer_question(
    repo_root: &Path,
    question: &str,
    search_areas: &[String],
    trace: &mut TraceState,
) -> Result<(Vec<Finding>, Vec<String>), Box<dyn Error>> {
    let tokens = question_tokens(question);
    let mut findings = Vec::new();

    for area in search_areas {
        let area_path = resolve_path(repo_root, area);
        for file in collect_files(&area_path, trace)? {
            trace.files_considered.insert(file.clone());
            let text = match fs::read_to_string(&file) {
                Ok(text) => text,
                Err(_) => {
                    trace.skipped_paths.push(display_path(repo_root, &file));
                    continue;
                }
            };

            let mut file_matched = false;
            for (index, line) in text.lines().enumerate() {
                let lower_line = line.to_lowercase();
                if let Some(matched_from) = tokens.iter().find(|token| lower_line.contains(token.as_str())) {
                    file_matched = true;
                    findings.push(Finding {
                        path: display_path(repo_root, &file),
                        line: (index + 1) as u32,
                        excerpt: line.trim().to_string(),
                        source: "lexical".to_string(),
                        matched_from: matched_from.clone(),
                    });
                }
            }

            if file_matched {
                trace.files_matched.insert(file);
            }
        }
    }

    let negative_evidence = if findings.is_empty() {
        vec![format!("searched declared areas: {}", search_areas.join(", "))]
    } else {
        Vec::new()
    };

    Ok((findings, negative_evidence))
}

fn question_tokens(question: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    for raw in question.split(|ch: char| !ch.is_ascii_alphanumeric()) {
        let token = raw.trim().to_lowercase();
        if token.len() < 3 {
            continue;
        }
        if !tokens.contains(&token) {
            tokens.push(token);
        }
    }
    tokens
}

fn collect_files(dir: &Path, trace: &mut TraceState) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    if dir.is_file() {
        return Ok(vec![dir.to_path_buf()]);
    }

    if !dir.is_dir() {
        trace.skipped_paths.push(dir.to_string_lossy().into_owned());
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    let read_dir = match fs::read_dir(dir) {
        Ok(read_dir) => read_dir,
        Err(_) => {
            trace.skipped_paths.push(dir.to_string_lossy().into_owned());
            return Ok(Vec::new());
        }
    };
    for entry in read_dir {
        match entry {
            Ok(entry) => entries.push(entry.path()),
            Err(_) => trace.skipped_paths.push(dir.to_string_lossy().into_owned()),
        }
    }
    entries.sort();

    let mut files = Vec::new();
    for path in entries {
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(_) => {
                trace.skipped_paths.push(path.to_string_lossy().into_owned());
                continue;
            }
        };
        if metadata.is_dir() {
            files.extend(collect_files(&path, trace)?);
        } else if metadata.is_file() {
            files.push(path);
        }
    }
    Ok(files)
}

fn resolve_path(repo_root: &Path, raw: &str) -> PathBuf {
    let path = Path::new(raw);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo_root.join(path)
    }
}

fn display_path(repo_root: &Path, path: &Path) -> String {
    path.strip_prefix(repo_root)
        .map(|relative| relative.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string_lossy().into_owned())
}
