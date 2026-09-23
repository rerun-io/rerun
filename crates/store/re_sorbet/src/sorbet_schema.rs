use std::collections::BTreeSet;

use arrow::datatypes::{Schema as ArrowSchema, SchemaRef as ArrowSchemaRef};
use re_log_types::EntityPath;
use re_types_core::{ChunkId, SegmentId};

use crate::{
    ArrowBatchMetadata, LatencyMetadata, SorbetColumnDescriptors, SorbetError, migrate_schema_ref,
};

// ----------------------------------------------------------------------------

/// The parsed schema of a `SorbetBatch`.
///
/// This does NOT contain custom arrow metadata.
/// It only contains the metadata used by Rerun.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SorbetSchema {
    pub(crate) columns: SorbetColumnDescriptors,

    /// The globally unique ID of this chunk,
    /// if this is a chunk.
    pub(crate) chunk_id: Option<ChunkId>,

    /// Which entity is this chunk for?
    pub(crate) entity_path: Option<EntityPath>,

    /// The segment id that this chunk belongs to.
    pub(crate) segment_id: Option<SegmentId>,

    /// Timing statistics.
    pub(crate) latency_metadata: LatencyMetadata,
}

/// ## Accessors
impl SorbetSchema {
    #[inline]
    pub fn columns(&self) -> &SorbetColumnDescriptors {
        &self.columns
    }

    /// The globally unique ID of this chunk, if this is a chunk.
    #[inline]
    pub fn chunk_id(&self) -> Option<ChunkId> {
        self.chunk_id
    }

    /// Which entity is this chunk for?
    #[inline]
    pub fn entity_path(&self) -> Option<&EntityPath> {
        self.entity_path.as_ref()
    }

    /// The segment this chunk belongs to, if known.
    #[inline]
    pub fn segment_id(&self) -> Option<&SegmentId> {
        self.segment_id.as_ref()
    }

    /// Latency-measurement timestamps of the pipeline stages this batch has passed.
    #[inline]
    pub fn latency_metadata(&self) -> &LatencyMetadata {
        &self.latency_metadata
    }

    /// Puts the columns in a canonical order, so that equality does not depend on the column order of the source.
    pub fn sort_columns(&mut self) {
        self.columns.columns.sort();
    }
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

/// The Rerun record batch metadata implied by these fields.
///
/// Shared by [`SorbetSchema`] and [`crate::ChunkSchema`] so both encode the same keys.
pub fn arrow_batch_metadata(
    chunk_id: Option<&ChunkId>,
    entity_path: Option<&EntityPath>,
    segment_id: Option<&SegmentId>,
    latency_metadata: &LatencyMetadata,
) -> ArrowBatchMetadata {
    fn chunk_id_metadata(chunk_id: &ChunkId) -> (String, String) {
        (
            crate::metadata::RERUN_CHUNK_ID.to_owned(),
            chunk_id.to_string(),
        )
    }

    fn entity_path_metadata(entity_path: &EntityPath) -> (String, String) {
        (
            crate::metadata::SORBET_ENTITY_PATH.to_owned(),
            entity_path.to_string(),
        )
    }

    fn segment_id_metadata(segment_id: impl AsRef<str>) -> (String, String) {
        (
            "rerun:segment_id".to_owned(),
            segment_id.as_ref().to_owned(),
        )
    }

    std::iter::chain(
        [
            Some((
                SorbetSchema::METADATA_KEY_VERSION.to_owned(),
                SorbetSchema::METADATA_VERSION.to_string(),
            )),
            chunk_id.map(chunk_id_metadata),
            entity_path.map(entity_path_metadata),
            segment_id.map(segment_id_metadata),
        ]
        .into_iter()
        .flatten(),
        latency_metadata.to_metadata(),
    )
    .collect()
}

impl SorbetSchema {
    pub fn arrow_batch_metadata(&self) -> ArrowBatchMetadata {
        let Self {
            columns: _,
            chunk_id,
            entity_path,
            segment_id,
            latency_metadata,
        } = self;

        arrow_batch_metadata(
            chunk_id.as_ref(),
            entity_path.as_ref(),
            segment_id.as_ref(),
            latency_metadata,
        )
    }

    /// All the entities referenced by any column.
    pub fn all_entities(&self) -> BTreeSet<&EntityPath> {
        std::iter::chain(
            self.columns.iter().filter_map(|c| c.entity_path()),
            self.entity_path.iter(),
        )
        .collect()
    }
}

impl re_byte_size::SizeBytes for SorbetSchema {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            columns,
            chunk_id: _,
            entity_path,
            segment_id,
            latency_metadata,
        } = self;

        columns.heap_size_bytes()
            + entity_path.heap_size_bytes()
            + segment_id.heap_size_bytes()
            + latency_metadata.heap_size_bytes()
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

        let chunk_id = if let Some(chunk_id_str) = metadata.get(crate::metadata::RERUN_CHUNK_ID) {
            Some(chunk_id_str.parse().map_err(|err| {
                SorbetError::ChunkIdDeserializationError(format!(
                    "Failed to deserialize chunk id {chunk_id_str:?}: {err}"
                ))
            })?)
        } else {
            None
        };

        // Support both new "rerun:segment_id" and legacy "rerun:partition_id" keys
        let segment_id = metadata
            .get("rerun:segment_id")
            .or_else(|| metadata.get("rerun:partition_id"))
            .map(|s| SegmentId::from(s.as_str()));

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
            latency_metadata: LatencyMetadata::parse_record_batch_metadata(metadata),
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
            Some(partition_id_value.into()),
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
            Some(segment_id_value.into()),
            "rerun:segment_id should take precedence over rerun:partition_id"
        );
    }
}
