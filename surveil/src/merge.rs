#[cfg(test)]
mod tests;

use crate::schema::{
    Answer, EvidenceFinding, EvidencePack, EvidenceReport, Finding, FindingOccurrence,
    QueryEvidenceNote, ReportEvidenceNote, ResearchOutput, EVIDENCE_SCHEMA_VERSION, SCHEMA_VERSION,
};
use crate::taskfile::validate_task_name;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

/// One task-report path parsed from a positional command argument.
#[derive(Debug, Clone, PartialEq, Eq)]
struct TaskReport {
    path: PathBuf,
}

/// A task report that passed version, shape, and task-name validation.
#[derive(Debug, Clone, PartialEq, Eq)]
struct LoadedTaskReport {
    report: ResearchOutput,
}

#[derive(Deserialize)]
struct VersionEnvelope {
    schema_version: String,
}

/// The exact path, line, and excerpt identity used for finding lookup.
#[derive(Debug, PartialEq, Eq, Hash)]
struct FindingKey {
    path: String,
    line: u32,
    excerpt: String,
}

#[derive(Debug)]
enum MergeError {
    InvalidArguments(String),
    ReadReport {
        path: PathBuf,
        source: io::Error,
    },
    ParseReport {
        path: PathBuf,
        source: serde_json::Error,
    },
    UnsupportedVersion {
        path: PathBuf,
        expected: &'static str,
        actual: String,
    },
}

impl fmt::Display for MergeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArguments(message) => formatter.write_str(message),
            Self::ReadReport { path, source } => {
                write!(
                    formatter,
                    "failed to read report {}: {source}",
                    path.display()
                )
            }
            Self::ParseReport { path, source } => {
                write!(
                    formatter,
                    "failed to parse report {}: {source}",
                    path.display()
                )
            }
            Self::UnsupportedVersion {
                path,
                expected,
                actual,
            } => write!(
                formatter,
                "unsupported schema version in {}: expected {expected}, got {actual}",
                path.display()
            ),
        }
    }
}

impl Error for MergeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ReadReport { source, .. } => Some(source),
            Self::ParseReport { source, .. } => Some(source),
            Self::InvalidArguments(_) | Self::UnsupportedVersion { .. } => None,
        }
    }
}

pub(crate) fn run(paths: &[PathBuf]) -> Result<(), Box<dyn Error>> {
    let task_reports = parse_task_reports(paths);
    let loaded_task_reports = load_task_reports(task_reports)?;
    let pack = merge_task_reports(loaded_task_reports);
    write_evidence_pack(&pack)?;
    Ok(())
}

/// Parses ordered positional task-report paths.
fn parse_task_reports(paths: &[PathBuf]) -> Vec<TaskReport> {
    paths
        .iter()
        .cloned()
        .map(|path| TaskReport { path })
        .collect()
}

/// Reads reports in argument order and validates version before strict shape.
fn load_task_reports(inputs: Vec<TaskReport>) -> Result<Vec<LoadedTaskReport>, MergeError> {
    let mut task_names = HashSet::new();
    let mut reports = Vec::with_capacity(inputs.len());
    for input in inputs {
        let text = fs::read_to_string(&input.path).map_err(|source| MergeError::ReadReport {
            path: input.path.clone(),
            source,
        })?;
        let envelope: VersionEnvelope =
            serde_json::from_str(&text).map_err(|source| MergeError::ParseReport {
                path: input.path.clone(),
                source,
            })?;
        if envelope.schema_version != SCHEMA_VERSION {
            return Err(MergeError::UnsupportedVersion {
                path: input.path,
                expected: SCHEMA_VERSION,
                actual: envelope.schema_version,
            });
        }
        let report: ResearchOutput =
            serde_json::from_str(&text).map_err(|source| MergeError::ParseReport {
                path: input.path.clone(),
                source,
            })?;
        validate_task_name(&report.task_name).map_err(|source| {
            MergeError::InvalidArguments(format!(
                "invalid task_name in {}: {source}",
                input.path.display()
            ))
        })?;
        if !task_names.insert(report.task_name.clone()) {
            return Err(MergeError::InvalidArguments(format!(
                "duplicate task name: {}",
                report.task_name
            )));
        }
        reports.push(LoadedTaskReport { report });
    }
    Ok(reports)
}

/// Merges reports while retaining first-seen order for every output list.
fn merge_task_reports(reports: Vec<LoadedTaskReport>) -> EvidencePack {
    let mut report_metadata = Vec::new();
    let mut findings: Vec<EvidenceFinding> = Vec::new();
    let mut finding_indexes = HashMap::new();
    let mut negative_evidence = Vec::new();
    let mut blockers = Vec::new();
    let mut open_questions = Vec::new();

    for LoadedTaskReport { report } in reports {
        let ResearchOutput {
            task_name,
            summary,
            result,
            blockers: report_blockers,
            open_questions: report_questions,
            ..
        } = report;
        report_metadata.push(EvidenceReport {
            task_name: task_name.clone(),
            summary,
        });
        for Answer {
            query,
            findings: answer_findings,
            negative_evidence: answer_notes,
        } in result
        {
            for (index, finding) in answer_findings.into_iter().enumerate() {
                let rank = index as u64 + 1;
                add_finding(
                    &mut findings,
                    &mut finding_indexes,
                    &task_name,
                    &query,
                    rank,
                    finding,
                );
            }
            for text in answer_notes {
                push_unique(
                    &mut negative_evidence,
                    QueryEvidenceNote {
                        task_name: task_name.clone(),
                        query: query.clone(),
                        text,
                    },
                );
            }
        }
        for text in report_blockers {
            push_unique(
                &mut blockers,
                ReportEvidenceNote {
                    task_name: task_name.clone(),
                    text,
                },
            );
        }
        for text in report_questions {
            push_unique(
                &mut open_questions,
                ReportEvidenceNote {
                    task_name: task_name.clone(),
                    text,
                },
            );
        }
    }

    EvidencePack {
        schema_version: EVIDENCE_SCHEMA_VERSION.to_string(),
        reports: report_metadata,
        findings,
        negative_evidence,
        blockers,
        open_questions,
    }
}

/// Writes compact JSON followed by one newline.
fn write_evidence_pack(pack: &EvidencePack) -> Result<(), Box<dyn Error>> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    serde_json::to_writer(&mut handle, pack)?;
    handle.write_all(b"\n")?;
    Ok(())
}

fn add_finding(
    findings: &mut Vec<EvidenceFinding>,
    indexes: &mut HashMap<FindingKey, usize>,
    task_name: &str,
    query: &str,
    rank: u64,
    finding: Finding,
) {
    let Finding {
        path,
        line,
        excerpt,
        source,
        matched_from,
        symbol_kind,
        symbol_name,
        symbol_start_line,
        symbol_end_line,
    } = finding;
    let key = FindingKey {
        path: path.clone(),
        line,
        excerpt: excerpt.clone(),
    };
    let occurrence = FindingOccurrence {
        task_name: task_name.to_string(),
        query: query.to_string(),
        rank,
        source,
        matched_from,
        symbol_kind,
        symbol_name,
        symbol_start_line,
        symbol_end_line,
    };
    if let Some(index) = indexes.get(&key).copied() {
        push_unique(&mut findings[index].occurrences, occurrence);
        return;
    }
    indexes.insert(key, findings.len());
    findings.push(EvidenceFinding {
        path,
        line,
        excerpt,
        occurrences: vec![occurrence],
    });
}

fn push_unique<T: PartialEq>(items: &mut Vec<T>, item: T) {
    if !items.contains(&item) {
        items.push(item);
    }
}
