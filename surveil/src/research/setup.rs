use super::TraceState;
use crate::schema::ExplicitFile;
use crate::source::{self, SourceFile};
use std::error::Error;
use std::path::Path;

pub(super) fn collect_candidate_sources(
    repo_root: &Path,
    search_areas: &[String],
    explicit_files: &[ExplicitFile],
    trace: &mut TraceState,
) -> Result<Vec<SourceFile>, Box<dyn Error>> {
    let candidates = source::collect_candidate_files(
        repo_root,
        search_areas,
        explicit_files,
        &mut trace.skipped_paths,
    )?;
    for source in &candidates {
        trace.files_considered.insert(source.path().to_path_buf());
    }
    Ok(candidates)
}
