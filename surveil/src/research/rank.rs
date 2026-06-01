use super::RANKED_FILE_LIMIT;
use crate::chunk::SearchQuery;
use crate::index;
use crate::source::SourceFile;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::error::Error;
use std::path::{Path, PathBuf};

pub(super) struct RunRanker {
    open_index: Option<index::OpenChunkIndex>,
}

pub(super) fn build_run_ranker(repo_root: &Path) -> Result<RunRanker, Box<dyn Error>> {
    Ok(RunRanker {
        open_index: index::open_chunk_index_for_run(repo_root)?,
    })
}

impl RunRanker {
    pub(super) fn rank_query_candidates(
        &mut self,
        candidates: &[SourceFile],
        tokens: &[String],
    ) -> Result<(HashMap<PathBuf, f32>, Vec<SourceFile>), Box<dyn Error>> {
        let ranked_scores = {
            let Some(open_index) = self.open_index.as_ref() else {
                return Ok((HashMap::new(), candidates.to_vec()));
            };
            rank_candidate_files(open_index, candidates, tokens)
        };
        let ranked_scores = match ranked_scores {
            Ok(ranked_scores) => ranked_scores,
            Err(_) => {
                self.open_index = None;
                return Ok((HashMap::new(), candidates.to_vec()));
            }
        };

        let ordered_candidates = order_query_candidates(candidates, &ranked_scores);
        Ok((ranked_scores, ordered_candidates))
    }
}

pub(super) fn rank_query_candidates(
    repo_root: &Path,
    candidates: &[SourceFile],
    tokens: &[String],
) -> Result<(HashMap<PathBuf, f32>, Vec<SourceFile>), Box<dyn Error>> {
    let mut ranker = build_run_ranker(repo_root)?;
    ranker.rank_query_candidates(candidates, tokens)
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
    open_index: &index::OpenChunkIndex,
    candidates: &[SourceFile],
    tokens: &[String],
) -> Result<HashMap<PathBuf, f32>, Box<dyn Error>> {
    let ranked_chunks = index::search_open_chunk_index(
        open_index,
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

fn order_query_candidates(
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

#[cfg(test)]
mod tests {
    use super::rank_query_candidates;
    use crate::chunk::{temp_repo, write_file};
    use crate::index;
    use crate::schema::ExplicitFile;
    use crate::source;
    use std::fs;

    struct RankCase {
        name: &'static str,
        files: Vec<(&'static str, &'static str)>,
        build_index: bool,
        search_areas: Vec<&'static str>,
        explicit_files: Vec<&'static str>,
        terms: Vec<&'static str>,
        expected_order: Vec<&'static str>,
    }

    #[test]
    fn rank_case_tables() {
        let cases = vec![
            RankCase {
                name: "fallback-scope-order",
                files: vec![("src/a.rs", "attach\n"), ("src/b.rs", "attach\n")],
                build_index: false,
                search_areas: vec!["src/"],
                explicit_files: vec![],
                terms: vec!["attach"],
                expected_order: vec!["src/a.rs", "src/b.rs"],
            },
            RankCase {
                name: "explicit-file-stays-first",
                files: vec![
                    ("notes/design.md", "tree-sitter attach\n"),
                    ("src/lib.rs", "tree-sitter attach\n"),
                ],
                build_index: true,
                search_areas: vec!["src/"],
                explicit_files: vec!["notes/design.md"],
                terms: vec!["tree-sitter"],
                expected_order: vec!["notes/design.md", "src/lib.rs"],
            },
            RankCase {
                name: "ranked-tie-breaker-stays-deterministic",
                files: vec![("src/b.rs", "attach handler\n"), ("src/a.rs", "attach handler\n")],
                build_index: true,
                search_areas: vec!["src/"],
                explicit_files: vec![],
                terms: vec!["attach", "handler"],
                expected_order: vec!["src/a.rs", "src/b.rs"],
            },
            RankCase {
                name: "ranked-code-before-docs",
                files: vec![
                    (
                        "docs/guide.md",
                        "attach attach attach attach\nattach attach attach attach\n",
                    ),
                    (
                        "src/lib.rs",
                        "fn attach_handler() {\n    // attach handler\n}\n",
                    ),
                ],
                build_index: true,
                search_areas: vec!["src/", "docs/"],
                explicit_files: vec![],
                terms: vec!["attach", "handler"],
                expected_order: vec!["src/lib.rs", "docs/guide.md"],
            },
        ];

        for case in &cases {
            let repo = temp_repo(case.name);
            for (path, content) in &case.files {
                write_file(&repo.join(path), content);
            }
            if case.build_index {
                index::build_chunk_index(&repo).expect("build index");
            }

            let mut skipped_paths = Vec::new();
            let search_areas = case
                .search_areas
                .iter()
                .map(|item| item.to_string())
                .collect::<Vec<_>>();
            let explicit_files = case
                .explicit_files
                .iter()
                .map(|path| ExplicitFile {
                    path: path.to_string(),
                    found: true,
                })
                .collect::<Vec<_>>();
            let candidates = source::collect_candidate_files(
                &repo,
                &search_areas,
                &explicit_files,
                &mut skipped_paths,
            )
            .expect("collect candidates");

            let terms = case.terms.iter().map(|item| item.to_string()).collect::<Vec<_>>();
            let (_, ordered) = rank_query_candidates(&repo, &candidates, &terms).expect("rank candidates");

            assert_eq!(
                ordered.iter().map(|item| item.display_path()).collect::<Vec<_>>(),
                case.expected_order,
                "case: {}",
                case.name
            );

            let _ = fs::remove_dir_all(repo);
        }
    }
}
