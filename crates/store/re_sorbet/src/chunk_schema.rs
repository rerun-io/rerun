use arrow::datatypes::{Field as ArrowField, Schema as ArrowSchema};

use re_log_types::EntityPath;
use re_types_core::ChunkId;

use crate::{
    ArrowBatchMetadata, ComponentColumnDescriptor, IndexColumnDescriptor, InvalidSorbetSchema,
    RowIdColumnDescriptor, SorbetColumnDescriptors, SorbetSchema,
};

/// The parsed schema of a Rerun chunk, i.e. multiple columns of data for a single entity.
///
/// This does NOT preserve custom arrow metadata.
/// It only contains the metadata used by Rerun.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkSchema {
    sorbet: SorbetSchema,

    // Some things here are also in [`SorbetSchema]`, but are duplicated
    // here because they are non-optional:
    pub row_id: RowIdColumnDescriptor,
    pub chunk_id: ChunkId,
    pub entity_path: EntityPath,
}

/// ## Builders
impl ChunkSchema {
    pub fn new(
        chunk_id: ChunkId,
        entity_path: EntityPath,
        row_id: RowIdColumnDescriptor,
        indices: Vec<IndexColumnDescriptor>,
        components: Vec<ComponentColumnDescriptor>,
    ) -> Self {
        Self {
            sorbet: SorbetSchema {
                columns: SorbetColumnDescriptors {
                    row_id: Some(row_id.clone()),
                    indices,
                    components,
                },
                chunk_id: Some(chunk_id),
                entity_path: Some(entity_path.clone()),
                heap_size_bytes: None,
                is_sorted: false, // assume the worst
            },
            row_id,
            chunk_id,
            entity_path,
        }
    }

    #[inline]
    pub fn with_heap_size_bytes(mut self, heap_size_bytes: u64) -> Self {
        self.sorbet.heap_size_bytes = Some(heap_size_bytes);
        self
    }

    #[inline]
    pub fn with_sorted(mut self, sorted: bool) -> Self {
        self.sorbet.is_sorted = sorted;
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

    /// The heap size of this chunk in bytes, if known.
    #[inline]
    pub fn heap_size_bytes(&self) -> Option<u64> {
        self.sorbet.heap_size_bytes
    }

    /// Are we sorted by the row id column?
    #[inline]
    pub fn is_sorted(&self) -> bool {
        self.sorbet.is_sorted
    }

    /// Total number of columns in this chunk,
    /// including the row id column, the index columns,
    /// and the data columns.
    pub fn num_columns(&self) -> usize {
        self.sorbet.columns.num_columns()
    }

    #[inline]
    pub fn row_id_column(&self) -> &RowIdColumnDescriptor {
        &self.row_id
    }

    #[inline]
    pub fn index_columns(&self) -> &[IndexColumnDescriptor] {
        &self.sorbet.columns.indices
    }

    #[inline]
    pub fn component_columns(&self) -> &[ComponentColumnDescriptor] {
        &self.sorbet.columns.components
    }

    pub fn arrow_batch_metadata(&self) -> ArrowBatchMetadata {
        self.sorbet.arrow_batch_metadata()
    }

    pub fn arrow_fields(&self) -> Vec<ArrowField> {
        self.sorbet.columns.arrow_fields()
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

impl TryFrom<&ArrowSchema> for ChunkSchema {
    type Error = InvalidSorbetSchema;

    fn try_from(arrow_schema: &ArrowSchema) -> Result<Self, Self::Error> {
        let sorbet_schema = SorbetSchema::try_from(arrow_schema)?;

        Ok(Self {
            row_id: sorbet_schema
                .columns
                .row_id
                .clone()
                .ok_or_else(|| InvalidSorbetSchema::custom("Missing row_id column"))?,
            chunk_id: sorbet_schema
                .chunk_id
                .ok_or_else(|| InvalidSorbetSchema::custom("Missing chunk_id"))?,
            entity_path: sorbet_schema
                .entity_path
                .clone()
                .ok_or_else(|| InvalidSorbetSchema::custom("Missing entity_path"))?,

            sorbet: sorbet_schema,
        })
    }
}
