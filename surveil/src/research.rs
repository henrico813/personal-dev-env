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
        let (findings, negative_evidence) = answer_question(
            &repo_root,
            question,
            &gather.terms,
            &gather.search_areas,
            &mut trace,
        )?;
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
    terms: &[String],
    search_areas: &[String],
    trace: &mut TraceState,
) -> Result<(Vec<Finding>, Vec<String>), Box<dyn Error>> {
    let tokens = search_tokens(terms, question);
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

fn search_tokens(terms: &[String], question: &str) -> Vec<String> {
    let mut tokens = Vec::new();

    for term in terms {
        let token = term.trim().to_lowercase();
        push_token(&mut tokens, &token);
        if token.contains('-') {
            push_token(&mut tokens, &token.replace('-', "_"));
        } else if token.contains('_') {
            push_token(&mut tokens, &token.replace('_', "-"));
        }
    }

    for raw in question.split_whitespace() {
        let token = raw
            .trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '-' && ch != '_')
            .to_lowercase();
        if token.is_empty() || is_generic_question_token(&token) {
            continue;
        }
        push_token(&mut tokens, &token);
        if token.contains('-') {
            push_token(&mut tokens, &token.replace('-', "_"));
        } else if token.contains('_') {
            push_token(&mut tokens, &token.replace('_', "-"));
        }
    }

    tokens
}

fn push_token(tokens: &mut Vec<String>, token: &str) {
    if token.len() < 3 {
        return;
    }
    if !tokens.iter().any(|existing| existing == token) {
        tokens.push(token.to_string());
    }
}

fn is_generic_question_token(token: &str) -> bool {
    matches!(
        token,
        "what" | "where" | "when" | "why" | "how" | "who" | "whom" | "which" | "whose"
            | "should" | "would" | "could" | "can" | "may" | "might" | "do" | "does"
            | "did" | "is" | "are" | "was" | "were" | "be" | "been" | "being" | "the"
            | "a" | "an" | "to" | "of" | "and" | "or" | "for" | "in" | "on" | "at"
            | "by" | "with" | "from" | "into" | "this" | "that" | "these" | "those"
    )
}

fn collect_files(dir: &Path, trace: &mut TraceState) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    if is_skipped_path(dir) {
        trace.skipped_paths.push(dir.to_string_lossy().into_owned());
        return Ok(Vec::new());
    }

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
        if is_skipped_path(&path) {
            trace.skipped_paths.push(path.to_string_lossy().into_owned());
            continue;
        }
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

fn is_skipped_path(path: &Path) -> bool {
    path.components().any(|component| {
        matches!(
            component,
            std::path::Component::Normal(name)
                if matches!(
                    name.to_string_lossy().as_ref(),
                    "target" | "node_modules" | "dist" | "build" | "pack" | "worktrees" | ".git"
                )
        )
    })
}

fn display_path(repo_root: &Path, path: &Path) -> String {
    path.strip_prefix(repo_root)
        .map(|relative| relative.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::{answer_question, TraceState};
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
    fn skips_generated_output_and_prefers_declared_terms() {
        let repo = temp_repo("research");
        write_file(&repo.join("src/lib.rs"), "// tree-sitter attach\n");
        write_file(&repo.join("target/generated.rs"), "// tree-sitter attach\n");

        let mut trace = TraceState::default();
        let (findings, _) = answer_question(
            &repo,
            "Where should Tree-sitter attach?",
            &["tree-sitter".to_string()],
            &[".".to_string()],
            &mut trace,
        )
        .expect("research answer");

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].path, "src/lib.rs");
        assert_eq!(findings[0].matched_from, "tree-sitter");
        assert!(trace.skipped_paths.iter().any(|path| path.contains("target")));

        let _ = fs::remove_dir_all(repo);
    }
}
