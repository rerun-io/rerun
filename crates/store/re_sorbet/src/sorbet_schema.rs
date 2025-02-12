use arrow::datatypes::{Field as ArrowField, Fields as ArrowFields, Schema as ArrowSchema};

use re_log_types::EntityPath;
use re_types_core::ChunkId;

use crate::{
    ArrowBatchMetadata, ColumnError, ComponentColumnDescriptor, IndexColumnDescriptor,
    MetadataExt as _, RowIdColumnDescriptor,
};

#[derive(thiserror::Error, Debug)]
pub enum InvalidSorbetSchema {
    #[error(transparent)]
    MissingMetadataKey(#[from] crate::MissingMetadataKey),

    #[error(transparent)]
    MissingFieldMetadata(#[from] crate::MissingFieldMetadata),

    #[error(transparent)]
    UnsupportedTimeType(#[from] crate::UnsupportedTimeType),

    #[error(transparent)]
    WrongDatatypeError(#[from] crate::WrongDatatypeError),

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

impl InvalidSorbetSchema {
    pub fn custom(reason: impl Into<String>) -> Self {
        Self::Custom {
            reason: reason.into(),
        }
    }
}

// ----------------------------------------------------------------------------

pub enum ColumnKind {
    RowId,
    Index,
    Component,
}

impl TryFrom<&ArrowField> for ColumnKind {
    type Error = InvalidSorbetSchema;

    fn try_from(fields: &ArrowField) -> Result<Self, Self::Error> {
        let kind = fields.get_or_err("rerun.kind")?;
        match kind {
            "control" | "row_id" => Ok(Self::RowId),
            "index" | "time" => Ok(Self::Index),
            "component" | "data" => Ok(Self::Component),

            _ => Err(InvalidSorbetSchema::custom(format!(
                "Unknown column kind: {kind}"
            ))),
        }
    }
}

// ----------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SorbetColumnDescriptors {
    /// The primary row id column.
    /// If present, it is always the first column.
    pub row_id: Option<RowIdColumnDescriptor>,

    /// Index columns (timelines).
    pub indices: Vec<IndexColumnDescriptor>,

    /// The actual component data
    pub components: Vec<ComponentColumnDescriptor>,
}

impl SorbetColumnDescriptors {
    /// Total number of columns in this chunk,
    /// including the row id column, the index columns,
    /// and the data columns.
    pub fn num_columns(&self) -> usize {
        let Self {
            row_id,
            indices,
            components,
        } = self;
        row_id.is_some() as usize + indices.len() + components.len()
    }

    pub fn arrow_fields(&self) -> Vec<ArrowField> {
        let Self {
            row_id,
            indices,
            components,
        } = self;
        let mut fields: Vec<ArrowField> = Vec::with_capacity(self.num_columns());
        if let Some(row_id) = row_id {
            fields.push(row_id.to_arrow_field());
        }
        fields.extend(indices.iter().map(|column| column.to_arrow_field()));
        fields.extend(
            components
                .iter()
                .map(|column| column.to_arrow_field(crate::BatchType::Chunk)),
        );
        fields
    }
}

impl SorbetColumnDescriptors {
    fn try_from_arrow_fields(
        chunk_entity_path: Option<&EntityPath>,
        fields: &ArrowFields,
    ) -> Result<Self, InvalidSorbetSchema> {
        let mut row_ids = Vec::new();
        let mut indices = Vec::new();
        let mut components = Vec::new();

        for field in fields {
            let field = field.as_ref();
            let column_kind = ColumnKind::try_from(field)?;
            match column_kind {
                ColumnKind::RowId => {
                    if indices.is_empty() && components.is_empty() {
                        row_ids.push(RowIdColumnDescriptor::try_from(field)?);
                    } else {
                        return Err(InvalidSorbetSchema::custom(
                            "RowId column must be the first column",
                        ));
                    }
                }

                ColumnKind::Index => {
                    if components.is_empty() {
                        indices.push(IndexColumnDescriptor::try_from(field)?);
                    } else {
                        return Err(InvalidSorbetSchema::custom(
                            "Index columns must come before any data columns",
                        ));
                    }
                }

                ColumnKind::Component => {
                    components.push(ComponentColumnDescriptor::from_arrow_field(
                        chunk_entity_path,
                        field,
                    ));
                }
            }
        }

        if row_ids.len() > 1 {
            return Err(InvalidSorbetSchema::custom(
                "Multiple row_id columns are not supported",
            ));
        }

        Ok(Self {
            row_id: row_ids.pop(),
            indices,
            components,
        })
    }
}

// ----------------------------------------------------------------------------

/// The parsed schema of a `SorbetBatch`.
///
/// This does NOT contain custom arrow metadata.
/// It only contains the metadata used by Rerun.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SorbetSchema {
    pub columns: SorbetColumnDescriptors,

    /// The globally unique ID of this chunk,
    /// if this is a chunk.
    pub chunk_id: Option<ChunkId>,

    /// Which entity is this chunk for?
    pub entity_path: Option<EntityPath>,

    /// The heap size of this batch in bytes, if known.
    pub heap_size_bytes: Option<u64>,

    /// Are we sorted by the row id column?
    pub is_sorted: bool, // TODO(emilk): move to `RowIdColumnDescriptor`.
}

/// ## Metadata keys for the record batch metadata
impl SorbetSchema {
    /// The key used to identify the version of the Rerun schema.
    const METADATA_KEY_VERSION: &'static str = "rerun.version";

    /// The version of the Rerun schema.
    const METADATA_VERSION: &'static str = "1";
}

impl SorbetSchema {
    #[inline]
    pub fn with_heap_size_bytes(mut self, heap_size_bytes: u64) -> Self {
        self.heap_size_bytes = Some(heap_size_bytes);
        self
    }

    #[inline]
    pub fn with_sorted(mut self, sorted_by_row_id: bool) -> Self {
        self.is_sorted = sorted_by_row_id;
        self
    }

    pub fn chunk_id_metadata(chunk_id: &ChunkId) -> (String, String) {
        ("rerun.id".to_owned(), format!("{:X}", chunk_id.as_u128()))
    }

    pub fn entity_path_metadata(entity_path: &EntityPath) -> (String, String) {
        ("rerun.entity_path".to_owned(), entity_path.to_string())
    }

    pub fn arrow_batch_metadata(&self) -> ArrowBatchMetadata {
        let Self {
            columns: _,
            chunk_id,
            entity_path,
            heap_size_bytes,
            is_sorted,
        } = self;

        [
            Some((
                Self::METADATA_KEY_VERSION.to_owned(),
                Self::METADATA_VERSION.to_owned(),
            )),
            chunk_id.as_ref().map(Self::chunk_id_metadata),
            entity_path.as_ref().map(Self::entity_path_metadata),
            heap_size_bytes.as_ref().map(|heap_size_bytes| {
                (
                    "rerun.heap_size_bytes".to_owned(),
                    heap_size_bytes.to_string(),
                )
            }),
            is_sorted.then(|| ("rerun.is_sorted".to_owned(), "true".to_owned())),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

impl TryFrom<&ArrowSchema> for SorbetSchema {
    type Error = InvalidSorbetSchema;

    fn try_from(arrow_schema: &ArrowSchema) -> Result<Self, Self::Error> {
        let ArrowSchema { metadata, fields } = arrow_schema;

        let entity_path = metadata
            .get("rerun.entity_path")
            .map(|s| EntityPath::parse_forgiving(s));

        let columns = SorbetColumnDescriptors::try_from_arrow_fields(entity_path.as_ref(), fields)?;

        let chunk_id = if let Some(chunk_id_str) = metadata.get("rerun.id") {
            Some(chunk_id_str.parse().map_err(|err| {
                InvalidSorbetSchema::custom(format!(
                    "Failed to deserialize chunk id {chunk_id_str:?}: {err}"
                ))
            })?)
        } else {
            None
        };

        let sorted_by_row_id = metadata.get_bool("rerun.is_sorted");
        let heap_size_bytes = if let Some(heap_size_bytes) = metadata.get("rerun.heap_size_bytes") {
            heap_size_bytes
                .parse()
                .map_err(|err| {
                    re_log::warn_once!(
                        "Failed to parse heap_size_bytes {heap_size_bytes:?} in chunk: {err}"
                    );
                })
                .ok()
        } else {
            None
        };

        // Verify version
        if let Some(batch_version) = metadata.get(Self::METADATA_KEY_VERSION) {
            if batch_version != Self::METADATA_VERSION {
                re_log::warn_once!(
                    "Sorbet batch version mismatch. Expected {:?}, got {batch_version:?}",
                    Self::METADATA_VERSION
                );
            }
        }

        Ok(Self {
            columns,
            chunk_id,
            entity_path,
            heap_size_bytes,
            is_sorted: sorted_by_row_id,
        })
    }
}
