use arrow::datatypes::Schema as ArrowSchema;

use re_log_types::EntityPath;
use re_types_core::ChunkId;

use crate::{ArrowBatchMetadata, SorbetColumnDescriptors, SorbetError, TimestampMetadata};

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

    /// The partition id that this chunk belongs to.
    pub partition_id: Option<String>,

    /// The heap size of this batch in bytes, if known.
    pub heap_size_bytes: Option<u64>,

    /// Timing statistics.
    pub timestamps: TimestampMetadata,
}

/// ## Metadata keys for the record batch metadata
impl SorbetSchema {
    /// The key used to identify the version of the Rerun schema.
    pub(crate) const METADATA_KEY_VERSION: &'static str = "sorbet:version";

    /// The version of the Sorbet schema.
    ///
    /// This is bumped everytime we require a migration, but notable it is
    /// decoupled from the Rerun version to avoid confusion as there will not
    /// be a new Sorbet version for each Rerun version.
    pub(crate) const METADATA_VERSION: semver::Version = semver::Version::new(0, 1, 2);
}

impl SorbetSchema {
    #[inline]
    pub fn with_heap_size_bytes(mut self, heap_size_bytes: u64) -> Self {
        self.heap_size_bytes = Some(heap_size_bytes);
        self
    }

    pub fn chunk_id_metadata(chunk_id: &ChunkId) -> (String, String) {
        ("rerun:id".to_owned(), chunk_id.to_string())
    }

    pub fn entity_path_metadata(entity_path: &EntityPath) -> (String, String) {
        ("rerun:entity_path".to_owned(), entity_path.to_string())
    }

    pub fn partition_id_metadata(partition_id: impl AsRef<str>) -> (String, String) {
        (
            "rerun:partition_id".to_owned(),
            partition_id.as_ref().to_owned(),
        )
    }

    pub fn arrow_batch_metadata(&self) -> ArrowBatchMetadata {
        let Self {
            columns: _,
            chunk_id,
            entity_path,
            heap_size_bytes,
            partition_id,
            timestamps,
        } = self;

        [
            Some((
                Self::METADATA_KEY_VERSION.to_owned(),
                Self::METADATA_VERSION.to_string(),
            )),
            chunk_id.as_ref().map(Self::chunk_id_metadata),
            entity_path.as_ref().map(Self::entity_path_metadata),
            partition_id.as_ref().map(Self::partition_id_metadata),
            heap_size_bytes.as_ref().map(|heap_size_bytes| {
                (
                    "rerun:heap_size_bytes".to_owned(),
                    heap_size_bytes.to_string(),
                )
            }),
        ]
        .into_iter()
        .flatten()
        .chain(timestamps.to_metadata())
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

impl SorbetSchema {
    /// Parse an already migrated Arrow schema.
    #[tracing::instrument(level = "trace", skip_all)]
    pub(crate) fn try_from_migrated_arrow_schema(
        arrow_schema: &ArrowSchema,
    ) -> Result<Self, SorbetError> {
        debug_assert!(
            !arrow_schema.metadata.contains_key("rerun.id"),
            "The schema should not contain the legacy 'rerun.id' key, because it should have already been migrated to 'rerun:id'."
        );

        let ArrowSchema { metadata, fields } = arrow_schema;

        let entity_path = metadata
            .get("rerun:entity_path")
            .map(|s| EntityPath::parse_forgiving(s));

        let columns = SorbetColumnDescriptors::try_from_arrow_fields(entity_path.as_ref(), fields)?;

        let chunk_id = if let Some(chunk_id_str) = metadata.get("rerun:id") {
            Some(chunk_id_str.parse().map_err(|err| {
                SorbetError::ChunkIdDeserializationError(format!(
                    "Failed to deserialize chunk id {chunk_id_str:?}: {err}"
                ))
            })?)
        } else {
            None
        };

        let heap_size_bytes = if let Some(heap_size_bytes) = metadata.get("rerun:heap_size_bytes") {
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

        let partition_id = metadata.get("rerun:partition_id").map(|s| s.to_owned());

        // Verify version
        if let Some(batch_version) = metadata.get(Self::METADATA_KEY_VERSION)
            && batch_version != &Self::METADATA_VERSION.to_string()
        {
            re_log::warn_once!(
                "Sorbet batch version mismatch. Expected {}, got {batch_version:?}",
                Self::METADATA_VERSION
            );
        }

        Ok(Self {
            columns,
            chunk_id,
            entity_path,
            partition_id,
            heap_size_bytes,
            timestamps: TimestampMetadata::parse_record_batch_metadata(metadata),
        })
    }
}
