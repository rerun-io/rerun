use re_log_encoding::CodecError;

/// Errors of this crate.
///
/// Variants carry the chunk-index-level operation and object; the identity of the chunk index itself
/// (store id, file path) is the caller's to add — it owns the input.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to read chunk index column {column:?}: {source}")]
    ReadColumn {
        column: &'static str,
        source: CodecError,
    },

    #[error("Failed to compute the chunk index's temporal map: {source}")]
    TemporalMap { source: CodecError },

    #[error(
        "chunk index references chunk {chunk_id} for entity {entity_path}, but no such chunk row exists"
    )]
    UnknownChunkId {
        chunk_id: re_chunk::ChunkId,
        entity_path: re_log_types::EntityPath,
    },
}

// helpers for .map_err
impl Error {
    pub(crate) fn read_column(column: &'static str) -> impl Fn(CodecError) -> Self {
        move |source| Self::ReadColumn { column, source }
    }

    pub(crate) fn temporal_map(source: CodecError) -> Self {
        Self::TemporalMap { source }
    }
}
