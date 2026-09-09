use re_chunk::{ChunkError, ChunkId, ComponentIdentifier};
use re_log_encoding::{ChunkProviderError, CodecError};
use re_log_types::EntityPath;

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

    #[error("Malformed per-component chunk index column {column:?}: {reason}")]
    MalformedComponentColumn {
        column: String,
        reason: &'static str,
    },

    #[error(
        "Chunk index references chunk {chunk_id}, but no such chunk row exists\nEntity: {entity_path}"
    )]
    UnknownChunkId {
        chunk_id: ChunkId,
        entity_path: EntityPath,
    },

    #[error(
        "Failed to load {num_chunks} chunk(s) from the provider: {source}\nEntity: {entity_path}"
    )]
    LoadChunks {
        entity_path: EntityPath,
        num_chunks: usize,
        source: ChunkProviderError,
    },

    #[error("The provider did not return chunk {chunk_id}\nEntity: {entity_path}")]
    MissingChunk {
        chunk_id: ChunkId,
        entity_path: EntityPath,
    },

    #[error("Failed to merge chunks: {source}\nEntity: {entity_path}")]
    MergeChunks {
        entity_path: EntityPath,
        source: ChunkError,
    },

    /// The decoded chunk lacks columns the chunk index records for it, or a selection built from
    /// the index keeps none of its columns. An unrecorded extra column goes undetected.
    #[error(
        "Columns of chunk {chunk_id} do not match the index.\nEntity: {entity_path}\nMissing columns: {missing:?}"
    )]
    IndexMismatch {
        chunk_id: ChunkId,
        entity_path: EntityPath,
        missing: Vec<ComponentIdentifier>,
    },

    /// The plan's selection drops every column of the chunk, which the planner never emits for a
    /// chunk that matches the index.
    #[error("Column selection keeps no column of chunk {chunk_id}\nEntity: {entity_path}")]
    EmptySelection {
        chunk_id: ChunkId,
        entity_path: EntityPath,
    },
}

impl Error {
    pub fn read_column(column: &'static str, source: CodecError) -> Self {
        Self::ReadColumn { column, source }
    }

    pub fn temporal_map(source: CodecError) -> Self {
        Self::TemporalMap { source }
    }

    pub fn malformed_component_column(column: &str, reason: &'static str) -> Self {
        Self::MalformedComponentColumn {
            column: column.to_owned(),
            reason,
        }
    }

    pub fn unknown_chunk_id(chunk_id: ChunkId, entity_path: &EntityPath) -> Self {
        Self::UnknownChunkId {
            chunk_id,
            entity_path: entity_path.clone(),
        }
    }

    pub fn load_chunks(
        entity_path: &EntityPath,
        num_chunks: usize,
        source: ChunkProviderError,
    ) -> Self {
        Self::LoadChunks {
            entity_path: entity_path.clone(),
            num_chunks,
            source,
        }
    }

    pub fn missing_chunk(chunk_id: ChunkId, entity_path: &EntityPath) -> Self {
        Self::MissingChunk {
            chunk_id,
            entity_path: entity_path.clone(),
        }
    }

    pub fn merge_chunks(entity_path: &EntityPath, source: ChunkError) -> Self {
        Self::MergeChunks {
            entity_path: entity_path.clone(),
            source,
        }
    }

    pub fn index_mismatch(
        chunk_id: ChunkId,
        entity_path: &EntityPath,
        missing: Vec<ComponentIdentifier>,
    ) -> Self {
        Self::IndexMismatch {
            chunk_id,
            entity_path: entity_path.clone(),
            missing,
        }
    }

    pub fn empty_selection(chunk_id: ChunkId, entity_path: &EntityPath) -> Self {
        Self::EmptySelection {
            chunk_id,
            entity_path: entity_path.clone(),
        }
    }
}
