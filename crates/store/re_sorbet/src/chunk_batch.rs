use std::sync::Arc;

use arrow::array::{
    ArrayRef as ArrowArrayRef, AsArray as _, FixedSizeBinaryArray, RecordBatch, RecordBatchOptions,
};
use arrow::datatypes::Fields as ArrowFields;
use re_arrow_util::WrongDatatypeError;
use re_log_types::EntityPath;
use re_types_core::ChunkId;

use crate::{
    ArrowBatchMetadata, ChunkSchema, ComponentColumnDescriptor, IndexColumnDescriptor,
    RowIdColumnDescriptor, SorbetBatch, SorbetError, TimestampMetadata,
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

/// The [`RecordBatch`] representation of a Rerun chunk.
///
/// This is a wrapper around a [`ChunkSchema`] and a [`RecordBatch`].
///
/// Each [`ChunkBatch`] contains logging data for a single [`EntityPath`].
/// It always has a [`re_types_core::RowId`] column.
///
/// Every [`ChunkBatch`] can be turned into a [`SorbetBatch`], but the opposite does not hold.
#[derive(Debug, Clone, PartialEq)]
pub struct ChunkBatch {
    schema: ChunkSchema,

    /// Carries all the Rerun metadata described by `schema`, kept in sync with it on every
    /// mutation, plus any non-Rerun metadata intact from wherever it was created from.
    batch: RecordBatch,
}

impl ChunkBatch {
    pub fn try_new(
        schema: ChunkSchema,
        row_ids: ArrowArrayRef,
        index_arrays: Vec<ArrowArrayRef>,
        data_arrays: Vec<ArrowArrayRef>,
    ) -> Result<Self, SorbetError> {
        schema.columns().sanity_check();

        let arrow_columns =
            itertools::chain!(std::iter::once(row_ids), index_arrays, data_arrays).collect();

        let batch = RecordBatch::try_new_with_options(
            Arc::new(schema.to_arrow()),
            arrow_columns,
            &RecordBatchOptions::default(),
        )?;

        Ok(Self { schema, batch })
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

    /// Latency-measurement timestamps of the pipeline stages this chunk has passed.
    #[inline]
    pub fn latency_metadata(&self) -> &TimestampMetadata {
        self.schema.latency_metadata()
    }

    #[inline]
    pub fn fields(&self) -> &ArrowFields {
        &self.batch.schema_ref().fields
    }

    /// The record batch metadata, including any non-Rerun metadata.
    #[inline]
    pub fn arrow_batch_metadata(&self) -> &ArrowBatchMetadata {
        &self.batch.schema_ref().metadata
    }

    /// The `RowId` column.
    pub fn row_id_column(&self) -> (&RowIdColumnDescriptor, &FixedSizeBinaryArray) {
        // The first column is always the row IDs.
        (
            self.schema.row_id_column(),
            self.batch.columns()[0].as_fixed_size_binary(),
        )
    }

    /// The columns of the indices (timelines).
    pub fn index_columns(&self) -> impl Iterator<Item = (&IndexColumnDescriptor, &ArrowArrayRef)> {
        // Index columns directly follow the row id column.
        itertools::izip!(self.schema.index_columns(), &self.batch.columns()[1..])
    }

    /// The columns of the components.
    pub fn component_columns(
        &self,
    ) -> impl Iterator<Item = (&ComponentColumnDescriptor, &ArrowArrayRef)> {
        // Component columns follow the row id and index columns.
        let start = 1 + self.schema.index_columns().len();
        itertools::izip!(
            self.schema.component_columns(),
            &self.batch.columns()[start..]
        )
    }

    /// Returns self but with all rows removed.
    #[must_use]
    pub fn drop_all_rows(self) -> Self {
        Self {
            schema: self.schema,
            batch: self.batch.slice(0, 0),
        }
    }

    /// Records the current time as the moment this batch passed `location`.
    ///
    /// Updates both the Arrow metadata and the parsed [`ChunkSchema`].
    /// Does nothing for locations that are not carried in the batch metadata.
    pub fn track_latency(&mut self, location: crate::TimestampLocation) {
        self.schema
            .latency_metadata_mut()
            .track_latency(self.batch.schema_metadata_mut(), location);
    }
}

impl std::fmt::Display for ChunkBatch {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        re_arrow_util::format_record_batch_with_width(self, f.width(), f.sign_minus()).fmt(f)
    }
}

impl re_byte_size::SizeBytes for ChunkBatch {
    fn heap_size_bytes(&self) -> u64 {
        let Self { schema, batch } = self;
        schema.heap_size_bytes() + batch.heap_size_bytes()
    }
}

impl AsRef<RecordBatch> for ChunkBatch {
    #[inline]
    fn as_ref(&self) -> &RecordBatch {
        &self.batch
    }
}

impl std::ops::Deref for ChunkBatch {
    type Target = RecordBatch;

    #[inline]
    fn deref(&self) -> &RecordBatch {
        &self.batch
    }
}

impl From<ChunkBatch> for RecordBatch {
    #[inline]
    fn from(chunk: ChunkBatch) -> Self {
        chunk.batch
    }
}

impl From<&ChunkBatch> for RecordBatch {
    #[inline]
    fn from(chunk: &ChunkBatch) -> Self {
        chunk.batch.clone()
    }
}

impl TryFrom<&RecordBatch> for ChunkBatch {
    type Error = SorbetError;

    /// Will perform some transformations:
    /// * Will automatically wrap data columns in `ListArrays` if they are not already
    /// * Will migrate component descriptors to colon-based notation
    ///
    /// Columns are not reordered: the row id column must come first, followed by the index
    /// columns, then the component columns.
    fn try_from(batch: &RecordBatch) -> Result<Self, Self::Error> {
        re_tracing::profile_function!();

        Self::try_from(SorbetBatch::try_from_record_batch(
            batch,
            crate::BatchType::Chunk,
        )?)
    }
}

impl TryFrom<SorbetBatch> for ChunkBatch {
    type Error = SorbetError;

    fn try_from(sorbet_batch: SorbetBatch) -> Result<Self, Self::Error> {
        re_tracing::profile_function!();

        let (sorbet_schema, batch) = sorbet_batch.into_parts();

        Ok(Self {
            schema: ChunkSchema::try_from(sorbet_schema)?,
            batch,
        })
    }
}
