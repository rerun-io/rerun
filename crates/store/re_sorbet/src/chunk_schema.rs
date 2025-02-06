use arrow::datatypes::{Field as ArrowField, Schema as ArrowSchema};

use re_log_types::EntityPath;
use re_types_core::ChunkId;

use crate::{
    ArrowBatchMetadata, ComponentColumnDescriptor, MetadataExt as _, MissingFieldMetadata,
    MissingMetadataKey, RowIdColumnDescriptor, WrongDatatypeError,
};

#[derive(thiserror::Error, Debug)]
pub enum ChunkSchemaError {
    #[error(transparent)]
    MissingMetadataKey(#[from] MissingMetadataKey),

    #[error("Bad RowId columns: {0}")]
    BadRowIdColumn(WrongDatatypeError),

    #[error("Bad column '{field_name}': {error}")]
    BadColumn {
        field_name: String,
        error: MissingFieldMetadata,
    },

    #[error("Bad chunk schema: {reason}")]
    Custom { reason: String },
}

impl ChunkSchemaError {
    fn custom(reason: impl Into<String>) -> Self {
        Self::Custom {
            reason: reason.into(),
        }
    }
}

/// The parsed schema of a Rerun chunk, i.e. multiple columns of data for a single entity.
///
/// This does NOT preserve custom arrow metadata.
/// It only contains the metadata used by Rerun.
pub struct ChunkSchema {
    /// The globally unique ID of this chunk.
    chunk_id: ChunkId,

    /// Which entity is this chunk for?
    entity_path: EntityPath,

    /// The heap size of this chunk in bytes, if known.
    heap_size_bytes: Option<u64>,

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
            heap_size_bytes,
            is_sorted,
            row_id_column,
            columns,
        } = chunk_schema;

        let mut metadata = ArrowBatchMetadata::from([
            ("rerun.id".to_owned(), format!("{:X}", chunk_id.as_u128())),
            ("rerun.entity_path".to_owned(), entity_path.to_string()),
        ]);
        if let Some(heap_size_bytes) = heap_size_bytes {
            metadata.insert(
                "rerun.heap_size_bytes".to_owned(),
                heap_size_bytes.to_string(),
            );
        }
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

impl TryFrom<ArrowSchema> for ChunkSchema {
    type Error = ChunkSchemaError;

    fn try_from(arrow_schema: ArrowSchema) -> Result<Self, Self::Error> {
        let ArrowSchema { metadata, fields } = arrow_schema;

        let chunk_id = {
            let chunk_id = metadata.get_or_err("rerun.id")?;
            let chunk_id = u128::from_str_radix(chunk_id, 16).map_err(|err| {
                ChunkSchemaError::custom(format!(
                    "Failed to deserialize chunk id {chunk_id:?}: {err}"
                ))
            })?;
            ChunkId::from_u128(chunk_id)
        };

        let entity_path = EntityPath::parse_forgiving(metadata.get_or_err("rerun.entity_path")?);
        let is_sorted = metadata.get_bool("rerun.is_sorted");
        let heap_size_bytes = if let Some(heap_size_bytes) = metadata.get("rerun.heap_size_bytes") {
            heap_size_bytes.parse().ok() // TODO: log error
        } else {
            None
        };

        // The first field must be the row id column:
        let Some(first_field) = fields.first() else {
            return Err(ChunkSchemaError::custom("No fields in schema"));
        };

        let row_id_column = RowIdColumnDescriptor::try_from(first_field.as_ref())
            .map_err(ChunkSchemaError::BadRowIdColumn)?;

        let columns: Result<Vec<_>, _> = fields
            .iter()
            .skip(1)
            .map(|field| {
                ComponentColumnDescriptor::try_from(field.as_ref()).map_err(|err| {
                    ChunkSchemaError::BadColumn {
                        field_name: field.name().to_owned(),
                        error: err,
                    }
                })
            })
            .collect();
        let columns = columns?;

        Ok(Self {
            chunk_id,
            entity_path,
            heap_size_bytes,
            is_sorted,
            row_id_column,
            columns,
        })
    }
}
