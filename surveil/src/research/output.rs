use super::{rank, scan, setup, tokenize, LiveFileCache, TraceState};
use crate::schema::{Answer, GatherOutput, ResearchOutput, TraceOutput, SCHEMA_VERSION};
use std::collections::HashSet;
use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

pub(crate) fn run(context: &Path, trace_out: &Path) -> Result<(), Box<dyn Error>> {
    let context_text = fs::read_to_string(context)?;
    let gather: GatherOutput = serde_json::from_str(&context_text)?;
    let (report, trace_output) = execute_gather(gather)?;

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

fn execute_gather(gather: GatherOutput) -> Result<(ResearchOutput, TraceOutput), Box<dyn Error>> {
    let repo_root = Path::new(&gather.repo_root).to_path_buf();
    let mut trace = TraceState::default();
    let candidates = setup::collect_candidate_sources(
        &repo_root,
        &gather.search_areas,
        &gather.explicit_files,
        &mut trace,
    )?;
    let mut live_cache = LiveFileCache::new();
    let mut result = Vec::with_capacity(gather.query.len());

    for query in &gather.query {
        let tokens = tokenize::search_tokens(&gather.terms, query);
        let (ranked_scores, ordered_candidates) =
            rank::rank_query_candidates(&repo_root, &candidates, &tokens)?;
        let (findings, negative_evidence) = scan::answer_question_from_sources(
            &repo_root,
            &gather.search_areas,
            &ordered_candidates,
            &candidates,
            &tokens,
            &ranked_scores,
            &mut live_cache,
            &mut trace,
        )?;
        if findings.is_empty() {
            trace.unmatched_questions.push(query.clone());
        }
        result.push(Answer {
            query: query.clone(),
            findings,
            negative_evidence,
        });
    }

    let open_questions = trace.unmatched_questions.clone();
    let report = ResearchOutput {
        schema_version: SCHEMA_VERSION.to_string(),
        summary: gather.summary,
        result,
        blockers: gather.blockers,
        open_questions,
    };

    dedupe_in_place(&mut trace.skipped_paths);
    let trace_output = TraceOutput {
        schema_version: SCHEMA_VERSION.to_string(),
        searched_areas: gather.search_areas,
        skipped_paths: trace.skipped_paths,
        files_considered: trace.files_considered.len(),
        files_matched: trace.files_matched.len(),
        unmatched_questions: trace.unmatched_questions,
    };

    Ok((report, trace_output))
}

fn dedupe_in_place(values: &mut Vec<String>) {
    let mut seen = HashSet::new();
    values.retain(|value| seen.insert(value.clone()));
}

#[cfg(test)]
pub(super) fn answer_question_for_test(
    repo_root: &Path,
    question: &str,
    terms: &[String],
    search_areas: &[String],
    explicit_files: &[crate::schema::ExplicitFile],
    trace: &mut TraceState,
) -> Result<(Vec<crate::schema::Finding>, Vec<String>), Box<dyn Error>> {
    let candidates = setup::collect_candidate_sources(repo_root, search_areas, explicit_files, trace)?;
    let mut live_cache = LiveFileCache::new();
    let tokens = tokenize::search_tokens(terms, question);
    let (ranked_scores, ordered_candidates) =
        rank::rank_query_candidates(repo_root, &candidates, &tokens)?;
    scan::answer_question_from_sources(
        repo_root,
        search_areas,
        &ordered_candidates,
        &candidates,
        &tokens,
        &ranked_scores,
        &mut live_cache,
        trace,
    )
}

#[cfg(test)]
pub(super) fn run_for_test(
    gather: GatherOutput,
) -> Result<(ResearchOutput, TraceOutput), Box<dyn Error>> {
    execute_gather(gather)
}
