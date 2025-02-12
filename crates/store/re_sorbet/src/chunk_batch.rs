use arrow::{
    array::{
        ArrayRef as ArrowArrayRef, AsArray, RecordBatch as ArrowRecordBatch,
        StructArray as ArrowStructArray,
    },
    datatypes::Fields as ArrowFields,
};

use re_log_types::EntityPath;
use re_types_core::ChunkId;

use crate::{
    ArrowBatchMetadata, ChunkSchema, ComponentColumnDescriptor, IndexColumnDescriptor,
    RowIdColumnDescriptor, SorbetBatch, SorbetError, WrongDatatypeError,
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

/// The [`ArrowRecordBatch`] representation of a Rerun chunk.
///
/// This is a wrapper around a [`ChunkSchema`] and a [`ArrowRecordBatch`].
///
/// Each [`ChunkBatch`] contains logging data for a single [`EntityPath`].
/// It always has a [`re_types_core::RowId`] column.
#[derive(Debug, Clone)]
pub struct ChunkBatch {
    schema: ChunkSchema,
    sorbet_batch: SorbetBatch,
}

impl ChunkBatch {
    pub fn try_new(
        schema: ChunkSchema,
        row_ids: ArrowArrayRef,
        index_arrays: Vec<ArrowArrayRef>,
        data_arrays: Vec<ArrowArrayRef>,
    ) -> Result<Self, SorbetError> {
        Self::try_from(SorbetBatch::try_new(
            schema.into(),
            Some(row_ids),
            index_arrays,
            data_arrays,
        )?)
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
        &self.schema_ref().metadata
    }

    pub fn row_id_column(&self) -> (&RowIdColumnDescriptor, &ArrowStructArray) {
        // The first column is always the row IDs.
        (
            self.schema.row_id_column(),
            self.columns()[0]
                .as_struct_opt()
                .expect("Row IDs should be encoded as struct"),
        )
    }

    /// The columns of the indices (timelines).
    pub fn index_columns(&self) -> impl Iterator<Item = (&IndexColumnDescriptor, &ArrowArrayRef)> {
        self.sorbet_batch.index_columns()
    }

    /// The columns of the indices (timelines).
    pub fn component_columns(
        &self,
    ) -> impl Iterator<Item = (&ComponentColumnDescriptor, &ArrowArrayRef)> {
        self.sorbet_batch.component_columns()
    }
}

impl std::fmt::Display for ChunkBatch {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        re_format_arrow::format_record_batch_with_width(self, f.width()).fmt(f)
    }
}

impl AsRef<SorbetBatch> for ChunkBatch {
    #[inline]
    fn as_ref(&self) -> &SorbetBatch {
        &self.sorbet_batch
    }
}

impl std::ops::Deref for ChunkBatch {
    type Target = SorbetBatch;

    #[inline]
    fn deref(&self) -> &SorbetBatch {
        &self.sorbet_batch
    }
}

impl From<ChunkBatch> for ArrowRecordBatch {
    #[inline]
    fn from(chunk: ChunkBatch) -> Self {
        chunk.sorbet_batch.into()
    }
}

impl From<&ChunkBatch> for ArrowRecordBatch {
    #[inline]
    fn from(chunk: &ChunkBatch) -> Self {
        chunk.sorbet_batch.clone().into()
    }
}

impl TryFrom<&ArrowRecordBatch> for ChunkBatch {
    type Error = SorbetError;

    /// Will automatically wrap data columns in `ListArrays` if they are not already.
    fn try_from(batch: &ArrowRecordBatch) -> Result<Self, Self::Error> {
        re_tracing::profile_function!();

        Self::try_from(SorbetBatch::try_from(batch)?)
    }
}
impl TryFrom<SorbetBatch> for ChunkBatch {
    type Error = SorbetError;

    /// Will automatically wrap data columns in `ListArrays` if they are not already.
    fn try_from(sorbet_batch: SorbetBatch) -> Result<Self, Self::Error> {
        re_tracing::profile_function!();

        let chunk_schema = ChunkSchema::try_from(sorbet_batch.sorbet_schema().clone())?;

        Ok(Self {
            schema: chunk_schema,
            sorbet_batch,
        })
    }
}
