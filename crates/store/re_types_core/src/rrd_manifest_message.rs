#![expect(clippy::mem_forget)] // because of ouroboros

use arrow::array::{Array as _, AsArray as _, BooleanArray, RecordBatch, StringArray};
use arrow::error::ArrowError;
use re_arrow_util::{ArrowArrayDowncastRef as _, RecordBatchExt as _, WrongDatatypeError};

use crate::ChunkId;

// -----------------------------------------------------------------------------------------

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
    Arrow(#[from] ArrowError),

    #[error(transparent)]
    MissingColumn(#[from] re_arrow_util::MissingColumnError),

    #[error(transparent)]
    WrongDatatype(#[from] WrongDatatypeError),
}

// -----------------------------------------------------------------------------------------

/// Communicates the chunks in a store (recording) without actually holding the chunks.
///
/// This is sent from the server to the client/viewer.
///
///
/// ## Example (transposed)
/// ```
/// ┌─────────────────────────────────────────┬──────────────────────────────────────────┬──────────────────────────────────────────┐
/// │ chunk_entity_path                       ┆ /my/entity                               ┆ /my/entity                               │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ chunk_id                                ┆ 00000000000000010000000000000001         ┆ 00000000000000010000000000000002         │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ chunk_is_static                         ┆ false                                    ┆ true                                     │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ example_MyPoints:colors:has_static_data ┆ false                                    ┆ false                                    │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ example_MyPoints:labels:has_static_data ┆ false                                    ┆ true                                     │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ example_MyPoints:points:has_static_data ┆ false                                    ┆ false                                    │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ frame_nr:start                          ┆ 10                                       ┆ null                                     │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ frame_nr:end                            ┆ 40                                       ┆ null                                     │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ frame_nr:example_MyPoints:colors:start  ┆ 10                                       ┆ null                                     │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ frame_nr:example_MyPoints:colors:end    ┆ 40                                       ┆ null                                     │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ frame_nr:example_MyPoints:points:start  ┆ 10                                       ┆ null                                     │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ frame_nr:example_MyPoints:points:end    ┆ 40                                       ┆ null                                     │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ chunk_key                               ┆ 010000000000000001000000000000000a00000… ┆ 010000000000000002000000000000000a00000… │
/// └─────────────────────────────────────────┴──────────────────────────────────────────┴──────────────────────────────────────────┘
/// ```
#[ouroboros::self_referencing]
pub struct RrdManifestMessage {
    rb: RecordBatch,

    chunk_entity_path: StringArray,

    #[borrows(rb)]
    chunk_id: &'this [ChunkId],

    chunk_is_static: BooleanArray,
}

impl std::fmt::Debug for RrdManifestMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChunkIndexMessage")
            .field("num_chunks", &self.num_rows())
            .finish()
    }
}

impl Clone for RrdManifestMessage {
    fn clone(&self) -> Self {
        #![expect(clippy::unwrap_used)] // `self` is existence proof that this cannot fail
        Self::from_record_batch(self.borrow_rb().clone()).unwrap()
    }
}

impl RrdManifestMessage {
    pub fn from_record_batch(rb: RecordBatch) -> Result<Self, ChunkIndexError> {
        #![expect(clippy::unwrap_used)] // We validate before running the builder

        let chunk_entity_path = rb
            .try_get_column("chunk_entity_path")?
            .try_downcast_array_ref::<StringArray>()?
            .clone();

        chunk_id_from_rb(&rb)?; // validate
        let chunk_is_static = chunk_is_static_from_rb(&rb)?;
        if chunk_is_static.null_count() != 0 {
            return Err(ChunkIndexError::NullsInChunkIsStatic);
        }

        // TODO(emilk): parse all the other columns

        Ok(RrdManifestMessageBuilder {
            rb,
            chunk_entity_path,
            chunk_id_builder: |rb: &RecordBatch| chunk_id_from_rb(rb).unwrap(),
            chunk_is_static,
        }
        .build())
    }

    pub fn record_batch(&self) -> &RecordBatch {
        self.borrow_rb()
    }

    pub fn num_rows(&self) -> usize {
        self.borrow_rb().num_rows()
    }

    pub fn chunk_entity_path(&self) -> &StringArray {
        self.borrow_chunk_entity_path()
    }

    /// All the chunks in this index
    pub fn chunk_id(&self) -> &[ChunkId] {
        self.borrow_chunk_id()
    }

    /// Is a given chunk static (as opposed to temporal)?
    pub fn chunk_is_static(&self) -> impl Iterator<Item = bool> {
        self.borrow_chunk_is_static()
            .iter()
            .map(|b| b.unwrap_or_default()) // we've validated that there are no nulls
    }
}

fn chunk_id_from_rb(rb: &RecordBatch) -> Result<&[ChunkId], ChunkIndexError> {
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
