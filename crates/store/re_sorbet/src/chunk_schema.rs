use arrow::datatypes::{Field as ArrowField, Schema as ArrowSchema};

use itertools::Itertools as _;
use re_log_types::EntityPath;
use re_types_core::ChunkId;

use crate::{
    ArrowBatchMetadata, ColumnDescriptor, ColumnError, ComponentColumnDescriptor, MetadataExt as _,
    MissingMetadataKey, RowIdColumnDescriptor, TimeColumnDescriptor, WrongDatatypeError,
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
        error: ColumnError,
    },

    #[error("Bad chunk schema: {reason}")]
    Custom { reason: String },

    #[error("The data columns were not the last columns. Index columns must come before any data columns.")]
    UnorderedIndexAndDataColumns,
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
    pub chunk_id: ChunkId,

    /// Which entity is this chunk for?
    pub entity_path: EntityPath,

    /// The heap size of this chunk in bytes, if known.
    pub heap_size_bytes: Option<u64>,

    /// Are we sorted by the row id column?
    pub is_sorted: bool,

    /// The primary row id column.
    pub row_id_column: RowIdColumnDescriptor,

    /// Index columns (timelines).
    pub index_columns: Vec<TimeColumnDescriptor>,

    /// The actual component data
    pub data_columns: Vec<ComponentColumnDescriptor>,
}

/// ## Metadata keys for the record batch metadata
impl ChunkSchema {
    /// The key used to identify the version of the Rerun schema.
    const CHUNK_METADATA_KEY_VERSION: &'static str = "rerun.version";

    /// The version of the Rerun schema.
    const CHUNK_METADATA_VERSION: &'static str = "1";
}

/// ## Builders
impl ChunkSchema {
    pub fn new(
        chunk_id: ChunkId,
        entity_path: EntityPath,
        row_id_column: RowIdColumnDescriptor,
        index_columns: Vec<TimeColumnDescriptor>,
        data_columns: Vec<ComponentColumnDescriptor>,
    ) -> Self {
        Self {
            chunk_id,
            entity_path,
            heap_size_bytes: None,
            is_sorted: false, // assume the worst
            row_id_column,
            index_columns,
            data_columns,
        }
    }

    #[inline]
    pub fn with_heap_size_bytes(mut self, heap_size_bytes: u64) -> Self {
        self.heap_size_bytes = Some(heap_size_bytes);
        self
    }

    #[inline]
    pub fn with_sorted(mut self, sorted: bool) -> Self {
        self.is_sorted = sorted;
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
        self.heap_size_bytes
    }

    /// Are we sorted by the row id column?
    #[inline]
    pub fn is_sorted(&self) -> bool {
        self.is_sorted
    }

    /// Total number of columns in this chunk,
    /// including the row id column, the index columns,
    /// and the data columns.
    pub fn num_columns(&self) -> usize {
        1 + self.index_columns.len() + self.data_columns.len()
    }

    pub fn arrow_batch_metadata(&self) -> ArrowBatchMetadata {
        let Self {
            chunk_id,
            entity_path,
            heap_size_bytes,
            is_sorted,
            row_id_column: _,
            index_columns: _,
            data_columns: _,
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

impl From<&ChunkSchema> for ArrowSchema {
    fn from(chunk_schema: &ChunkSchema) -> Self {
        let metadata = chunk_schema.arrow_batch_metadata();
        let num_columns = chunk_schema.num_columns();

        let ChunkSchema {
            row_id_column,
            index_columns,
            data_columns,
            ..
        } = chunk_schema;

        let mut fields: Vec<ArrowField> = Vec::with_capacity(num_columns);
        fields.push(row_id_column.to_arrow_field());
        fields.extend(index_columns.iter().map(|column| column.to_arrow_field()));
        fields.extend(data_columns.iter().map(|column| column.to_arrow_field()));

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
                ColumnDescriptor::try_from(field.as_ref()).map_err(|err| {
                    InvalidChunkSchema::BadColumn {
                        field_name: field.name().to_owned(),
                        error: err,
                    }
                })
            })
            .collect();
        let columns = columns?;

        // Index columns should always come first:
        let num_index_columns = columns.partition_point(|p| matches!(p, ColumnDescriptor::Time(_)));

        let index_columns = columns[0..num_index_columns]
            .iter()
            .filter_map(|c| match c {
                ColumnDescriptor::Time(column) => Some(column.clone()),
                ColumnDescriptor::Component(_) => None,
            })
            .collect_vec();
        let data_columns = columns[0..num_index_columns]
            .iter()
            .filter_map(|c| match c {
                ColumnDescriptor::Time(_) => None,
                ColumnDescriptor::Component(column) => Some(column.clone()),
            })
            .collect_vec();

        if index_columns.len() + data_columns.len() < columns.len() {
            return Err(InvalidChunkSchema::UnorderedIndexAndDataColumns);
        }

        Ok(Self {
            chunk_id,
            entity_path,
            heap_size_bytes,
            is_sorted,
            row_id_column,
            index_columns,
            data_columns,
        })
    }
}
