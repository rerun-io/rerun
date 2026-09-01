use std::sync::Arc;

use arrow::array::RecordBatch;
use re_chunk::external::re_byte_size;
use re_chunk::{ChunkId, EntityPath};
use re_log_types::StoreId;
use re_sorbet::SorbetSchema;

use super::{HubRrdManifest, RawRrdManifest, RrdManifestStaticMap, RrdManifestTemporalMap};
use crate::{CodecError, CodecResult};

/// Concatenates one typed column per manifest, keeping the logical type and the column name.
fn concat_columns<'a, L: quiver::LogicalType + 'a>(
    columns: impl IntoIterator<Item = &'a quiver::Column<L>>,
) -> CodecResult<quiver::Column<L>> {
    let columns = columns.into_iter().collect::<Vec<_>>();

    let Some(first_column) = columns.first() else {
        return Err(CodecError::FrameDecoding(
            "concat_columns: no columns to concatenate".to_owned(),
        ));
    };
    // Each input is the same column of a different manifest, so they should all carry the same name.
    let name = first_column.name().to_owned();

    let arrays: Vec<&dyn arrow::array::Array> = columns
        .iter()
        .map(|column| column.as_arrow().as_ref())
        .collect();

    let concatenated = re_arrow_util::concat_arrays(&arrays)
        .map_err(|err| CodecError::FrameDecoding(format!("concat {name:?}: {err}")))?;

    quiver::Column::try_new(name.as_str(), concatenated)
        .map_err(|err| CodecError::FrameDecoding(format!("concat {name:?}: {err}")))
}

/// The heap size of a column's arrow array.
fn column_heap_size_bytes<L: quiver::LogicalType>(column: &quiver::Column<L>) -> u64 {
    re_byte_size::SizeBytes::heap_size_bytes(column.as_arrow().as_ref())
}

/// A pre-validated and parsed [`RawRrdManifest`].
///
/// This struct provides a more ergonomic interface to access manifest data without
/// having to handle `CodecResult` errors on every access. All validation and column
/// extraction is performed during construction.
///
/// The Arrow arrays stored here are clones of those in the underlying manifest,
/// but since Arrow uses `Arc` internally, this is just a reference count increment
/// and does not duplicate the actual data.
///
/// Use [`RrdManifest::try_new`] to create an instance from a [`RawRrdManifest`].
#[derive(Clone)]
pub struct RrdManifest {
    // NOTE: the `chunk_fetcher_rb` only contains the columns listed in
    // [`Self::CHUNK_FETCHER_COLUMNS`]. All other manifest columns are pre-extracted
    // into the typed fields below (or into the static/temporal maps).
    chunk_fetcher_rb: RecordBatch,

    store_id: StoreId,
    recording_schema: SorbetSchema,
    sorbet_schema: arrow::datatypes::Schema,

    /// Hash of `sorbet_schema`, to compare two schemas without walking their fields.
    /// See [`RawRrdManifest::compute_sorbet_schema_sha256`].
    sorbet_schema_sha256: [u8; 32],

    chunk_ids: quiver::Column<ChunkId>,
    chunk_entity_paths: quiver::Column<EntityPath>,
    chunk_is_static: quiver::Column<bool>,
    chunk_num_rows: quiver::Column<u64>,
    chunk_byte_offsets: quiver::Column<u64>,
    chunk_byte_sizes: quiver::Column<u64>,
    chunk_byte_sizes_uncompressed: quiver::Column<u64>,

    /// The values are optional: merging a keyed manifest with an unkeyed one leaves nulls behind.
    chunk_keys: Option<quiver::Column<Option<quiver::Binary>>>,

    static_data_map: RrdManifestStaticMap,
    temporal_data_map: RrdManifestTemporalMap,
}

