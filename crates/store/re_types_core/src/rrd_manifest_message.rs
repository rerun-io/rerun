#![expect(clippy::mem_forget)] // because of ouroboros

use arrow::array::{Array as _, BooleanArray, FixedSizeBinaryArray, RecordBatch, StringArray};
use arrow::error::ArrowError;
use itertools::izip;
use re_arrow_util::{ArrowArrayDowncastRef as _, RecordBatchExt as _, WrongDatatypeError};

use crate::ChunkId;

pub const FIELD_CHUNK_ID: &str = "chunk_id";
pub const FIELD_CHUNK_IS_STATIC: &str = "chunk_is_static";
pub const FIELD_CHUNK_ENTITY_PATH: &str = "chunk_entity_path";
pub const FIELD_CHUNK_BYTE_OFFSET: &str = "chunk_byte_offset";
pub const FIELD_CHUNK_BYTE_SIZE: &str = "chunk_byte_size";
pub const FIELD_CHUNK_KEY: &str = "chunk_key";

// -----------------------------------------------------------------------------------------

#[derive(thiserror::Error, Debug)]
pub enum ChunkIndexError {
    #[error(transparent)]
    Arrow(#[from] ArrowError),

    #[error(transparent)]
    MissingColumn(#[from] re_arrow_util::MissingColumnError),

    #[error(transparent)]
    WrongDatatype(#[from] WrongDatatypeError),

    #[error("Found nulls in column {column_name:?}")]
    UnexpectedNulls { column_name: String },
}

// -----------------------------------------------------------------------------------------

/// Communicates the chunks in a store (recording) without actually holding the chunks.
///
/// This is sent from the server to the client/viewer.
///
///
/// ## Example (transposed)
/// See schema in `crates/store/re_log_encoding/tests/snapshots/footers_and_manifests__rrd_manifest_blueprint_schema.snap`
///
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
        Self::try_from_record_batch(self.borrow_rb().clone()).unwrap()
    }
}

impl RrdManifestMessage {
    pub fn try_from_record_batch(rb: RecordBatch) -> Result<Self, ChunkIndexError> {
        #![expect(clippy::unwrap_used)] // We validate before running the builder

        let chunk_entity_path = rb
            .try_get_column(FIELD_CHUNK_ENTITY_PATH)?
            .try_downcast_array_ref::<StringArray>()?
            .clone();

        chunk_id_from_rb(&rb)?; // validate
        let chunk_is_static = chunk_is_static_from_rb(&rb)?;
        if chunk_is_static.null_count() != 0 {
            return Err(ChunkIndexError::UnexpectedNulls {
                column_name: FIELD_CHUNK_IS_STATIC.into(),
            });
        }

        // TODO(emilk): parse all the other columns
        for (field, column) in izip!(rb.schema().fields(), rb.columns()) {
            let is_special_field = matches!(
                field.name().as_str(),
                FIELD_CHUNK_ENTITY_PATH
                    | FIELD_CHUNK_ID
                    | FIELD_CHUNK_IS_STATIC
                    | FIELD_CHUNK_BYTE_OFFSET
                    | FIELD_CHUNK_BYTE_SIZE
                    | FIELD_CHUNK_KEY
            );
            if !is_special_field {
                _ = column; // TODO(emilk): parse and store the time ranges for all the components
            }
        }

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
        .try_get_column("chunk_id")?
        .try_downcast_array_ref::<FixedSizeBinaryArray>()?;

    Ok(ChunkId::try_slice_from_arrow(chunk_ids)?)
}

fn chunk_is_static_from_rb(rb: &RecordBatch) -> Result<BooleanArray, ChunkIndexError> {
    Ok(rb
        .try_get_column(FIELD_CHUNK_IS_STATIC)?
        .try_downcast_array_ref::<BooleanArray>()?
        .clone())
}
