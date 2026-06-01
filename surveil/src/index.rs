use crate::chunk::{build_chunks, Chunk, ChunkKind, RankedChunk, SearchQuery};
use crate::source::{self, SourceFile};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::collections::hash_map::DefaultHasher;
use std::error::Error;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::time::UNIX_EPOCH;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::Value;
use tantivy::Index;

pub const INDEX_DIR: &str = ".surveil/index";
const BUILD_INFO_PATH: &str = ".surveil/index/build-info.json";
const INDEX_FORMAT_VERSION: u32 = 1;
const SCHEMA_VERSION: u32 = 1;
const CHUNK_LAYOUT_VERSION: u32 = 1;

pub(crate) struct OpenChunkIndex {
    index: Index,
    reader: tantivy::IndexReader,
}

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

pub(crate) fn open_chunk_index_for_run(
    repo_root: &Path,
) -> Result<Option<OpenChunkIndex>, Box<dyn Error>> {
    Ok(resolve_open_chunk_index(repo_root)?.1)
}


pub(crate) fn search_open_chunk_index(
    open_index: &OpenChunkIndex,
    scoped_files: &[SourceFile],
    query: &SearchQuery,
) -> Result<Vec<RankedChunk>, Box<dyn Error>> {
    if query.tokens.is_empty() || query.limit == 0 {
        return Ok(Vec::new());
    }

    let schema = open_index.index.schema();
    let fields = schema::IndexFields::from_schema(&schema);
    let source_by_path: HashMap<String, SourceFile> = scoped_files
        .iter()
        .cloned()
        .map(|source| (source.display_path().to_string(), source))
        .collect();
    let allowed_paths: HashSet<String> = source_by_path.keys().cloned().collect();

    let searcher = open_index.reader.searcher();
    let parser = QueryParser::for_index(
        &open_index.index,
        vec![
            fields.text,
            fields.symbol_name,
            fields.section_path_segment,
            fields.key_path_segment,
            fields.language,
        ],
    );
    let parsed = parser.parse_query(&query.tokens.join(" "))?;
    let overfetch_limit = query.limit.saturating_mul(8).max(query.limit);
    let top_docs = searcher.search(&parsed, &TopDocs::with_limit(overfetch_limit))?;

    let mut ranked = Vec::new();
    for (score, address) in top_docs {
        let document: tantivy::TantivyDocument = searcher.doc(address)?;
        let Some(path) = read_text_field(&document, fields.path) else {
            continue;
        };
        if !allowed_paths.contains(&path) {
            continue;
        }
        let Some(source) = source_by_path.get(&path).cloned() else {
            continue;
        };
        let Some(chunk) = decode_ranked_chunk(&document, &fields, source, score) else {
            continue;
        };
        ranked.push(chunk);
        if ranked.len() == query.limit {
            break;
        }
    }

    Ok(ranked)
}

#[cfg(test)]
pub(crate) fn inspect_chunk_index(repo_root: &Path) -> Result<IndexState, Box<dyn Error>> {
    Ok(resolve_open_chunk_index(repo_root)?.0)
}