impl PartialEq for RrdManifest {
    fn eq(&self, other: &Self) -> bool {
        // Destructure to get a compile error when new fields are added,
        // ensuring we consciously decide whether to include them.
        let Self {
            chunk_fetcher_rb,
            store_id,
            recording_schema,
            // We skip `sorbet_schema` (the raw `arrow::datatypes::Schema`) because it is
            // redundant with `recording_schema` for semantic equality, and its field order
            // is not preserved through protobuf round-trips. Its hash is skipped along with it.
            sorbet_schema: _,
            sorbet_schema_sha256: _,
            chunk_ids,
            chunk_entity_paths,
            chunk_is_static,
            chunk_num_rows,
            chunk_byte_offsets,
            chunk_byte_sizes,
            chunk_byte_sizes_uncompressed,
            chunk_keys,
            static_data_map,
            temporal_data_map,
        } = self;

        *chunk_fetcher_rb == other.chunk_fetcher_rb
            && *store_id == other.store_id
            && *recording_schema == other.recording_schema
            && *chunk_ids == other.chunk_ids
            && *chunk_entity_paths == other.chunk_entity_paths
            && *chunk_is_static == other.chunk_is_static
            && *chunk_num_rows == other.chunk_num_rows
            && *chunk_byte_offsets == other.chunk_byte_offsets
            && *chunk_byte_sizes == other.chunk_byte_sizes
            && *chunk_byte_sizes_uncompressed == other.chunk_byte_sizes_uncompressed
            && *chunk_keys == other.chunk_keys
            && *static_data_map == other.static_data_map
            && *temporal_data_map == other.temporal_data_map
    }
}

impl std::fmt::Debug for RrdManifest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RrdManifest").finish_non_exhaustive()
    }
}

impl re_byte_size::SizeBytes for RrdManifest {
    fn heap_size_bytes(&self) -> u64 {
        re_tracing::profile_function!();

        // After `try_new`, some extracted arrays (chunk_ids, chunk_is_static, …) share their
        // underlying `Arc<Buffer>` with the pruned `RecordBatch` columns, so they are already
        // covered by `chunk_fetcher_rb.heap_size_bytes()`. However, after `merge` all arrays
        // are independently allocated, so the pruned-batch size alone would undercount.
        // We intentionally accept that minor double-count (via Arc sharing) from `try_new`
        // in exchange for always being correct after `merge`.
        //
        // Fields that are never in the pruned batch must always be counted separately:
        self.chunk_fetcher_rb.heap_size_bytes()
            + column_heap_size_bytes(&self.chunk_entity_paths)
            + column_heap_size_bytes(&self.chunk_num_rows)
            + column_heap_size_bytes(&self.chunk_byte_sizes)
            + column_heap_size_bytes(&self.chunk_byte_sizes_uncompressed)
            + self.sorbet_schema.heap_size_bytes()
            + self.static_data_map.heap_size_bytes()
            + self.temporal_data_map.heap_size_bytes()
    }
}

// Columns retained in the pruned `chunk_fetcher_rb`.
//
// The full manifest can have 1000+ sparse columns (one per timeline x component pair).
// After extracting all indexing data into typed fields and maps, we prune the
// `RecordBatch` down to only the columns needed for chunk fetching. This list is the
// single source of truth for which columns survive that pruning — it is used by
// [`RawRrdManifest::chunk_fetcher_record_batch`] to do the pruning, and should be
// referenced by any code that accesses the pruned batch (e.g. sorting, sending over gRPC).
impl RrdManifest {
    pub const COLUMN_CHUNK_ID: quiver::ColumnDesc<ChunkId> = RawRrdManifest::COLUMN_CHUNK_ID;
    pub const COLUMN_CHUNK_KEY: quiver::ColumnDesc<quiver::Binary> =
        RawRrdManifest::COLUMN_CHUNK_KEY;
    pub const COLUMN_CHUNK_IS_STATIC: quiver::ColumnDesc<bool> =
        RawRrdManifest::COLUMN_CHUNK_IS_STATIC;
    pub const COLUMN_CHUNK_BYTE_OFFSET: quiver::ColumnDesc<u64> =
        RawRrdManifest::COLUMN_CHUNK_BYTE_OFFSET;
    pub const COLUMN_CHUNK_PARTITION_ID: quiver::ColumnDesc<re_types_core::SegmentId> =
        HubRrdManifest::COLUMN_CHUNK_PARTITION_ID;
    pub const COLUMN_RERUN_PARTITION_LAYER: quiver::ColumnDesc<re_types_core::LayerName> =
        HubRrdManifest::COLUMN_RERUN_PARTITION_LAYER;

