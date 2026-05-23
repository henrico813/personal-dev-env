use crate::chunk::{build_chunks, Chunk, ChunkKind};
use crate::source::{self, SourceFile};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::error::Error;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::time::UNIX_EPOCH;
use tantivy::Index;

pub const INDEX_DIR: &str = ".surveil/index";
const BUILD_INFO_PATH: &str = ".surveil/index/build-info.json";
const INDEX_FORMAT_VERSION: u32 = 1;
const SCHEMA_VERSION: u32 = 1;
const CHUNK_LAYOUT_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IndexState {
    Usable,
    Missing,
    Stale,
    Incompatible,
    Corrupt,
}

#[derive(Debug, Serialize, Deserialize)]
struct IndexBuildInfo {
    format_version: u32,
    schema_version: u32,
    chunk_layout_version: u32,
    repo_fingerprint: u64,
}

pub fn build_chunk_index(repo_root: &Path) -> Result<(), Box<dyn Error>> {
    build_index_directory(repo_root)?;
    let schema = schema::build_tantivy_schema();
    let fields = schema::IndexFields::from_schema(&schema);
    let mut writer = build_chunk_index_writer(repo_root, schema)?;
    build_repo_chunk_documents(repo_root, &fields, &mut writer)?;
    writer.commit()?;
    writer.wait_merging_threads()?;
    build_index_metadata(repo_root)?;
    Ok(())
}

pub(crate) fn inspect_chunk_index(repo_root: &Path) -> Result<IndexState, Box<dyn Error>> {
    if !repo_root.join(INDEX_DIR).is_dir() {
        return Ok(IndexState::Missing);
    }

    let build_info = match read_index_metadata(repo_root) {
        Ok(build_info) => build_info,
        Err(_) => return Ok(IndexState::Corrupt),
    };

    let compatibility = check_index_compatibility(&build_info, repo_root)?;
    if compatibility != IndexState::Usable {
        return Ok(compatibility);
    }

    match open_existing_chunk_index(repo_root) {
        Ok(_) => Ok(IndexState::Usable),
        Err(_) => Ok(IndexState::Corrupt),
    }
}

fn build_index_directory(repo_root: &Path) -> Result<(), Box<dyn Error>> {
    let index_dir = repo_root.join(INDEX_DIR);
    if index_dir.exists() {
        fs::remove_dir_all(&index_dir)?;
    }
    fs::create_dir_all(&index_dir)?;
    Ok(())
}

fn build_chunk_index_writer(
    repo_root: &Path,
    schema: tantivy::schema::Schema,
) -> Result<tantivy::IndexWriter, Box<dyn Error>> {
    let index = Index::create_in_dir(repo_root.join(INDEX_DIR), schema)?;
    Ok(index.writer(50_000_000)?)
}

fn build_repo_chunk_documents(
    repo_root: &Path,
    fields: &schema::IndexFields,
    writer: &mut tantivy::IndexWriter,
) -> Result<(), Box<dyn Error>> {
    let mut _skipped_paths = Vec::new();
    let search_areas = [".".to_string()];
    let candidates =
        source::collect_candidate_files(repo_root, &search_areas, &[], &mut _skipped_paths)?;

    for source in candidates {
        let text = match fs::read_to_string(source.path()) {
            Ok(text) => text,
            Err(_) => continue,
        };

        for chunk in build_chunks(&source, &text) {
            let document = encode::encode_chunk_document(fields, &source, &chunk);
            writer.add_document(document)?;
        }
    }

    Ok(())
}

fn build_index_metadata(repo_root: &Path) -> Result<(), Box<dyn Error>> {
    let build_info = IndexBuildInfo {
        format_version: INDEX_FORMAT_VERSION,
        schema_version: SCHEMA_VERSION,
        chunk_layout_version: CHUNK_LAYOUT_VERSION,
        repo_fingerprint: compute_repo_fingerprint(repo_root)?,
    };
    fs::write(
        repo_root.join(BUILD_INFO_PATH),
        serde_json::to_vec_pretty(&build_info)?,
    )?;
    Ok(())
}

