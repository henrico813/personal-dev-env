#[cfg(test)]
mod tests;

use crate::schema::{
    Answer, EvidenceFinding, EvidencePack, EvidenceReport, Finding, FindingOccurrence,
    QueryEvidenceNote, ReportEvidenceNote, ResearchOutput, EVIDENCE_SCHEMA_VERSION, SCHEMA_VERSION,
};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

/// A labeled report path parsed from one command argument.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ReportInput {
    perspective: String,
    path: PathBuf,
}

/// A labeled report that passed version and shape validation.
#[derive(Debug, Clone, PartialEq, Eq)]
struct LoadedReport {
    perspective: String,
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

pub(crate) fn run(values: &[String]) -> Result<(), Box<dyn Error>> {
    let inputs = parse_report_args(values)?;
    let reports = load_reports(inputs)?;
    let pack = merge_reports(reports);
    write_evidence_pack(&pack)?;
    Ok(())
}

/// Parses ordered `<perspective>=<path>` arguments and rejects duplicate labels.
fn parse_report_args(values: &[String]) -> Result<Vec<ReportInput>, MergeError> {
    let mut perspectives = HashSet::new();
    let mut inputs = Vec::with_capacity(values.len());
    for value in values {
        let Some((perspective, path)) = value.split_once('=') else {
            return Err(MergeError::InvalidArguments(format!(
                "invalid report argument {value:?}; expected <perspective>=<path>"
            )));
        };
        let perspective = perspective.trim();
        // A filesystem path may contain spaces, so preserve it exactly.
        if perspective.is_empty() || path.is_empty() {
            return Err(MergeError::InvalidArguments(format!(
                "invalid report argument {value:?}; perspective and path are required"
            )));
        }
        if !perspectives.insert(perspective.to_string()) {
            return Err(MergeError::InvalidArguments(format!(
                "duplicate report perspective: {perspective}"
            )));
        }
        inputs.push(ReportInput {
            perspective: perspective.to_string(),
            path: PathBuf::from(path),
        });
    }
    Ok(inputs)
}

/// Reads reports in argument order and validates version before strict shape.
fn load_reports(inputs: Vec<ReportInput>) -> Result<Vec<LoadedReport>, MergeError> {
    let mut reports = Vec::with_capacity(inputs.len());
    for input in inputs {
        let text = fs::read_to_string(&input.path).map_err(|source| MergeError::ReadReport {
            path: input.path.clone(),
            source,
        })?;
        // Check the version envelope first so old formats get a stable error.
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
        let report = serde_json::from_str(&text).map_err(|source| MergeError::ParseReport {
            path: input.path.clone(),
            source,
        })?;
        reports.push(LoadedReport {
            perspective: input.perspective,
            report,
        });
    }
    Ok(reports)
}

/// Merges reports while retaining first-seen order for every output list.
fn merge_reports(reports: Vec<LoadedReport>) -> EvidencePack {
    let mut report_metadata = Vec::new();
    let mut findings: Vec<EvidenceFinding> = Vec::new();
    // The map is lookup only; the vector controls deterministic output order.
    let mut finding_indexes = HashMap::new();
    let mut negative_evidence = Vec::new();
    let mut blockers = Vec::new();
    let mut open_questions = Vec::new();

    for LoadedReport {
        perspective,
        report,
    } in reports
    {
        let ResearchOutput {
            summary,
            result,
            blockers: report_blockers,
            open_questions: report_questions,
            ..
        } = report;
        report_metadata.push(EvidenceReport {
            perspective: perspective.clone(),
            summary,
        });
        for Answer {
            query,
            findings: answer_findings,
            negative_evidence: answer_notes,
        } in result
        {
            for (index, finding) in answer_findings.into_iter().enumerate() {
                // enumerate is zero-based; evidence ranks are one-based.
                let rank = index as u64 + 1;
                add_finding(
                    &mut findings,
                    &mut finding_indexes,
                    &perspective,
                    &query,
                    rank,
                    finding,
                );
            }
            for text in answer_notes {
                push_unique(
                    &mut negative_evidence,
                    QueryEvidenceNote {
                        perspective: perspective.clone(),
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
                    perspective: perspective.clone(),
                    text,
                },
            );
        }
        for text in report_questions {
            push_unique(
                &mut open_questions,
                ReportEvidenceNote {
                    perspective: perspective.clone(),
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
    perspective: &str,
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
        perspective: perspective.to_string(),
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