    /// All columns present in the pruned batch returned by [`Self::chunk_fetcher_rb()`].
    pub const CHUNK_FETCHER_COLUMNS: &[&str] = &[
        Self::COLUMN_CHUNK_ID.name,
        Self::COLUMN_CHUNK_KEY.name,
        Self::COLUMN_CHUNK_IS_STATIC.name,
        Self::COLUMN_CHUNK_BYTE_OFFSET.name,
        Self::COLUMN_CHUNK_PARTITION_ID.name,
        Self::COLUMN_RERUN_PARTITION_LAYER.name,
    ];
}

impl RrdManifest {
    /// Creates a new [`RrdManifest`].
    ///
    /// This validates the manifest and extracts all columns. If validation fails
    /// or any required column is missing/malformed, an error is returned.
    ///
    /// All arrays must be non-null (no missing values).
    pub fn try_new(manifest: &RawRrdManifest) -> CodecResult<Self> {
        re_tracing::profile_function!();

        if cfg!(debug_assertions) {
            manifest.sanity_check_heavy()?;
        } else {
            manifest.sanity_check_cheap()?;
        }

        let chunk_ids = manifest.col_chunk_id()?;
        let chunk_entity_paths = manifest.col_chunk_entity_path()?;
        let chunk_is_static = manifest.col_chunk_is_static()?;
        let chunk_num_rows = manifest.col_chunk_num_rows()?;
        let chunk_byte_offsets = manifest.col_chunk_byte_offset()?;
        let chunk_byte_sizes = manifest.col_chunk_byte_size()?;
        let chunk_byte_sizes_uncompressed = manifest.col_chunk_byte_size_uncompressed()?;

        // The chunk-key column is optional: local RRDs have no keys, and merging a keyed manifest
        // with an unkeyed one leaves nulls behind. A column that is present but malformed is an
        // error though, not a manifest without keys.
        let chunk_keys = manifest
            .data
            .schema_ref()
            .column_with_name(RawRrdManifest::COLUMN_CHUNK_KEY.name)
            .is_some()
            .then(|| manifest.col_chunk_key())
            .transpose()?;

        let static_data_map = manifest.calc_static_map()?;
        let temporal_data_map = manifest.calc_temporal_map()?;

        let mut recording_schema =
            SorbetSchema::try_from_raw_arrow_schema(Arc::new(manifest.sorbet_schema.clone()))?;
        // Sort columns so that PartialEq is stable across protobuf round-trips,
        // which do not preserve column ordering.
        recording_schema.columns.columns.sort();

        let pruned_batch = manifest.chunk_fetcher_record_batch();

        Ok(Self {
            chunk_fetcher_rb: pruned_batch,
            store_id: manifest.store_id.clone(),
            recording_schema,
            sorbet_schema: manifest.sorbet_schema.clone(),
            sorbet_schema_sha256: manifest.sorbet_schema_sha256,
            chunk_ids,
            chunk_entity_paths,
            chunk_is_static,
            chunk_num_rows,
            chunk_byte_offsets,
            chunk_byte_sizes,
            chunk_byte_sizes_uncompressed,
            chunk_keys,
            static_data_map,
            temporal_data_map,
        })
    }

