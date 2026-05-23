use super::RANKED_FILE_LIMIT;
use crate::chunk::SearchQuery;
use crate::index;
use crate::source::SourceFile;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::error::Error;
use std::path::{Path, PathBuf};

pub(super) fn rank_query_candidates(
    repo_root: &Path,
    candidates: &[SourceFile],
    tokens: &[String],
) -> Result<(HashMap<PathBuf, f32>, Vec<SourceFile>), Box<dyn Error>> {
    if index::inspect_chunk_index(repo_root)? != index::IndexState::Usable {
        return Ok((HashMap::new(), candidates.to_vec()));
    }

    let ranked_scores = rank_candidate_files(repo_root, candidates, tokens)?;
    let ordered_candidates = ordered_query_candidates(candidates, &ranked_scores);
    Ok((ranked_scores, ordered_candidates))
}

pub(super) fn compare_best_chunk_score(
    left: Option<f32>,
    right: Option<f32>,
) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => left.partial_cmp(&right).unwrap_or(Ordering::Equal),
        (Some(_), None) => Ordering::Greater,
        (None, Some(_)) => Ordering::Less,
        (None, None) => Ordering::Equal,
    }
}

fn rank_candidate_files(
    repo_root: &Path,
    candidates: &[SourceFile],
    tokens: &[String],
) -> Result<HashMap<PathBuf, f32>, Box<dyn Error>> {
    let ranked_chunks = index::search_chunk_index(
        repo_root,
        candidates,
        &SearchQuery {
            tokens: tokens.to_vec(),
            limit: RANKED_FILE_LIMIT,
        },
    )?;
    let mut ranked_scores = HashMap::new();

    for ranked in ranked_chunks {
        ranked_scores
            .entry(ranked.chunk.source.path().to_path_buf())
            .and_modify(|score| {
                if ranked.score > *score {
                    *score = ranked.score;
                }
            })
            .or_insert(ranked.score);
    }

    Ok(ranked_scores)
}

fn ordered_query_candidates(
    candidates: &[SourceFile],
    ranked_scores: &HashMap<PathBuf, f32>,
) -> Vec<SourceFile> {
    let mut ordered: Vec<SourceFile> = candidates
        .iter()
        .filter(|source| source.is_explicit())
        .cloned()
        .collect();
    let mut ranked: Vec<(SourceFile, f32)> = candidates
        .iter()
        .filter(|source| !source.is_explicit())
        .filter_map(|source| {
            ranked_scores
                .get(source.path())
                .copied()
                .map(|score| (source.clone(), score))
        })
        .collect();

    ranked.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.0.display_path().cmp(b.0.display_path()))
    });

    ordered.extend(ranked.into_iter().map(|(source, _)| source));
    ordered
}
