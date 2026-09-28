use arrow::datatypes::{Field as ArrowField, Schema as ArrowSchema};
use re_log_types::EntityPath;
use re_types_core::{ChunkId, SegmentId};

use crate::chunk_columns::ChunkColumnDescriptors;
use crate::{
    ArrowBatchMetadata, BatchType, ComponentColumnDescriptor, IndexColumnDescriptor,
    LatencyMetadata, RowIdColumnDescriptor, SorbetError, SorbetSchema,
};

/// The parsed schema of a Rerun chunk, i.e. multiple columns of data for a single entity.
///
/// Compared to a [`SorbetSchema`], the chunk id and entity path are mandatory, and the columns
/// are always ordered as row id, then indices, then components.
///
/// This does NOT preserve custom arrow metadata.
/// It only contains the metadata used by Rerun.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkSchema {
    chunk_id: ChunkId,
    entity_path: EntityPath,
    segment_id: Option<SegmentId>,
    columns: ChunkColumnDescriptors,
    latency_metadata: LatencyMetadata,
}

/// ## Builders
impl ChunkSchema {
    pub fn new(
        chunk_id: ChunkId,
        entity_path: EntityPath,
        row_id: RowIdColumnDescriptor,
        indices: Vec<IndexColumnDescriptor>,
        components: Vec<ComponentColumnDescriptor>,
        latency_metadata: LatencyMetadata,
    ) -> Self {
        Self {
            chunk_id,
            entity_path,
            segment_id: None, // TODO(#9977): This should be required in the future.
            columns: ChunkColumnDescriptors {
                row_id,
                indices,
                components,
            },
            latency_metadata,
        }
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

    /// The segment this chunk belongs to, if known.
    #[inline]
    pub fn segment_id(&self) -> Option<&SegmentId> {
        self.segment_id.as_ref()
    }

    /// Is this chunk static?
    #[inline]
    pub fn is_static(&self) -> bool {
        self.columns.indices.is_empty()
    }

    /// Total number of columns in this chunk,
    /// including the row id column, the index columns,
    /// and the data columns.
    pub fn num_columns(&self) -> usize {
        1 + self.columns.indices.len() + self.columns.components.len()
    }

    #[inline]
    pub fn columns(&self) -> &ChunkColumnDescriptors {
        &self.columns
    }

    #[inline]
    pub fn row_id_column(&self) -> &RowIdColumnDescriptor {
        &self.columns.row_id
    }

    /// The index (timeline) columns, in column order.
    #[inline]
    pub fn index_columns(&self) -> &[IndexColumnDescriptor] {
        &self.columns.indices
    }

    /// The component columns, in column order.
    #[inline]
    pub fn component_columns(&self) -> &[ComponentColumnDescriptor] {
        &self.columns.components
    }

    /// Latency-measurement timestamps of the pipeline stages this chunk has passed.
    ///
    /// NOT related to timelines.
    #[inline]
    pub fn latency_metadata(&self) -> &LatencyMetadata {
        &self.latency_metadata
    }

    #[inline]
    pub fn latency_metadata_mut(&mut self) -> &mut LatencyMetadata {
        &mut self.latency_metadata
    }

    pub fn arrow_batch_metadata(&self) -> ArrowBatchMetadata {
        crate::sorbet_schema::arrow_batch_metadata(
            Some(&self.chunk_id),
            Some(&self.entity_path),
            self.segment_id.as_ref(),
            &self.latency_metadata,
        )
    }

    pub fn arrow_fields(&self) -> Vec<ArrowField> {
        self.columns
            .iter_ref()
            .map(|c| c.to_arrow_field(BatchType::Chunk))
            .collect()
    }

    pub fn to_arrow(&self) -> ArrowSchema {
        ArrowSchema {
            metadata: self.arrow_batch_metadata(),
            fields: self.arrow_fields().into(),
        }
    }
}

impl re_byte_size::SizeBytes for ChunkSchema {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            chunk_id: _,
            entity_path,
            segment_id,
            columns,
            latency_metadata,
        } = self;

        entity_path.heap_size_bytes()
            + segment_id.heap_size_bytes()
            + columns.heap_size_bytes()
            + latency_metadata.heap_size_bytes()
    }
}

impl From<&ChunkSchema> for ArrowSchema {
    fn from(chunk_schema: &ChunkSchema) -> Self {
        chunk_schema.to_arrow()
    }
}

impl From<ChunkSchema> for SorbetSchema {
    fn from(chunk_schema: ChunkSchema) -> Self {
        let ChunkSchema {
            chunk_id,
            entity_path,
            segment_id,
            columns,
            latency_metadata,
        } = chunk_schema;

        Self {
            columns: columns.into(),
            chunk_id: Some(chunk_id),
            entity_path: Some(entity_path),
            segment_id,
            latency_metadata,
        }
    }
}

impl TryFrom<SorbetSchema> for ChunkSchema {
    type Error = SorbetError;

    fn try_from(sorbet_schema: SorbetSchema) -> Result<Self, Self::Error> {
        let SorbetSchema {
            columns,
            chunk_id,
            entity_path,
            segment_id,
            latency_metadata,
        } = sorbet_schema;

        Ok(Self {
            chunk_id: chunk_id.ok_or(SorbetError::MissingChunkId)?,
            entity_path: entity_path.ok_or(SorbetError::MissingEntityPath)?,
            segment_id,
            columns: ChunkColumnDescriptors::try_from(columns)?,
            latency_metadata,
        })
    }
}
