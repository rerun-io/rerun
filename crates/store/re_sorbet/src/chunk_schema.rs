use arrow::datatypes::{Field as ArrowField, Schema as ArrowSchema};

use re_log_types::EntityPath;
use re_types_core::ChunkId;

use crate::{
    ArrowBatchMetadata, ComponentColumnDescriptor, MetadataExt as _, MissingFieldMetadata,
    MissingMetadataKey, RowIdColumnDescriptor, WrongDatatypeError,
};

#[derive(thiserror::Error, Debug)]
pub enum InvalidChunkSchema {
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

impl InvalidChunkSchema {
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
#[derive(Debug, Clone)]
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

/// ## Metadata keys for the record batch metadata
impl ChunkSchema {
    /// The key used to identify the version of the Rerun schema.
    const CHUNK_METADATA_KEY_VERSION: &'static str = "rerun.version";

    /// The version of the Rerun schema.
    const CHUNK_METADATA_VERSION: &'static str = "1";
}

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
        self.heap_size_bytes
    }

    /// Are we sorted by the row id column?

    #[inline]
    pub fn is_sorted(&self) -> bool {
        self.is_sorted
    }

    pub fn arrow_batch_metadata(&self) -> ArrowBatchMetadata {
        let Self {
            chunk_id,
            entity_path,
            heap_size_bytes,
            is_sorted,
            row_id_column: _,
            columns: _,
        } = self;

        let mut arrow_metadata = ArrowBatchMetadata::from([
            (
                Self::CHUNK_METADATA_KEY_VERSION.to_owned(),
                Self::CHUNK_METADATA_VERSION.to_owned(),
            ),
            ("rerun.id".to_owned(), format!("{:X}", chunk_id.as_u128())),
            ("rerun.entity_path".to_owned(), entity_path.to_string()),
        ]);
        if let Some(heap_size_bytes) = heap_size_bytes {
            arrow_metadata.insert(
                "rerun.heap_size_bytes".to_owned(),
                heap_size_bytes.to_string(),
            );
        }
        if *is_sorted {
            arrow_metadata.insert("rerun.is_sorted".to_owned(), "true".to_owned());
        }

        arrow_metadata
    }
}

impl From<ChunkSchema> for ArrowSchema {
    fn from(chunk_schema: ChunkSchema) -> Self {
        let metadata = chunk_schema.arrow_batch_metadata();

        let ChunkSchema {
            row_id_column,
            columns,
            ..
        } = chunk_schema;
        let mut fields: Vec<ArrowField> = Vec::with_capacity(1 + columns.len());
        fields.push(row_id_column.to_arrow_field());
        fields.extend(columns.iter().map(|column| column.to_arrow_field()));

        Self {
            metadata,
            fields: fields.into(),
        }
    }
}

impl TryFrom<&ArrowSchema> for ChunkSchema {
    type Error = InvalidChunkSchema;

    fn try_from(arrow_schema: &ArrowSchema) -> Result<Self, Self::Error> {
        let ArrowSchema { metadata, fields } = arrow_schema;

        let chunk_id = {
            let chunk_id = metadata.get_or_err("rerun.id")?;
            let chunk_id = u128::from_str_radix(chunk_id, 16).map_err(|err| {
                InvalidChunkSchema::custom(format!(
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

        // Verify version
        if let Some(batch_version) = metadata.get(Self::CHUNK_METADATA_KEY_VERSION) {
            if batch_version != Self::CHUNK_METADATA_VERSION {
                re_log::warn_once!(
                    "ChunkSchema version mismatch. Expected {:?}, got {batch_version:?}",
                    Self::CHUNK_METADATA_VERSION
                );
            }
        }

        // The first field must be the row id column:
        let Some(first_field) = fields.first() else {
            return Err(InvalidChunkSchema::custom("No fields in schema"));
        };

        let row_id_column = RowIdColumnDescriptor::try_from(first_field.as_ref())
            .map_err(InvalidChunkSchema::BadRowIdColumn)?;

        let columns: Result<Vec<_>, _> = fields
            .iter()
            .skip(1)
            .map(|field| {
                ComponentColumnDescriptor::try_from(field.as_ref()).map_err(|err| {
                    InvalidChunkSchema::BadColumn {
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