fn resolve_open_chunk_index(
    repo_root: &Path,
) -> Result<(IndexState, Option<OpenChunkIndex>), Box<dyn Error>> {
    if !repo_root.join(INDEX_DIR).is_dir() {
        return Ok((IndexState::Missing, None));
    }

    let build_info = match read_index_metadata(repo_root) {
        Ok(build_info) => build_info,
        Err(_) => return Ok((IndexState::Corrupt, None)),
    };

    let compatibility = check_index_compatibility(&build_info, repo_root)?;
    if compatibility != IndexState::Usable {
        return Ok((compatibility, None));
    }

    let index = match open_existing_chunk_index(repo_root) {
        Ok(index) => index,
        Err(_) => return Ok((IndexState::Corrupt, None)),
    };
    let reader = match index.reader() {
        Ok(reader) => reader,
        Err(_) => return Ok((IndexState::Corrupt, None)),
    };

    Ok((IndexState::Usable, Some(OpenChunkIndex { index, reader })))
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
    let mut skipped_paths = Vec::new();
    let search_areas = [".".to_string()];
    let candidates =
        source::collect_candidate_files(repo_root, &search_areas, &[], &mut skipped_paths)?;

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
    let mut skipped_paths = Vec::new();
    let search_areas = [".".to_string()];
    let candidates =
        source::collect_candidate_files(repo_root, &search_areas, &[], &mut skipped_paths)?;
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

fn parse_chunk_kind(value: &str) -> Option<ChunkKind> {
    match value {
        "code_symbol" => Some(ChunkKind::CodeSymbol),
        "code_fallback_window" => Some(ChunkKind::CodeFallbackWindow),
        "markdown_block" => Some(ChunkKind::MarkdownBlock),
        "config_stanza" => Some(ChunkKind::ConfigStanza),
        "generic_fallback_window" => Some(ChunkKind::GenericFallbackWindow),
        _ => None,
    }
}

fn read_text_field(
    document: &tantivy::TantivyDocument,
    field: tantivy::schema::Field,
) -> Option<String> {
    document
        .get_first(field)
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

fn read_u64_field(
    document: &tantivy::TantivyDocument,
    field: tantivy::schema::Field,
) -> Option<u64> {
    document.get_first(field).and_then(|value| value.as_u64())
}

fn decode_ranked_chunk(
    document: &tantivy::TantivyDocument,
    fields: &schema::IndexFields,
    source: SourceFile,
    score: f32,
) -> Option<RankedChunk> {
    let kind = parse_chunk_kind(&read_text_field(document, fields.kind)?)?;
    let start_line = read_u64_field(document, fields.start_line)? as u32;
    let end_line = read_u64_field(document, fields.end_line)? as u32;
    let text = read_text_field(document, fields.text)?;
    let language = read_text_field(document, fields.language);
    let symbol_name = read_text_field(document, fields.symbol_name);
    let section_path = read_text_field(document, fields.section_path_full)
        .map(|path| path.split(" / ").map(str::to_string).collect())
        .unwrap_or_default();
    let key_path = read_text_field(document, fields.key_path_full);

    Some(RankedChunk {
        chunk: Chunk {
            source,
            kind,
            start_line,
            end_line,
            text,
            language,
            symbol_name,
            section_path,
            key_path,
        },
        score,
    })
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
        pub(super) start_line: tantivy::schema::Field,
        pub(super) end_line: tantivy::schema::Field,
        pub(super) language: tantivy::schema::Field,
        pub(super) symbol_name: tantivy::schema::Field,
        pub(super) section_path_full: tantivy::schema::Field,
        pub(super) section_path_segment: tantivy::schema::Field,
        pub(super) key_path_full: tantivy::schema::Field,
        pub(super) key_path_segment: tantivy::schema::Field,
        pub(super) text: tantivy::schema::Field,
    }

    pub(super) fn build_tantivy_schema() -> Schema {
        let mut builder = Schema::builder();
        let numeric = NumericOptions::default().set_indexed().set_stored();

        builder.add_text_field("path", STRING | STORED);
        builder.add_text_field("kind", STRING | STORED);
        builder.add_u64_field("start_line", numeric.clone());
        builder.add_u64_field("end_line", numeric);
        builder.add_text_field("language", STRING | STORED);
        builder.add_text_field("symbol_name", STRING | STORED);
        builder.add_text_field("section_path_full", STRING | STORED);
        builder.add_text_field("section_path_segment", TEXT | STORED);
        builder.add_text_field("key_path_full", STRING | STORED);
        builder.add_text_field("key_path_segment", TEXT | STORED);
        builder.add_text_field("text", TEXT | STORED);
        builder.build()
    }

    impl IndexFields {
        pub(super) fn from_schema(schema: &Schema) -> Self {
            Self {
                path: schema.get_field("path").expect("missing path field"),
                kind: schema.get_field("kind").expect("missing kind field"),
                start_line: schema.get_field("start_line").expect("missing start_line field"),
                end_line: schema.get_field("end_line").expect("missing end_line field"),
                language: schema.get_field("language").expect("missing language field"),
                symbol_name: schema.get_field("symbol_name").expect("missing symbol_name field"),
                section_path_full: schema
                    .get_field("section_path_full")
                    .expect("missing section_path_full field"),
                section_path_segment: schema
                    .get_field("section_path_segment")
                    .expect("missing section_path_segment field"),
                key_path_full: schema
                    .get_field("key_path_full")
                    .expect("missing key_path_full field"),
                key_path_segment: schema
                    .get_field("key_path_segment")
                    .expect("missing key_path_segment field"),
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
    ) -> tantivy::TantivyDocument {
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
            document.add_text(fields.section_path_full, chunk.section_path.join(" / "));
            for section in &chunk.section_path {
                document.add_text(fields.section_path_segment, section);
            }
        }
        if let Some(key_path) = chunk.key_path.as_deref() {
            document.add_text(fields.key_path_full, key_path);
            for segment in key_path.split('.').filter(|segment| !segment.is_empty()) {
                document.add_text(fields.key_path_segment, segment);
            }
        }

        document
    }
}

#[cfg(test)]
fn tantivy_meta_exists(repo_root: &Path) -> bool {
    repo_root.join(INDEX_DIR).join("meta.json").is_file()
}

#[cfg(test)]
fn build_info_exists(repo_root: &Path) -> bool {
    repo_root.join(BUILD_INFO_PATH).is_file()
}

#[cfg(test)]
fn overwrite_build_info(repo_root: &Path, contents: &str) {
    fs::write(repo_root.join(BUILD_INFO_PATH), contents).expect("write build info");
}

#[cfg(test)]
fn write_note(repo_root: &Path, relative: &str, text: &str) {
    let path = repo_root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent dirs");
    }
    fs::write(path, text).expect("write file");
}

#[cfg(test)]
fn temp_repo(name: &str) -> std::path::PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("surveil-index-{name}-{stamp}"));
    fs::create_dir_all(&path).expect("create temp repo");
    path
}

