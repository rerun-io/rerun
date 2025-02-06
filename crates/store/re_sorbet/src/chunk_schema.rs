use arrow::datatypes::{Field as ArrowField, Schema as ArrowSchema};

use re_log_types::EntityPath;
use re_types_core::ChunkId;

use crate::{ArrowBatchMetadata, ComponentColumnDescriptor, RowIdColumnDescriptor};

/// The parsed schema of a Rerun chunk, i.e. multiple columns of data for a single entity.
///
/// This does NOT preserve custom arrow metadata.
/// It only contains the metadata used by Rerun.
pub struct ChunkSchema {
    /// The globally unique ID of this chunk.
    chunk_id: ChunkId,

    /// Which entity is this chunk for?
    entity_path: EntityPath,

    /// Are we sorted by the row id column?
    is_sorted: bool,

    /// The primary row id column.
    row_id_column: RowIdColumnDescriptor,

    /// All other columns (indices and data).
    columns: Vec<ComponentColumnDescriptor>,
}

impl From<ChunkSchema> for ArrowSchema {
    fn from(chunk_schema: ChunkSchema) -> Self {
        let ChunkSchema {
            chunk_id,
            entity_path,
            is_sorted,
            row_id_column,
            columns,
        } = chunk_schema;

        let mut metadata = ArrowBatchMetadata::from([
            ("rerun.id".to_owned(), format!("{:X}", chunk_id.as_u128())),
            ("rerun.entity_path".to_owned(), entity_path.to_string()),
            // TODO: heap_size_bytes ?
        ]);
        if is_sorted {
            metadata.insert("rerun.is_sorted".to_owned(), "true".to_owned());
        }

        let mut fields: Vec<ArrowField> = Vec::with_capacity(1 + columns.len());
        fields.push(row_id_column.to_arrow_field());
        fields.extend(columns.iter().map(|column| column.to_arrow_field()));

        Self {
            metadata,
            fields: fields.into(),
        }
    }
}

// impl TryFrom<ArrowSchema> for ChunkSchema { }
