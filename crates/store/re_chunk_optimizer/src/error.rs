use re_chunk::{ChunkError, ChunkId};
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
}

impl Error {
    pub fn read_column(column: &'static str, source: CodecError) -> Self {
        Self::ReadColumn { column, source }
    }

    pub fn temporal_map(source: CodecError) -> Self {
        Self::TemporalMap { source }
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
}