    /// The schema for the entire recording.
    pub fn recording_schema(&self) -> &SorbetSchema {
        &self.recording_schema
    }

    /// The chunk fetcher batches of every manifest, concatenated in the given order.
    ///
    /// See [`Self::CHUNK_FETCHER_COLUMNS`] for the columns these pruned batches keep.
    fn concat_chunk_fetcher_rb(manifests: &[&Self]) -> CodecResult<RecordBatch> {
        re_tracing::profile_function!();

        let first = manifests
            .first()
            .ok_or_else(|| CodecError::FrameDecoding("No manifests to concatenate".to_owned()))?;

        if manifests.len() == 1 {
            return Ok(first.chunk_fetcher_rb.clone());
        }

        let any_has_chunk_keys = manifests.iter().any(|m| m.chunk_keys.is_some());

        // When some manifests have `chunk_key` and others don't, we must normalize
        // the schemas before calling `concat_batches` (which requires matching schemas).
        let normalized_batches: Vec<RecordBatch>;
        let batches_to_concat: Vec<&RecordBatch> =
            if any_has_chunk_keys && manifests.iter().any(|m| m.chunk_keys.is_none()) {
                // Some have chunk_key, some don't — normalize by adding a null column.
                normalized_batches = manifests
                    .iter()
                    .map(|m| {
                        if m.chunk_keys.is_some() {
                            m.chunk_fetcher_rb.clone()
                        } else {
                            Self::add_null_chunk_key_column(&m.chunk_fetcher_rb)
                        }
                    })
                    .collect();
                normalized_batches.iter().collect()
            } else {
                manifests.iter().map(|m| &m.chunk_fetcher_rb).collect()
            };

        let combined_schema = batches_to_concat
            .first()
            .map(|b| b.schema())
            .unwrap_or_else(|| first.chunk_fetcher_rb.schema());

        arrow::compute::concat_batches(&combined_schema, batches_to_concat).map_err(|err| {
            CodecError::FrameDecoding(format!("Failed to concatenate RRD manifest parts: {err}"))
        })
    }

    /// The schema covering the columns of every manifest, and its hash.
    ///
    /// Manifests that share a schema keep it as it is. Ones that do not have their columns unified,
    /// so the entities and components of all of them can be read off the result. The metadata stays
    /// that of the first manifest.
    ///
    /// Manifests that disagree on the type of a column they both have cannot be unified. The schema
    /// of the first is then used as it is, with a warning.
    fn merge_schemas(
        manifests: &[&Self],
    ) -> CodecResult<(SorbetSchema, arrow::datatypes::Schema, [u8; 32])> {
        let first = manifests
            .first()
            .ok_or_else(|| CodecError::FrameDecoding("No manifests to concatenate".to_owned()))?;

        let schema_of_first = || {
            (
                first.recording_schema.clone(),
                first.sorbet_schema.clone(),
                first.sorbet_schema_sha256,
            )
        };

        if manifests
            .iter()
            .all(|m| m.sorbet_schema_sha256 == first.sorbet_schema_sha256)
        {
            return Ok(schema_of_first());
        }

        re_tracing::profile_function!();

        match Self::unify_columns(manifests) {
            Ok(unified) => Ok(unified),
            Err(err) => {
                re_log::warn_once!(
                    "Failed to merge the schemas of the manifests, using the first one: {err}"
                );
                Ok(schema_of_first())
            }
        }
    }

