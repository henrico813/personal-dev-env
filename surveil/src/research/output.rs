use super::{rank, scan, setup, tokenize, LiveFileCache, SourceFile, TraceState};
use crate::index;
use crate::schema::{Answer, GatherOutput, QueryTrace, ResearchOutput, TraceOutput, SCHEMA_VERSION};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub(crate) fn write_research_output(context: &Path, trace_out: &Path) -> Result<(), Box<dyn Error>> {
    let context_text = fs::read_to_string(context)?;
    let gather: GatherOutput = serde_json::from_str(&context_text)?;
    let (report, trace_output) = create_research_outputs(gather)?;

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

fn create_research_outputs(gather: GatherOutput) -> Result<(ResearchOutput, TraceOutput), Box<dyn Error>> {
    let repo_root = Path::new(&gather.repo_root).to_path_buf();
    let mut trace = TraceState::default();
    let index_state = index::inspect_chunk_index(&repo_root)?;
    let ranking_usable = index_state == index::IndexState::Usable;
    let candidates = setup::collect_candidate_sources(
        &repo_root,
        &gather.search_areas,
        &gather.explicit_files,
        &mut trace,
    )?;
    let mut live_cache = LiveFileCache::new();
    let ranker = rank::build_run_ranker(&repo_root)?;
    let mut result = Vec::with_capacity(gather.query.len());
    let mut queries = Vec::with_capacity(gather.query.len());

    for query in &gather.query {
        let tokens = tokenize::create_search_tokens(&gather.terms, query);
        let (ranked_scores, ordered_candidates) =
            ranker.rank_query_candidates(&candidates, &tokens)?;
        let ranked_files = ranked_file_paths(&ordered_candidates, &ranked_scores, ranking_usable);
        let (findings, negative_evidence, fallback_used) = scan::create_answer_from_sources(
            &repo_root,
            &gather.search_areas,
            &ordered_candidates,
            &candidates,
            &tokens,
            &ranked_scores,
            ranking_usable,
            &mut live_cache,
            &mut trace,
        )?;
        queries.push(QueryTrace {
            query: query.clone(),
            retrieval_mode: retrieval_mode_name(ranking_usable, fallback_used).to_string(),
            ranked_files,
            findings_returned: findings.len(),
        });
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

    let mut missing_explicit_files = gather.missing_explicit_files;
    let mut skipped_explicit_files = gather.skipped_explicit_files;
    dedupe_values_in_place(&mut trace.skipped_paths);
    dedupe_values_in_place(&mut missing_explicit_files);
    dedupe_values_in_place(&mut skipped_explicit_files);
    let trace_output = TraceOutput {
        schema_version: SCHEMA_VERSION.to_string(),
        searched_areas: gather.search_areas,
        skipped_paths: trace.skipped_paths,
        files_considered: trace.files_considered.len(),
        files_matched: trace.files_matched.len(),
        missing_explicit_files,
        skipped_explicit_files,
        index_state: index_state_name(&index_state).to_string(),
        unmatched_questions: trace.unmatched_questions,
        queries,
    };

    Ok((report, trace_output))
}

fn dedupe_values_in_place(values: &mut Vec<String>) {
    let mut seen = HashSet::new();
    values.retain(|value| seen.insert(value.clone()));
}

fn index_state_name(state: &index::IndexState) -> &'static str {
    match state {
        index::IndexState::Usable => "usable",
        index::IndexState::Missing => "missing",
        index::IndexState::Stale => "stale",
        index::IndexState::Incompatible => "incompatible",
        index::IndexState::Corrupt => "corrupt",
    }
}

fn retrieval_mode_name(ranking_usable: bool, fallback_used: bool) -> &'static str {
    if !ranking_usable {
        "full_lexical_scan"
    } else if fallback_used {
        "ranked_then_fallback"
    } else {
        "ranked_only"
    }
}

fn ranked_file_paths(
    ordered_candidates: &[SourceFile],
    ranked_scores: &HashMap<PathBuf, f32>,
    ranking_usable: bool,
) -> Vec<String> {
    if !ranking_usable {
        return Vec::new();
    }

    ordered_candidates
        .iter()
        .filter(|source| !source.is_explicit())
        .filter(|source| ranked_scores.contains_key(source.path()))
        .map(|source| source.display_path().to_string())
        .collect()
}

#[cfg(test)]
pub(super) fn create_test_answer(
    repo_root: &Path,
    question: &str,
    terms: &[String],
    search_areas: &[String],
    explicit_files: &[crate::schema::ExplicitFile],
    trace: &mut TraceState,
) -> Result<(Vec<crate::schema::Finding>, Vec<String>), Box<dyn Error>> {
    let candidates = setup::collect_candidate_sources(repo_root, search_areas, explicit_files, trace)?;
    let mut live_cache = LiveFileCache::new();
    let ranker = rank::build_run_ranker(repo_root)?;
    let tokens = tokenize::create_search_tokens(terms, question);
    let index_state = index::inspect_chunk_index(repo_root)?;
    let ranking_usable = index_state == index::IndexState::Usable;
    let (ranked_scores, ordered_candidates) =
        ranker.rank_query_candidates(&candidates, &tokens)?;
    scan::create_answer_from_sources(
        repo_root,
        search_areas,
        &ordered_candidates,
        &candidates,
        &tokens,
        &ranked_scores,
        ranking_usable,
        &mut live_cache,
        trace,
    )
    .map(|(findings, negative_evidence, _)| (findings, negative_evidence))
}

#[cfg(test)]
pub(super) fn create_test_outputs(
    gather: GatherOutput,
) -> Result<(ResearchOutput, TraceOutput), Box<dyn Error>> {
    create_research_outputs(gather)
}
