use arrow::datatypes::Schema as ArrowSchema;

use re_log_types::EntityPath;
use re_types_core::ChunkId;

use crate::{ArrowBatchMetadata, SorbetColumnDescriptors, SorbetError};

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

    pub fn chunk_id_metadata(chunk_id: &ChunkId) -> (String, String) {
        ("rerun.id".to_owned(), chunk_id.to_string())
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
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

impl From<SorbetSchema> for SorbetColumnDescriptors {
    #[inline]
    fn from(sorbet_schema: SorbetSchema) -> Self {
        sorbet_schema.columns
    }
}

impl SorbetSchema {
    pub fn to_arrow(&self, batch_type: crate::BatchType) -> ArrowSchema {
        ArrowSchema {
            metadata: self.arrow_batch_metadata(),
            fields: self.columns.arrow_fields(batch_type).into(),
        }
    }
}

impl TryFrom<&ArrowSchema> for SorbetSchema {
    type Error = SorbetError;

    fn try_from(arrow_schema: &ArrowSchema) -> Result<Self, Self::Error> {
        let ArrowSchema { metadata, fields } = arrow_schema;

        let entity_path = metadata
            .get("rerun.entity_path")
            .map(|s| EntityPath::parse_forgiving(s));

        let columns = SorbetColumnDescriptors::try_from_arrow_fields(entity_path.as_ref(), fields)?;

        let chunk_id = if let Some(chunk_id_str) = metadata.get("rerun.id") {
            Some(chunk_id_str.parse().map_err(|err| {
                SorbetError::custom(format!(
                    "Failed to deserialize chunk id {chunk_id_str:?}: {err}"
                ))
            })?)
        } else {
            None
        };

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
        })
    }
}