fn read_index_metadata(repo_root: &Path) -> Result<IndexBuildInfo, Box<dyn Error>> {
    let text = fs::read_to_string(repo_root.join(BUILD_INFO_PATH))?;
    Ok(serde_json::from_str(&text)?)
}

fn compute_repo_fingerprint(repo_root: &Path) -> Result<u64, Box<dyn Error>> {
    let mut _skipped_paths = Vec::new();
    let search_areas = [".".to_string()];
    let candidates =
        source::collect_candidate_files(repo_root, &search_areas, &[], &mut _skipped_paths)?;
    let mut hasher = DefaultHasher::new();

    for source in candidates {
        source.display_path().hash(&mut hasher);
        if let Ok(metadata) = fs::metadata(source.path()) {
            metadata.len().hash(&mut hasher);
            if let Ok(modified) = metadata.modified() {
                if let Ok(duration) = modified.duration_since(UNIX_EPOCH) {
                    duration.as_nanos().hash(&mut hasher);
                }
            }
        }
        if let Ok(text) = fs::read_to_string(source.path()) {
            text.hash(&mut hasher);
        }
    }

    Ok(hasher.finish())
}

fn check_index_compatibility(
    build_info: &IndexBuildInfo,
    repo_root: &Path,
) -> Result<IndexState, Box<dyn Error>> {
    if build_info.format_version != INDEX_FORMAT_VERSION
        || build_info.schema_version != SCHEMA_VERSION
        || build_info.chunk_layout_version != CHUNK_LAYOUT_VERSION
    {
        return Ok(IndexState::Incompatible);
    }

    if build_info.repo_fingerprint != compute_repo_fingerprint(repo_root)? {
        return Ok(IndexState::Stale);
    }

    Ok(IndexState::Usable)
}

fn open_existing_chunk_index(repo_root: &Path) -> Result<Index, Box<dyn Error>> {
    Ok(Index::open_in_dir(repo_root.join(INDEX_DIR))?)
}

fn chunk_kind_name(kind: ChunkKind) -> &'static str {
    match kind {
        ChunkKind::CodeSymbol => "code_symbol",
        ChunkKind::CodeFallbackWindow => "code_fallback_window",
        ChunkKind::MarkdownBlock => "markdown_block",
        ChunkKind::ConfigStanza => "config_stanza",
        ChunkKind::GenericFallbackWindow => "generic_fallback_window",
    }
}

mod schema {
    use tantivy::schema::{NumericOptions, Schema, STORED, STRING, TEXT};

    pub(super) struct IndexFields {
        pub(super) path: tantivy::schema::Field,
        pub(super) kind: tantivy::schema::Field,
        pub(super) language: tantivy::schema::Field,
        pub(super) symbol_name: tantivy::schema::Field,
        pub(super) section_path: tantivy::schema::Field,
        pub(super) key_path: tantivy::schema::Field,
        pub(super) start_line: tantivy::schema::Field,
        pub(super) end_line: tantivy::schema::Field,
        pub(super) text: tantivy::schema::Field,
    }

    pub(super) fn build_tantivy_schema() -> Schema {
        let mut builder = Schema::builder();
        let numeric = NumericOptions::default().set_indexed().set_stored();

        builder.add_text_field("path", STRING | STORED);
        builder.add_text_field("kind", STRING | STORED);
        builder.add_text_field("language", STRING | STORED);
        builder.add_text_field("symbol_name", STRING | STORED);
        builder.add_text_field("section_path", TEXT | STORED);
        builder.add_text_field("key_path", TEXT | STORED);
        builder.add_u64_field("start_line", numeric);
        builder.add_u64_field("end_line", numeric);
        builder.add_text_field("text", TEXT | STORED);
        builder.build()
    }

