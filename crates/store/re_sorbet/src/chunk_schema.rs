use std::ops::{Deref, DerefMut};

use arrow::datatypes::{Field as ArrowField, Schema as ArrowSchema};
use re_log_types::EntityPath;
use re_types_core::ChunkId;

use crate::chunk_columns::ChunkColumnDescriptors;
use crate::{
    ArrowBatchMetadata, ColumnDescriptor, ComponentColumnDescriptor, IndexColumnDescriptor,
    RowIdColumnDescriptor, SorbetColumnDescriptors, SorbetError, SorbetSchema,
};

/// The parsed schema of a Rerun chunk, i.e. multiple columns of data for a single entity.
///
/// This does NOT preserve custom arrow metadata.
/// It only contains the metadata used by Rerun.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkSchema {
    sorbet: SorbetSchema,

    // Some things here are also in [`SorbetSchema]`, but are duplicated
    // here because they have additional constraints (e.g. ordering, non-optional):
    chunk_columns: ChunkColumnDescriptors,
    chunk_id: ChunkId,
    entity_path: EntityPath,
}

impl From<ChunkSchema> for SorbetSchema {
    #[inline]
    fn from(value: ChunkSchema) -> Self {
        value.sorbet
    }
}

impl Deref for ChunkSchema {
    type Target = SorbetSchema;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.sorbet
    }
}

impl DerefMut for ChunkSchema {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.sorbet
    }
}

/// ## Builders
impl ChunkSchema {
    pub fn new(
        chunk_id: ChunkId,
        entity_path: EntityPath,
        row_id: RowIdColumnDescriptor,
        indices: Vec<IndexColumnDescriptor>,
        components: Vec<ComponentColumnDescriptor>,
        timestamps: crate::TimestampMetadata,
    ) -> Self {
        Self {
            sorbet: SorbetSchema {
                columns: SorbetColumnDescriptors {
                    columns: itertools::chain!(
                        std::iter::once(ColumnDescriptor::RowId(row_id.clone())),
                        indices.iter().cloned().map(ColumnDescriptor::Time),
                        components.iter().cloned().map(ColumnDescriptor::Component),
                    )
                    .collect(),
                },
                segment_id: None, // TODO(#9977): This should be required in the future.
                chunk_id: Some(chunk_id),
                entity_path: Some(entity_path.clone()),
                heap_size_bytes: None,
                timestamps,
            },
            chunk_columns: ChunkColumnDescriptors {
                row_id,
                indices,
                components,
            },
            chunk_id,
            entity_path,
        }
    }

    #[inline]
    pub fn with_heap_size_bytes(mut self, heap_size_bytes: u64) -> Self {
        self.sorbet.heap_size_bytes = Some(heap_size_bytes);
        self
    }
}

/// ## Accessors
impl ChunkSchema {
    /// The globally unique ID of this chunk.
    #[inline]
    pub fn chunk_id(&self) -> ChunkId {
        self.chunk_id
    }

    /// Which entity is this chunk for?
    #[inline]
    pub fn entity_path(&self) -> &EntityPath {
        &self.entity_path
    }

    /// Is this chunk static?
    #[inline]
    pub fn is_static(&self) -> bool {
        self.chunk_columns.indices.is_empty()
    }

    /// The heap size of this chunk in bytes, if known.
    #[inline]
    pub fn heap_size_bytes(&self) -> Option<u64> {
        self.sorbet.heap_size_bytes
    }

    /// Total number of columns in this chunk,
    /// including the row id column, the index columns,
    /// and the data columns.
    pub fn num_columns(&self) -> usize {
        self.sorbet.columns.num_columns()
    }

    #[inline]
    pub fn row_id_column(&self) -> &RowIdColumnDescriptor {
        &self.chunk_columns.row_id
    }

    pub fn arrow_batch_metadata(&self) -> ArrowBatchMetadata {
        self.sorbet.arrow_batch_metadata()
    }

    pub fn arrow_fields(&self) -> Vec<ArrowField> {
        self.sorbet.columns.arrow_fields(crate::BatchType::Chunk)
    }
}

impl From<&ChunkSchema> for ArrowSchema {
    fn from(chunk_schema: &ChunkSchema) -> Self {
        Self {
            metadata: chunk_schema.arrow_batch_metadata(),
            fields: chunk_schema.arrow_fields().into(),
        }
    }
}

impl TryFrom<SorbetSchema> for ChunkSchema {
    type Error = SorbetError;

    fn try_from(sorbet_schema: SorbetSchema) -> Result<Self, Self::Error> {
        Ok(Self {
            sorbet: sorbet_schema.clone(),

            chunk_columns: ChunkColumnDescriptors::try_from(sorbet_schema.columns.clone())?,

            chunk_id: sorbet_schema.chunk_id.ok_or(SorbetError::MissingChunkId)?,

            entity_path: sorbet_schema
                .entity_path
                .ok_or(SorbetError::MissingEntityPath)?,
        })
    }
}
