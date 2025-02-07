use arrow::{
    array::{ArrayRef as ArrowArrayRef, RecordBatch as ArrowRecordBatch, RecordBatchOptions},
    datatypes::{Fields as ArrowFields, Schema as ArrowSchema},
};

use re_log_types::EntityPath;
use re_types_core::ChunkId;

use crate::{
    chunk_schema::InvalidChunkSchema, ArrowBatchMetadata, ChunkSchema, WrongDatatypeError,
};

#[derive(thiserror::Error, Debug)]
pub enum MismatchedChunkSchemaError {
    #[error("{0}")]
    Custom(String),

    #[error(transparent)]
    WrongDatatypeError(#[from] WrongDatatypeError),
}

impl MismatchedChunkSchemaError {
    pub fn custom(s: impl Into<String>) -> Self {
        Self::Custom(s.into())
    }
}

/// The arrow [`ArrowRecordBatch`] representation of a Rerun chunk.
///
/// This is a wrapper around a [`ArrowRecordBatch`].
///
/// Each [`ChunkBatch`] contains logging data for a single [`EntityPath`].
/// It always has a [`RowId`] column.
#[derive(Debug, Clone)]
pub struct ChunkBatch {
    schema: ChunkSchema,

    // TODO: should we store a record batch here, or just the parsed columns?
    batch: ArrowRecordBatch,
}

impl ChunkBatch {
    pub fn try_new(
        schema: ChunkSchema,
        row_ids: ArrowArrayRef,
        index_arrays: Vec<ArrowArrayRef>,
        data_arrays: Vec<ArrowArrayRef>,
    ) -> Result<Self, MismatchedChunkSchemaError> {
        let row_count = row_ids.len();

        WrongDatatypeError::compare_expected_actual(
            &schema.row_id_column.datatype(),
            row_ids.data_type(),
        )?;

        if index_arrays.len() != schema.index_columns.len() {
            return Err(MismatchedChunkSchemaError::custom(format!(
                "Schema had {} index columns, but got {}",
                schema.index_columns.len(),
                index_arrays.len()
            )));
        }
        for (schema, array) in itertools::izip!(&schema.index_columns, &index_arrays) {
            WrongDatatypeError::compare_expected_actual(schema.datatype(), array.data_type())?;
            if array.len() != row_count {
                return Err(MismatchedChunkSchemaError::custom(format!(
                    "Index column {:?} had {} rows, but we got {} row IDs",
                    schema.name(),
                    array.len(),
                    row_count
                )));
            }
        }

        if data_arrays.len() != schema.data_columns.len() {
            return Err(MismatchedChunkSchemaError::custom(format!(
                "Schema had {} data columns, but got {}",
                schema.data_columns.len(),
                data_arrays.len()
            )));
        }
        for (schema, array) in itertools::izip!(&schema.data_columns, &data_arrays) {
            WrongDatatypeError::compare_expected_actual(&schema.store_datatype, array.data_type())?;
            if array.len() != row_count {
                return Err(MismatchedChunkSchemaError::custom(format!(
                    "Data column {:?} had {} rows, but we got {} row IDs",
                    schema.column_name(),
                    array.len(),
                    row_count
                )));
            }
        }

        let arrow_columns = itertools::chain!(Some(row_ids), index_arrays, data_arrays).collect();

        let batch = ArrowRecordBatch::try_new_with_options(
            std::sync::Arc::new(ArrowSchema::from(&schema)),
            arrow_columns,
            &RecordBatchOptions::default().with_row_count(Some(row_count)),
        )
        .unwrap();

        Ok(Self { schema, batch })
    }
}

impl ChunkBatch {
    /// The parsed rerun schema of this chunk.
    #[inline]
    pub fn chunk_schema(&self) -> &ChunkSchema {
        &self.schema
    }

    /// The globally unique ID of this chunk.
    #[inline]
    pub fn chunk_id(&self) -> ChunkId {
        self.schema.chunk_id()
    }

    /// Which entity is this chunk for?
    #[inline]
    pub fn entity_path(&self) -> &EntityPath {
        self.schema.entity_path()
    }

    /// The heap size of this chunk in bytes, if known.
    #[inline]
    pub fn heap_size_bytes(&self) -> Option<u64> {
        self.schema.heap_size_bytes()
    }

    /// Are we sorted by the row id column?
    #[inline]
    pub fn is_sorted(&self) -> bool {
        self.schema.is_sorted()
    }

    #[inline]
    pub fn fields(&self) -> &ArrowFields {
        &self.schema_ref().fields
    }

    #[inline]
    pub fn arrow_bacth_metadata(&self) -> &ArrowBatchMetadata {
        &self.batch.schema_ref().metadata
    }
}

impl std::fmt::Display for ChunkBatch {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        re_format_arrow::format_record_batch_with_width(self, f.width()).fmt(f)
    }
}

impl AsRef<ArrowRecordBatch> for ChunkBatch {
    #[inline]
    fn as_ref(&self) -> &ArrowRecordBatch {
        &self.batch
    }
}

impl std::ops::Deref for ChunkBatch {
    type Target = ArrowRecordBatch;

    #[inline]
    fn deref(&self) -> &ArrowRecordBatch {
        &self.batch
    }
}

impl From<ChunkBatch> for ArrowRecordBatch {
    #[inline]
    fn from(chunk: ChunkBatch) -> Self {
        chunk.batch
    }
}

impl From<&ChunkBatch> for ArrowRecordBatch {
    #[inline]
    fn from(chunk: &ChunkBatch) -> Self {
        chunk.batch.clone()
    }
}

impl TryFrom<ArrowRecordBatch> for ChunkBatch {
    type Error = InvalidChunkSchema;

    fn try_from(batch: ArrowRecordBatch) -> Result<Self, Self::Error> {
        let chunk_schema = ChunkSchema::try_from(batch.schema_ref().as_ref())?;

        // Extend with any metadata that might have been missing:
        let mut arrow_schema = ArrowSchema::clone(batch.schema_ref().as_ref());
        arrow_schema
            .metadata
            .extend(chunk_schema.arrow_batch_metadata());
        let batch = batch.with_schema(arrow_schema.into()).expect("Can't fail");

        Ok(Self {
            schema: chunk_schema,
            batch,
        })
    }
}