    impl IndexFields {
        pub(super) fn from_schema(schema: &Schema) -> Self {
            Self {
                path: schema.get_field("path").expect("missing path field"),
                kind: schema.get_field("kind").expect("missing kind field"),
                language: schema.get_field("language").expect("missing language field"),
                symbol_name: schema.get_field("symbol_name").expect("missing symbol_name field"),
                section_path: schema.get_field("section_path").expect("missing section_path field"),
                key_path: schema.get_field("key_path").expect("missing key_path field"),
                start_line: schema.get_field("start_line").expect("missing start_line field"),
                end_line: schema.get_field("end_line").expect("missing end_line field"),
                text: schema.get_field("text").expect("missing text field"),
            }
        }
    }
}

mod encode {
    use super::{chunk_kind_name, schema};
    use crate::chunk::Chunk;
    use crate::source::SourceFile;
    use tantivy::doc;

    pub(super) fn encode_chunk_document(
        fields: &schema::IndexFields,
        source: &SourceFile,
        chunk: &Chunk,
    ) -> tantivy::Document {
        let mut document = doc!(
            fields.path => source.display_path().to_string(),
            fields.kind => chunk_kind_name(chunk.kind),
            fields.start_line => chunk.start_line as u64,
            fields.end_line => chunk.end_line as u64,
            fields.text => chunk.text.clone(),
        );

        if let Some(language) = chunk.language.as_deref() {
            document.add_text(fields.language, language);
        }
        if let Some(symbol_name) = chunk.symbol_name.as_deref() {
            document.add_text(fields.symbol_name, symbol_name);
        }
        if !chunk.section_path.is_empty() {
            document.add_text(fields.section_path, chunk.section_path.join(" / "));
        }
        if let Some(key_path) = chunk.key_path.as_deref() {
            document.add_text(fields.key_path, key_path);
        }

        document
    }
}

#[cfg(test)]
mod tests {
    use super::{build_chunk_index, inspect_chunk_index, IndexState, BUILD_INFO_PATH, INDEX_DIR};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_repo(name: &str) -> PathBuf {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("surveil-index-{name}-{stamp}"));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn write_note(repo: &PathBuf, path: &str, content: &str) {
        let file_path = repo.join(path);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(file_path, content).unwrap();
    }

    fn overwrite_build_info(repo: &PathBuf, contents: &str) {
        fs::write(repo.join(BUILD_INFO_PATH), contents).unwrap();
    }

    #[test]
    fn builds_chunk_index_and_inspects_as_usable() {
        let repo = temp_repo("builds");
        write_note(&repo, "notes/design.md", "attach index here\n");

        build_chunk_index(&repo).unwrap();

        assert!(repo.join(INDEX_DIR).is_dir());
        assert_eq!(inspect_chunk_index(&repo).unwrap(), IndexState::Usable);

        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn reports_missing_when_index_directory_is_absent() {
        let repo = temp_repo("missing");

        assert_eq!(inspect_chunk_index(&repo).unwrap(), IndexState::Missing);

        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn reports_incompatible_for_wrong_metadata_version() {
        let repo = temp_repo("incompatible");
        write_note(&repo, "notes/design.md", "attach index here\n");
        build_chunk_index(&repo).unwrap();
        overwrite_build_info(
            &repo,
            &serde_json::json!({
                "format_version": 999,
                "schema_version": 1,
                "chunk_layout_version": 1,
                "repo_fingerprint": 0
            })
            .to_string(),
        );

        assert_eq!(inspect_chunk_index(&repo).unwrap(), IndexState::Incompatible);

        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn reports_stale_when_repository_changes() {
        let repo = temp_repo("stale");
        write_note(&repo, "notes/design.md", "old text\n");
        build_chunk_index(&repo).unwrap();
        write_note(&repo, "notes/design.md", "new text\n");

        assert_eq!(inspect_chunk_index(&repo).unwrap(), IndexState::Stale);

        let _ = fs::remove_dir_all(repo);
    }
}
