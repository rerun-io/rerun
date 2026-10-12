use re_chunk::{ChunkError, ChunkId, ComponentIdentifier};
use re_chunk_index::{ChunkIndexError, ChunkProviderError};
use re_log_types::{EntityPath, TimelineName};
use re_sdk_types::components::VideoCodec;

use crate::plan::{codec_column, sample_column};

/// Errors of this crate.
///
/// Variants carry the chunk-index-level operation and object; the identity of the chunk index itself
/// (store id, file path) is the caller's to add — it owns the input.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to read chunk index column {column:?}: {source}")]
    ReadColumn {
        column: &'static str,
        source: ChunkIndexError,
    },

    #[error("Failed to compute the chunk index's temporal map: {source}")]
    TemporalMap { source: ChunkIndexError },

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

    /// The decoded chunk lacks a timeline the chunk index records for it.
    #[error(
        "Chunk {chunk_id} has no {timeline:?} timeline, but the index records one\nEntity: {entity_path}"
    )]
    MissingTimeline {
        chunk_id: ChunkId,
        entity_path: EntityPath,
        timeline: TimelineName,
    },

    /// The codec or sample column of a video stream cannot be GoP-batched.
    #[error(
        "Cannot GoP-batch the video stream: {source}\nEntity: {entity_path}\nChunk: {chunk_id}"
    )]
    VideoStream {
        entity_path: EntityPath,
        chunk_id: ChunkId,
        source: VideoStreamErrorSource,
    },

    /// A chunk the optimizer generates itself failed [`re_chunk::Chunk::sanity_check`].
    ///
    /// Its inputs are built to be valid, so this is a bug in the optimizer.
    #[error(
        "Bug in the chunk optimizer: a chunk it generated is malformed: {source}\nEntity: {entity_path}"
    )]
    MalformedGeneratedChunk {
        entity_path: EntityPath,
        source: ChunkError,
    },
}

impl Error {
    pub fn read_column(column: &'static str, source: ChunkIndexError) -> Self {
        Self::ReadColumn { column, source }
    }

    pub fn temporal_map(source: ChunkIndexError) -> Self {
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

    pub fn missing_timeline(
        chunk_id: ChunkId,
        entity_path: &EntityPath,
        timeline: TimelineName,
    ) -> Self {
        Self::MissingTimeline {
            chunk_id,
            entity_path: entity_path.clone(),
            timeline,
        }
    }

    pub fn video_stream(
        entity_path: &EntityPath,
        chunk_id: ChunkId,
        source: VideoStreamErrorSource,
    ) -> Self {
        Self::VideoStream {
            entity_path: entity_path.clone(),
            chunk_id,
            source,
        }
    }

    /// Panics in debug builds.
    pub fn malformed_generated_chunk(entity_path: &EntityPath, source: ChunkError) -> Self {
        re_log::debug_panic!("the chunk optimizer generated a malformed chunk: {source}");
        Self::MalformedGeneratedChunk {
            entity_path: entity_path.clone(),
            source,
        }
    }
}

/// Why [`Error::VideoStream`] cannot GoP-batch a video stream.
#[derive(thiserror::Error, Debug)]
pub enum VideoStreamErrorSource {
    /// No codec cell of the stream holds a value.
    #[error("the {} column holds no value", codec_column())]
    NoCodec,

    /// The stream's temporal codec cells disagree, and no static codec shadows them.
    #[error(
        "the {} column holds several values: {first:?} and {second:?}",
        codec_column()
    )]
    SeveralCodecs {
        first: VideoCodec,
        second: VideoCodec,
    },

    /// A codec cell stored next to the samples disagrees with the codec decided up front.
    #[error(
        "the {} column holds {found:?}, but the stream's codec is {expected:?}",
        codec_column()
    )]
    CodecMismatch {
        found: VideoCodec,
        expected: VideoCodec,
    },

    /// The deciding static codec cell cannot be deserialized.
    #[error("the {} column cannot be read: {source}", codec_column())]
    UnreadableCodec { source: ChunkError },

    /// The sample column has an Arrow datatype the keyframe detection cannot read samples from.
    #[error(
        "the {column} column has datatype {0:?}, which the sample reader does not handle",
        column = sample_column()
    )]
    UnsupportedSampleDatatype(arrow::datatypes::DataType),
}
