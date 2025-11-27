#![expect(clippy::mem_forget)] // because of ouroboros

use arrow::array::{Array as _, AsArray as _, BooleanArray, RecordBatch};

use crate::ChunkId;

#[derive(thiserror::Error, Debug)]
pub enum ChunkIndexError {
    #[error("Missing chunk_id column in ChunkIndex")]
    MissingChunkIdColumn,

    #[error("Expected ChunkId to be encoded as a FixedSizeBinary array")]
    InvalidChunkIdEncoding,

    #[error("Missing chunk_is_static column in ChunkIndex")]
    MissingChunkIsStaticColumn,

    #[error("Expected chunk_is_static to be encoded as a Boolean array")]
    InvalidChunkIsStaticEncoding,

    #[error("Found nulls in chunk_is_static column")]
    NullsInChunkIsStatic,

    #[error(transparent)]
    WrongDatatype(#[from] re_arrow_util::WrongDatatypeError),
}

/// Keeps track of all the chunks in a store (recording) without actually holding the chunks.
#[ouroboros::self_referencing]
pub struct ChunkIndex {
    rb: RecordBatch,

    #[borrows(rb)]
    chunk_ids: &'this [ChunkId],

    chunk_is_static: BooleanArray,
}

impl ChunkIndex {
    pub fn from_record_batch(rb: RecordBatch) -> Result<Self, ChunkIndexError> {
        #![expect(clippy::unwrap_used)] // We validate before running the builder

        chunk_ids_from_rb(&rb)?; // validate
        let chunk_is_static = chunk_is_static_from_rb(&rb)?;
        if chunk_is_static.null_count() != 0 {
            return Err(ChunkIndexError::NullsInChunkIsStatic);
        }

        Ok(ChunkIndexBuilder {
            rb,
            chunk_is_static,
            chunk_ids_builder: |rb: &RecordBatch| chunk_ids_from_rb(rb).unwrap(),
        }
        .build())
    }

    pub fn num_rows(&self) -> usize {
        self.borrow_rb().num_rows()
    }

    /// All the chunks in this index
    pub fn chunk_ids(&self) -> &[ChunkId] {
        self.borrow_chunk_ids()
    }

    /// Is a given chunk static (as opposed to temporal)?
    pub fn chunk_is_static(&self) -> impl Iterator<Item = bool> {
        self.borrow_chunk_is_static()
            .iter()
            .map(|b| b.unwrap_or_default()) // we've validated that there are no nulls
    }
}

fn chunk_ids_from_rb(rb: &RecordBatch) -> Result<&[ChunkId], ChunkIndexError> {
    let chunk_ids = rb
        .column_by_name("chunk_id")
        .ok_or(ChunkIndexError::MissingChunkIdColumn)?;

    let chunk_ids = chunk_ids
        .as_fixed_size_binary_opt()
        .ok_or(ChunkIndexError::InvalidChunkIdEncoding)?;

    Ok(ChunkId::try_slice_from_arrow(chunk_ids)?)
}

fn chunk_is_static_from_rb(rb: &RecordBatch) -> Result<BooleanArray, ChunkIndexError> {
    let chunk_is_static = rb
        .column_by_name("chunk_is_static")
        .ok_or(ChunkIndexError::MissingChunkIsStaticColumn)?;

    let chunk_is_static = chunk_is_static
        .as_boolean_opt()
        .ok_or(ChunkIndexError::InvalidChunkIsStaticEncoding)?;

    Ok(chunk_is_static.clone())
}
