use arrow::datatypes::{Schema as ArrowSchema, SchemaRef as ArrowSchemaRef};
use re_log_types::EntityPath;
use re_types_core::ChunkId;

use crate::{
    ArrowBatchMetadata, SorbetColumnDescriptors, SorbetError, TimestampMetadata, migrate_schema_ref,
};

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

    /// The segment id that this chunk belongs to.
    pub segment_id: Option<String>,

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
    pub(crate) const METADATA_VERSION: semver::Version = semver::Version::new(0, 1, 3);
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
        (
            crate::metadata::SORBET_ENTITY_PATH.to_owned(),
            entity_path.to_string(),
        )
    }

    pub fn segment_id_metadata(segment_id: impl AsRef<str>) -> (String, String) {
        (
            "rerun:segment_id".to_owned(),
            segment_id.as_ref().to_owned(),
        )
    }

    pub fn arrow_batch_metadata(&self) -> ArrowBatchMetadata {
        let Self {
            columns: _,
            chunk_id,
            entity_path,
            heap_size_bytes,
            segment_id,
            timestamps,
        } = self;

        [
            Some((
                Self::METADATA_KEY_VERSION.to_owned(),
                Self::METADATA_VERSION.to_string(),
            )),
            chunk_id.as_ref().map(Self::chunk_id_metadata),
            entity_path.as_ref().map(Self::entity_path_metadata),
            segment_id.as_ref().map(Self::segment_id_metadata),
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
    /// Parse an arbitrary arrow schema by first migrating it to the Rerun schema.
    pub fn try_from_raw_arrow_schema(arrow_schema: ArrowSchemaRef) -> Result<Self, SorbetError> {
        Self::try_from_migrated_arrow_schema(&migrate_schema_ref(arrow_schema))
    }

    /// Parse an already migrated Arrow schema.
    #[tracing::instrument(level = "trace", skip_all)]
    pub(crate) fn try_from_migrated_arrow_schema(
        arrow_schema: &ArrowSchema,
    ) -> Result<Self, SorbetError> {
        re_log::debug_assert!(
            !arrow_schema.metadata.contains_key("rerun.id"),
            "The schema should not contain the legacy 'rerun.id' key, because it should have already been migrated to 'rerun:id'."
        );

        let ArrowSchema { metadata, fields } = arrow_schema;

        let entity_path = metadata
            .get(crate::metadata::SORBET_ENTITY_PATH)
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

        // Support both new "rerun:segment_id" and legacy "rerun:partition_id" keys
        let segment_id = metadata
            .get("rerun:segment_id")
            .or_else(|| metadata.get("rerun:partition_id"))
            .map(|s| s.to_owned());

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
            segment_id,
            heap_size_bytes,
            timestamps: TimestampMetadata::parse_record_batch_metadata(metadata),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arrow::datatypes::Schema as ArrowSchema;

    use super::*;
    use crate::RowIdColumnDescriptor;

    /// Test that the legacy `rerun:partition_id` metadata key is correctly read as `segment_id`.
    #[test]
    fn test_legacy_partition_id_backward_compatibility() {
        let partition_id_value = "test-partition-123";

        // Create an Arrow schema with the legacy "rerun:partition_id" metadata key
        let row_id_field = RowIdColumnDescriptor::from_sorted(false).to_arrow_field();
        let fields = vec![Arc::new(row_id_field)];
        let arrow_schema = ArrowSchema::new_with_metadata(
            fields,
            std::iter::once((
                "rerun:partition_id".to_owned(),
                partition_id_value.to_owned(),
            ))
            .collect(),
        );

        // Parse the schema
        let sorbet_schema = SorbetSchema::try_from_migrated_arrow_schema(&arrow_schema).unwrap();

        // Verify that segment_id is correctly populated from the legacy partition_id
        assert_eq!(
            sorbet_schema.segment_id,
            Some(partition_id_value.to_owned()),
            "Legacy rerun:partition_id should be read as segment_id"
        );
    }

    /// Test that the new `rerun:segment_id` metadata key takes precedence over legacy `rerun:partition_id`.
    #[test]
    fn test_segment_id_takes_precedence_over_partition_id() {
        let segment_id_value = "new-segment-456";
        let partition_id_value = "old-partition-123";

        // Create an Arrow schema with both keys - segment_id should take precedence
        let row_id_field = RowIdColumnDescriptor::from_sorted(false).to_arrow_field();
        let fields = vec![Arc::new(row_id_field)];
        let arrow_schema = ArrowSchema::new_with_metadata(
            fields,
            [
                ("rerun:segment_id".to_owned(), segment_id_value.to_owned()),
                (
                    "rerun:partition_id".to_owned(),
                    partition_id_value.to_owned(),
                ),
            ]
            .into_iter()
            .collect(),
        );

        // Parse the schema
        let sorbet_schema = SorbetSchema::try_from_migrated_arrow_schema(&arrow_schema).unwrap();

        // Verify that segment_id takes precedence
        assert_eq!(
            sorbet_schema.segment_id,
            Some(segment_id_value.to_owned()),
            "rerun:segment_id should take precedence over rerun:partition_id"
        );
    }
}