#[cfg(test)]
mod tests {
    use super::{
        build_chunk_index, build_info_exists, inspect_chunk_index, open_chunk_index_for_run,
        overwrite_build_info, search_open_chunk_index, tantivy_meta_exists, temp_repo,
        write_note, IndexState, BUILD_INFO_PATH, INDEX_DIR,
    };
    use crate::chunk::SearchQuery;
    use crate::source;
    use std::fs;
    use std::path::Path;
    use tantivy::Index;

    fn seed_repo(repo: &Path) {
        write_note(repo, "notes/design.md", "attach index here\n");
    }

    fn build_index(repo: &Path) {
        build_chunk_index(repo).expect("build index");
    }

    fn remove_build_info(repo: &Path) {
        fs::remove_file(repo.join(BUILD_INFO_PATH)).expect("remove build info");
    }

    fn rewrite_repo_file(repo: &Path) {
        write_note(repo, "notes/design.md", "attach changed text\n");
    }

    fn overwrite_incompatible_build_info(repo: &Path) {
        overwrite_build_info(
            repo,
            "{\n  \"format_version\": 1,\n  \"schema_version\": 99,\n  \"chunk_layout_version\": 1,\n  \"repo_fingerprint\": 1\n}\n",
        );
    }

    fn overwrite_invalid_build_info(repo: &Path) {
        overwrite_build_info(repo, "{not json\n");
    }

    fn remove_tantivy_meta(repo: &Path) {
        fs::remove_file(repo.join(INDEX_DIR).join("meta.json")).expect("remove tantivy meta");
    }

