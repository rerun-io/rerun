use arrow::array::{
    ArrayRef as ArrowArrayRef, AsArray as _, FixedSizeBinaryArray, RecordBatch as ArrowRecordBatch,
};
use arrow::datatypes::Fields as ArrowFields;
use re_arrow_util::WrongDatatypeError;
use re_log_types::EntityPath;
use re_types_core::ChunkId;

use crate::{ChunkSchema, RowIdColumnDescriptor, SorbetBatch, SorbetError};

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
            crate::BatchType::Chunk,
            schema.into(),
            Some(row_ids),
            index_arrays,
            data_arrays,
        )?)
    }
}

impl ChunkBatch {
    /// The parsed rerun schema of this chunk.
    ///
    /// *IMPORTANT*: the returned `ChunkSchema` has potentially incorrect metadata, since it can
    /// only be derived from an entire chunk store (e.g. a column is static if _any_ chunk
    /// containing that column is static).
    ///
    /// See `re_chunk_store::ChunkStore::schema` or [`crate::SchemaBuilder`] to compute
    /// schemas with accurate metadata.
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

    /// Is this chunk static?
    #[inline]
    pub fn is_static(&self) -> bool {
        self.schema.is_static()
    }

    #[inline]
    pub fn fields(&self) -> &ArrowFields {
        &self.schema_ref().fields
    }

    /// The `RowId` column.
    pub fn row_id_column(&self) -> (&RowIdColumnDescriptor, &FixedSizeBinaryArray) {
        // The first column is always the row IDs.
        (
            self.schema.row_id_column(),
            self.columns()[0].as_fixed_size_binary(),
        )
    }

    /// Returns self but with all rows removed.
    #[must_use]
    pub fn drop_all_rows(self) -> Self {
        Self {
            schema: self.schema.clone(),
            sorbet_batch: self.sorbet_batch.drop_all_rows(),
        }
    }
}

impl std::fmt::Display for ChunkBatch {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        re_arrow_util::format_record_batch_with_width(self, f.width(), f.sign_minus()).fmt(f)
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

    /// Will perform some transformations:
    /// * Will automatically wrap data columns in `ListArrays` if they are not already
    /// * Will reorder columns so that Row ID comes before timelines, which come before data
    /// * Will migrate component descriptors to colon-based notation
    #[tracing::instrument(level = "trace", skip_all)]
    fn try_from(batch: &ArrowRecordBatch) -> Result<Self, Self::Error> {
        re_tracing::profile_function!();

        Self::try_from(SorbetBatch::try_from_record_batch(
            batch,
            crate::BatchType::Chunk,
        )?)
    }
}

impl TryFrom<SorbetBatch> for ChunkBatch {
    type Error = SorbetError;

    /// Will automatically wrap data columns in `ListArrays` if they are not already.
    #[tracing::instrument(level = "trace", skip_all)]
    fn try_from(sorbet_batch: SorbetBatch) -> Result<Self, Self::Error> {
        re_tracing::profile_function!();

        let chunk_schema = ChunkSchema::try_from(sorbet_batch.sorbet_schema().clone())?;

        Ok(Self {
            schema: chunk_schema,
            sorbet_batch,
        })
    }
}