    /// The columns of every manifest in one schema, with the metadata of the first.
    fn unify_columns(
        manifests: &[&Self],
    ) -> CodecResult<(SorbetSchema, arrow::datatypes::Schema, [u8; 32])> {
        let first = manifests
            .first()
            .ok_or_else(|| CodecError::FrameDecoding("No manifests to concatenate".to_owned()))?;

        // Merge the fields into the first schema instead of using `Schema::try_merge`: that one
        // also merges the schema metadata and errors when two schemas disagree on a key. The
        // metadata comes from the first manifest.
        let mut builder = arrow::datatypes::SchemaBuilder::from(&first.sorbet_schema);
        for manifest in &manifests[1..] {
            for field in manifest.sorbet_schema.fields() {
                builder.try_merge(field).map_err(|err| {
                    CodecError::FrameDecoding(format!("Failed to merge manifest schemas: {err}"))
                })?;
            }
        }
        let sorbet_schema = builder.finish();

        let sorbet_schema_sha256 = RawRrdManifest::compute_sorbet_schema_sha256(&sorbet_schema)
            .map_err(CodecError::ArrowSerialization)?;

        let mut recording_schema =
            SorbetSchema::try_from_raw_arrow_schema(Arc::new(sorbet_schema.clone()))?;
        // Sorted for the same reason as in `try_new`: to keep `PartialEq` stable.
        recording_schema.columns.columns.sort();

        Ok((recording_schema, sorbet_schema, sorbet_schema_sha256))
    }

    /// One manifest describing every chunk of every part, in the given order.
    ///
    /// The parts can be the pieces of one segment's manifest, or the manifests of several
    /// segments. The store id is that of the first part, and the schema covers the columns of all
    /// of them.
    pub fn merge(manifests: &[&Self]) -> CodecResult<Self> {
        re_tracing::profile_function!();

        let first = manifests
            .first()
            .ok_or_else(|| CodecError::FrameDecoding("No manifests to concatenate".to_owned()))?;

        let any_has_chunk_keys = manifests.iter().any(|m| m.chunk_keys.is_some());

        let (recording_schema, sorbet_schema, sorbet_schema_sha256) =
            Self::merge_schemas(manifests)?;

        let combined_batches = Self::concat_chunk_fetcher_rb(manifests)?;

        // Concatenate pre-extracted columns directly, avoiding a round-trip through `try_new`
        // which would fail on pruned data (missing sparse columns).
        let chunk_ids = concat_columns(manifests.iter().map(|m| &m.chunk_ids))?;
        let chunk_entity_paths = concat_columns(manifests.iter().map(|m| &m.chunk_entity_paths))?;
        let chunk_is_static = concat_columns(manifests.iter().map(|m| &m.chunk_is_static))?;
        let chunk_num_rows = concat_columns(manifests.iter().map(|m| &m.chunk_num_rows))?;
        let chunk_byte_offsets = concat_columns(manifests.iter().map(|m| &m.chunk_byte_offsets))?;
        let chunk_byte_sizes = concat_columns(manifests.iter().map(|m| &m.chunk_byte_sizes))?;
        let chunk_byte_sizes_uncompressed =
            concat_columns(manifests.iter().map(|m| &m.chunk_byte_sizes_uncompressed))?;

        // When some manifests have chunk keys and others don't, the keyless ones contribute
        // all-null columns, to keep the rows aligned.
        //
        let null_keys: Vec<quiver::Column<Option<quiver::Binary>>> = manifests
            .iter()
            .filter(|m| m.chunk_keys.is_none())
            .map(|m| {
                RawRrdManifest::COLUMN_CHUNK_KEY
                    .optional()
                    .new_null(m.num_chunks())
            })
            .collect();
        let mut null_keys = null_keys.iter();
        let chunk_keys = any_has_chunk_keys
            .then(|| {
                let columns = manifests.iter().map(|m| {
                    m.chunk_keys
                        .as_ref()
                        .or_else(|| null_keys.next())
                        .ok_or_else(|| {
                            CodecError::FrameDecoding(
                                "concat chunk_keys: row count mismatch".to_owned(),
                            )
                        })
                });
                concat_columns(columns.collect::<CodecResult<Vec<_>>>()?)
            })
            .transpose()?;

        // Merge pre-computed maps.
        let mut static_data_map = first.static_data_map.clone();
        for m in &manifests[1..] {
            for (entity, components) in &m.static_data_map {
                let entry = static_data_map.entry(entity.clone()).or_default();
                for (component, chunk_id) in components {
                    entry
                        .entry(*component)
                        .and_modify(|id| *id = *chunk_id)
                        .or_insert(*chunk_id);
                }
            }
        }

        let mut temporal_data_map = first.temporal_data_map.clone();
        for m in &manifests[1..] {
            for (entity, timelines) in &m.temporal_data_map {
                let entity_entry = temporal_data_map.entry(entity.clone()).or_default();
                for (timeline, components) in timelines {
                    let timeline_entry = entity_entry.entry(*timeline).or_default();
                    for (component, chunks) in components {
                        let component_entry = timeline_entry.entry(*component).or_default();
                        for (chunk_id, map_entry) in chunks {
                            component_entry.insert(*chunk_id, *map_entry);
                        }
                    }
                }
            }
        }

        Ok(Self {
            chunk_fetcher_rb: combined_batches,
            store_id: first.store_id.clone(),
            recording_schema,
            sorbet_schema,
            sorbet_schema_sha256,
            chunk_ids,
            chunk_entity_paths,
            chunk_is_static,
            chunk_num_rows,
            chunk_byte_offsets,
            chunk_byte_sizes,
            chunk_byte_sizes_uncompressed,
            chunk_keys,
            static_data_map,
            temporal_data_map,
        })
    }

