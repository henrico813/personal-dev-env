use super::super::{merge_reports, LoadedReport};
use super::{finding, report};
use crate::schema::{
    EvidenceFinding, EvidencePack, EvidenceReport, FindingOccurrence, QueryEvidenceNote,
    ReportEvidenceNote, EVIDENCE_SCHEMA_VERSION,
};

#[test]
fn merges_complete_ordered_pack() {
    let actual = merge_reports(vec![
        LoadedReport {
            perspective: "architecture".to_string(),
            report: report("Where?", vec![finding("lexical")]),
        },
        LoadedReport {
            perspective: "tests".to_string(),
            report: report("How?", vec![finding("explicit_file")]),
        },
    ]);
    let occurrence = |perspective: &str, query: &str, source: &str| FindingOccurrence {
        perspective: perspective.to_string(),
        query: query.to_string(),
        rank: 1,
        source: source.to_string(),
        matched_from: "merge".to_string(),
        symbol_kind: Some("function".to_string()),
        symbol_name: Some("run".to_string()),
        symbol_start_line: Some(75),
        symbol_end_line: Some(86),
    };
    assert_eq!(
        actual,
        EvidencePack {
            schema_version: EVIDENCE_SCHEMA_VERSION.to_string(),
            reports: vec![
                EvidenceReport {
                    perspective: "architecture".to_string(),
                    summary: "merge command".to_string(),
                },
                EvidenceReport {
                    perspective: "tests".to_string(),
                    summary: "merge command".to_string(),
                },
            ],
            findings: vec![EvidenceFinding {
                path: "surveil/src/cli.rs".to_string(),
                line: 84,
                excerpt: "Command::Merge(args) => merge::run(&args.reports),".to_string(),
                occurrences: vec![
                    occurrence("architecture", "Where?", "lexical"),
                    occurrence("tests", "How?", "explicit_file"),
                ],
            }],
            negative_evidence: vec![
                QueryEvidenceNote {
                    perspective: "architecture".to_string(),
                    query: "Where?".to_string(),
                    text: "no existing route".to_string(),
                },
                QueryEvidenceNote {
                    perspective: "tests".to_string(),
                    query: "How?".to_string(),
                    text: "no existing route".to_string(),
                },
            ],
            blockers: vec![
                ReportEvidenceNote {
                    perspective: "architecture".to_string(),
                    text: "strict input required".to_string(),
                },
                ReportEvidenceNote {
                    perspective: "tests".to_string(),
                    text: "strict input required".to_string(),
                },
            ],
            open_questions: vec![
                ReportEvidenceNote {
                    perspective: "architecture".to_string(),
                    text: "which caller adopts this?".to_string(),
                },
                ReportEvidenceNote {
                    perspective: "tests".to_string(),
                    text: "which caller adopts this?".to_string(),
                },
            ],
        }
    );
}

#[test]
fn keeps_different_finding_excerpts() {
    let first = finding("lexical");
    let mut second = first.clone();
    second.excerpt = "mod merge;".to_string();
    let pack = merge_reports(vec![LoadedReport {
        perspective: "architecture".to_string(),
        report: report("Where?", vec![first, second]),
    }]);
    assert_eq!(
        pack.findings
            .iter()
            .map(|finding| (finding.line, finding.excerpt.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (84, "Command::Merge(args) => merge::run(&args.reports),"),
            (84, "mod merge;"),
        ]
    );
    assert_eq!(pack.findings[0].occurrences[0].rank, 1);
    assert_eq!(pack.findings[1].occurrences[0].rank, 2);
}

#[test]
fn suppresses_exact_duplicates() {
    let mut duplicate = report("Where?", vec![finding("lexical")]);
    let answer = duplicate.result[0].clone();
    let blocker = duplicate.blockers[0].clone();
    let question = duplicate.open_questions[0].clone();
    duplicate.result.push(answer);
    duplicate.blockers.push(blocker);
    duplicate.open_questions.push(question);
    let pack = merge_reports(vec![LoadedReport {
        perspective: "architecture".to_string(),
        report: duplicate,
    }]);
    assert_eq!(
        pack.findings[0].occurrences,
        vec![FindingOccurrence {
            perspective: "architecture".to_string(),
            query: "Where?".to_string(),
            rank: 1,
            source: "lexical".to_string(),
            matched_from: "merge".to_string(),
            symbol_kind: Some("function".to_string()),
            symbol_name: Some("run".to_string()),
            symbol_start_line: Some(75),
            symbol_end_line: Some(86),
        }]
    );
    assert_eq!(
        pack.negative_evidence,
        vec![QueryEvidenceNote {
            perspective: "architecture".to_string(),
            query: "Where?".to_string(),
            text: "no existing route".to_string(),
        }]
    );
    assert_eq!(
        pack.blockers,
        vec![ReportEvidenceNote {
            perspective: "architecture".to_string(),
            text: "strict input required".to_string(),
        }]
    );
    assert_eq!(
        pack.open_questions,
        vec![ReportEvidenceNote {
            perspective: "architecture".to_string(),
            text: "which caller adopts this?".to_string(),
        }]
    );
}
