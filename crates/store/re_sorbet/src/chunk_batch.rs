use arrow::{
    array::RecordBatch as ArrowRecordBatch,
    datatypes::{Fields as ArrowFields, Schema as ArrowSchema},
};

use re_log_types::EntityPath;
use re_types_core::ChunkId;

use crate::{chunk_schema::InvalidChunkSchema, ArrowBatchMetadata, ChunkSchema};

/// The arrow [`ArrowRecordBatch`] representation of a Rerun chunk.
///
/// This is a wrapper around a [`ArrowRecordBatch`].
///
/// Each [`ChunkBatch`] contains logging data for a single [`EntityPath`].
/// It always has a [`RowId`] column.
#[derive(Debug, Clone)]
pub struct ChunkBatch {
    schema: ChunkSchema,
    batch: ArrowRecordBatch,
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
