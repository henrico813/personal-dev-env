use crate::source::SourceFile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChunkKind {
    CodeSymbol,
    CodeWindow,
    MarkdownBlock,
    ConfigStanza,
    GenericWindow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Chunk {
    pub(crate) source: SourceFile,
    pub(crate) kind: ChunkKind,
    pub(crate) start_line: u32,
    pub(crate) end_line: u32,
    pub(crate) text: String,
    pub(crate) language: Option<String>,
    pub(crate) symbol_name: Option<String>,
    pub(crate) section_path: Vec<String>,
    pub(crate) key_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RankedChunk {
    pub(crate) chunk: Chunk,
    pub(crate) score: f32,
}