    /// Builds an [`RrdManifest`] for in-memory chunks (useful for tests).
    ///
    /// This is a convenience wrapper around [`RawRrdManifest::build_in_memory_from_chunks`].
    ///
    /// Chunk offsets will start at 0 and increment from there according to their heap size.
    /// There are no chunk keys whatsoever.
    pub fn build_in_memory_from_chunks<'a>(
        store_id: StoreId,
        chunks: impl Iterator<Item = &'a re_chunk::Chunk>,
    ) -> CodecResult<Arc<Self>> {
        let raw = RawRrdManifest::build_in_memory_from_chunks(store_id, chunks)?;
        Ok(Arc::new(Self::try_new(&raw)?))
    }

    /// Returns the store ID this manifest belongs to.
    #[inline]
    pub fn store_id(&self) -> &StoreId {
        &self.store_id
    }

    /// Returns the number of chunks (rows) in this manifest.
    #[inline]
    pub fn num_chunks(&self) -> usize {
        self.chunk_ids.len()
    }

    /// Returns the Sorbet schema of the recording.
    #[inline]
    pub fn sorbet_schema(&self) -> &arrow::datatypes::Schema {
        &self.sorbet_schema
    }

    /// Returns the `RecordBatch` with only the columns needed to do a `FetchChunk` request.
    ///
    /// See [`Self::CHUNK_FETCHER_COLUMNS`].
    #[inline]
    pub fn chunk_fetcher_rb(&self) -> &arrow::array::RecordBatch {
        &self.chunk_fetcher_rb
    }

    /// Returns all the chunk ids
    #[inline]
    pub fn col_chunk_ids(&self) -> &[ChunkId] {
        self.chunk_ids.as_slice()
    }

    /// Returns the chunk id column of a batch that has a [`Self::COLUMN_CHUNK_ID`] column.
    ///
    /// Use [`quiver::Column::as_slice`] for a zero-copy `&[ChunkId]` view of the result.
    pub fn col_chunk_ids_of(batch: &RecordBatch) -> Option<quiver::Column<ChunkId>> {
        Self::COLUMN_CHUNK_ID.extract(batch).ok()
    }

    /// Returns the entity path column.
    #[inline]
    pub fn col_chunk_entity_path(&self) -> &quiver::Column<EntityPath> {
        &self.chunk_entity_paths
    }

    /// Returns an iterator over the decoded Arrow data for the entity path column.
    ///
    /// This might incur interning costs, but is otherwise basically free.
    pub fn col_chunk_entity_path_iter(&self) -> impl Iterator<Item = EntityPath> {
        self.chunk_entity_paths.iter_owned()
    }

    /// Returns the is-static column.
    #[inline]
    pub fn col_chunk_is_static(&self) -> &quiver::Column<bool> {
        &self.chunk_is_static
    }

    /// Returns an iterator over the is-static values.
    #[inline]
    pub fn col_chunk_is_static_iter(&self) -> impl Iterator<Item = bool> + '_ {
        self.chunk_is_static.iter()
    }

    /// Returns the num-rows column.
    #[inline]
    pub fn col_chunk_num_rows(&self) -> &[u64] {
        &self.chunk_num_rows
    }

    /// Returns the chunk byte offsets column.
    #[inline]
    pub fn col_chunk_byte_offset(&self) -> &[u64] {
        &self.chunk_byte_offsets
    }

    /// Returns the chunk byte sizes column (compressed if applicable).
    ///
    /// See also the `Understand size/offset columns` section of the [`RawRrdManifest`] documentation.
    #[inline]
    pub fn col_chunk_byte_size(&self) -> &[u64] {
        &self.chunk_byte_sizes
    }

    /// Returns the uncompressed chunk byte sizes column.
    ///
    /// See also the `Understand size/offset columns` section of the [`RawRrdManifest`] documentation.
    #[inline]
    pub fn col_chunk_byte_size_uncompressed(&self) -> &[u64] {
        &self.chunk_byte_sizes_uncompressed
    }

    /// Returns the chunk key column, if present.
    ///
    /// Chunk keys are backend-specific identifiers that can be used to fetch chunk data.
    #[inline]
    pub fn col_chunk_key(&self) -> Option<&quiver::Column<Option<quiver::Binary>>> {
        self.chunk_keys.as_ref()
    }

    /// The segment each chunk comes from, row-aligned with [`Self::col_chunk_ids`].
    ///
    /// A manifest served from a server always has this column, since `FetchChunks`
    /// needs it. One read from an RRD file doesn't.
    pub fn col_chunk_partition_id(&self) -> Option<quiver::Column<re_types_core::SegmentId>> {
        Self::COLUMN_CHUNK_PARTITION_ID
            .extract(&self.chunk_fetcher_rb)
            .ok()
    }

    /// Returns the map-based representation of the static data in this RRD manifest.
    #[inline]
    pub fn static_map(&self) -> &RrdManifestStaticMap {
        &self.static_data_map
    }

    /// Returns the map-based representation of the temporal data in this RRD manifest.
    #[inline]
    pub fn temporal_map(&self) -> &RrdManifestTemporalMap {
        &self.temporal_data_map
    }

    /// Add an all-null `chunk_key` column to a `RecordBatch` that doesn't have one.
    ///
    /// Used by [`Self::merge`] to normalize schemas when some manifests have chunk keys
    /// and others don't.
    fn add_null_chunk_key_column(batch: &RecordBatch) -> RecordBatch {
        let num_rows = batch.num_rows();

        let schema = batch.schema();
        let mut fields: Vec<_> = schema.fields().iter().cloned().collect();
        let mut columns: Vec<_> = batch.columns().to_vec();

        // The field comes off the same descriptor as the array, so the two cannot disagree.
        // It is nullable because every row of the added column is null.
        let (field, array) = Self::COLUMN_CHUNK_KEY
            .optional()
            .new_null(num_rows)
            .into_dyn()
            .into_parts();
        fields.push(field);
        columns.push(array);

        RecordBatch::try_new_with_options(
            Arc::new(arrow::datatypes::Schema::new_with_metadata(
                fields,
                schema.metadata().clone(),
            )),
            columns,
            &arrow::array::RecordBatchOptions::new().with_row_count(Some(num_rows)),
        )
        .expect("adding a null column to a valid batch should not fail")
    }
}