    #[test]
    fn state_transitions_match_expected_status() {
        struct Case {
            name: &'static str,
            setup: fn(&Path),
            mutate: Option<fn(&Path)>,
            expected: IndexState,
            expect_open_index: bool,
            expect_index_files: bool,
        }

        let cases = [
            Case {
                name: "missing_without_build",
                setup: seed_repo,
                mutate: None,
                expected: IndexState::Missing,
                expect_open_index: false,
                expect_index_files: false,
            },
            Case {
                name: "usable_after_build",
                setup: seed_repo,
                mutate: Some(build_index),
                expected: IndexState::Usable,
                expect_open_index: true,
                expect_index_files: true,
            },
            Case {
                name: "corrupt_when_build_info_missing",
                setup: seed_repo,
                mutate: Some(|repo| {
                    build_index(repo);
                    remove_build_info(repo);
                }),
                expected: IndexState::Corrupt,
                expect_open_index: false,
                expect_index_files: false,
            },
            Case {
                name: "stale_after_repo_change",
                setup: seed_repo,
                mutate: Some(|repo| {
                    build_index(repo);
                    rewrite_repo_file(repo);
                }),
                expected: IndexState::Stale,
                expect_open_index: false,
                expect_index_files: true,
            },
            Case {
                name: "incompatible_after_shape_change",
                setup: seed_repo,
                mutate: Some(|repo| {
                    build_index(repo);
                    overwrite_incompatible_build_info(repo);
                }),
                expected: IndexState::Incompatible,
                expect_open_index: false,
                expect_index_files: false,
            },
            Case {
                name: "corrupt_after_build_info_damage",
                setup: seed_repo,
                mutate: Some(|repo| {
                    build_index(repo);
                    overwrite_invalid_build_info(repo);
                }),
                expected: IndexState::Corrupt,
                expect_open_index: false,
                expect_index_files: false,
            },
            Case {
                name: "corrupt_after_tantivy_damage",
                setup: seed_repo,
                mutate: Some(|repo| {
                    build_index(repo);
                    remove_tantivy_meta(repo);
                }),
                expected: IndexState::Corrupt,
                expect_open_index: false,
                expect_index_files: false,
            },
        ];

        for case in cases {
            let repo = temp_repo(case.name);
            (case.setup)(&repo);
            if let Some(mutate) = case.mutate {
                mutate(&repo);
            }

            assert_eq!(
                inspect_chunk_index(&repo).expect("inspect chunk index"),
                case.expected,
                "case: {}",
                case.name,
            );
            assert_eq!(
                open_chunk_index_for_run(&repo)
                    .expect("open chunk index for run")
                    .is_some(),
                case.expect_open_index,
                "case: {}",
                case.name,
            );

            if case.expect_index_files {
                assert!(repo.join(INDEX_DIR).is_dir(), "case: {}", case.name);
                assert!(tantivy_meta_exists(&repo), "case: {}", case.name);
                assert!(build_info_exists(&repo), "case: {}", case.name);
            }

            let _ = fs::remove_dir_all(repo);
        }
    }

    #[test]
    fn chunk_index_contains_tantivy_documents() {
        let repo = temp_repo("documents");
        seed_repo(&repo);
        write_note(
            &repo,
            "src/lib.rs",
            "fn attach() {\n    // tree-sitter attach\n}\n",
        );

        build_chunk_index(&repo).expect("build chunk index");

        let index = Index::open_in_dir(repo.join(INDEX_DIR)).expect("open tantivy index");
        let reader = index.reader().expect("index reader");
        assert!(reader.searcher().num_docs() > 0);
        assert!(repo.join(BUILD_INFO_PATH).is_file());

        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn search_open_chunk_index_returns_scoped_ranked_chunks() {
        let repo = temp_repo("search-scope");
        write_note(&repo, "docs/guide.md", "attach attach attach attach\n");
        write_note(
            &repo,
            "src/lib.rs",
            "fn attach_handler() {\n    // attach handler\n}\n",
        );

        build_chunk_index(&repo).expect("build chunk index");

        let mut skipped_paths = Vec::new();
        let scoped = source::collect_candidate_files(
            &repo,
            &["src/".to_string()],
            &[],
            &mut skipped_paths,
        )
        .expect("collect scoped files");
        let open_index = open_chunk_index_for_run(&repo)
            .expect("open chunk index")
            .expect("usable index");
        let ranked = search_open_chunk_index(
            &open_index,
            &scoped,
            &SearchQuery {
                tokens: vec!["attach".to_string(), "handler".to_string()],
                limit: 4,
            },
        )
        .expect("search open chunk index");

        assert!(!ranked.is_empty());
        assert!(ranked
            .iter()
            .all(|item| item.chunk.source.display_path() == "src/lib.rs"));

        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn search_open_chunk_index_skips_empty_queries() {
        let repo = temp_repo("search-guards");
        write_note(&repo, "src/lib.rs", "fn attach_handler() {\n    // attach handler\n}\n");

        build_chunk_index(&repo).expect("build chunk index");

        let mut skipped_paths = Vec::new();
        let scoped = source::collect_candidate_files(
            &repo,
            &["src/".to_string()],
            &[],
            &mut skipped_paths,
        )
        .expect("collect scoped files");
        let open_index = open_chunk_index_for_run(&repo)
            .expect("open chunk index")
            .expect("usable index");

        assert!(search_open_chunk_index(
            &open_index,
            &scoped,
            &SearchQuery {
                tokens: Vec::new(),
                limit: 4,
            },
        )
        .expect("empty query")
        .is_empty());
        assert!(search_open_chunk_index(
            &open_index,
            &scoped,
            &SearchQuery {
                tokens: vec!["attach".to_string()],
                limit: 0,
            },
        )
        .expect("zero limit")
        .is_empty());

        let _ = fs::remove_dir_all(repo);
    }
}
