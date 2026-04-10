use std::sync::Arc;

use arrow::array::{Array as _, BinaryArray, FixedSizeBinaryArray, RecordBatch, StringArray};
use arrow::buffer::{BooleanBuffer, ScalarBuffer};
use arrow::datatypes::Field;
use re_chunk::external::re_byte_size;
use re_chunk::{ChunkId, EntityPath};
use re_log_types::StoreId;
use re_sorbet::SorbetSchema;

use super::{RawRrdManifest, RrdManifestStaticMap, RrdManifestTemporalMap};
use crate::{CodecError, CodecResult};

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

    recording_schema: SorbetSchema,
    sorbet_schema: arrow::datatypes::Schema,

    chunk_ids: FixedSizeBinaryArray,
    chunk_entity_paths: StringArray,
    chunk_is_static: BooleanBuffer,
    chunk_num_rows: ScalarBuffer<u64>,
    chunk_byte_offsets: ScalarBuffer<u64>,
    chunk_byte_sizes: ScalarBuffer<u64>,
    chunk_byte_sizes_uncompressed: ScalarBuffer<u64>,
    chunk_keys: Option<BinaryArray>,

    static_data_map: RrdManifestStaticMap,
    temporal_data_map: RrdManifestTemporalMap,
}

impl PartialEq for RrdManifest {
    fn eq(&self, other: &Self) -> bool {
        // Destructure to get a compile error when new fields are added,
        // ensuring we consciously decide whether to include them.
        let Self {
            chunk_fetcher_rb,
            recording_schema,
            // We skip `sorbet_schema` (the raw `arrow::datatypes::Schema`) because it is
            // redundant with `recording_schema` for semantic equality, and its field order
            // is not preserved through protobuf round-trips.
            sorbet_schema: _,
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
        // covered by `chunk_fetcher_rb.heap_size_bytes()`. However, after `concat` all arrays
        // are independently allocated, so the pruned-batch size alone would undercount.
        // We intentionally accept that minor double-count (via Arc sharing) from `try_new`
        // in exchange for always being correct after `concat`.
        //
        // Fields that are never in the pruned batch must always be counted separately:
        self.chunk_fetcher_rb.heap_size_bytes()
            + re_byte_size::SizeBytes::heap_size_bytes(
                &self.chunk_entity_paths as &dyn arrow::array::Array,
            )
            + self.chunk_num_rows.heap_size_bytes()
            + self.chunk_byte_sizes.heap_size_bytes()
            + self.chunk_byte_sizes_uncompressed.heap_size_bytes()
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
    pub const FIELD_CHUNK_ID: &str = RawRrdManifest::FIELD_CHUNK_ID;
    pub const FIELD_CHUNK_KEY: &str = RawRrdManifest::FIELD_CHUNK_KEY;
    pub const FIELD_CHUNK_IS_STATIC: &str = RawRrdManifest::FIELD_CHUNK_IS_STATIC;
    pub const FIELD_CHUNK_BYTE_OFFSET: &str = RawRrdManifest::FIELD_CHUNK_BYTE_OFFSET;
    pub const FIELD_CHUNK_PARTITION_ID: &str = "chunk_partition_id";
    pub const FIELD_RERUN_PARTITION_LAYER: &str = "rerun_partition_layer";

    /// All columns present in the pruned batch returned by [`Self::chunk_fetcher_rb()`].
    pub const CHUNK_FETCHER_COLUMNS: &[&str] = &[
        Self::FIELD_CHUNK_ID,
        Self::FIELD_CHUNK_KEY,
        Self::FIELD_CHUNK_IS_STATIC,
        Self::FIELD_CHUNK_BYTE_OFFSET,
        Self::FIELD_CHUNK_PARTITION_ID,
        Self::FIELD_RERUN_PARTITION_LAYER,
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

        let chunk_ids = manifest.col_chunk_id_raw()?.clone();

        // Validate:
        ChunkId::try_slice_from_arrow(&chunk_ids).map_err(|err| {
            crate::CodecError::FrameDecoding(format!("chunk_id column has wrong datatype: {err}"))
        })?;

        let chunk_entity_paths = manifest.col_chunk_entity_path_raw()?.clone();

        let chunk_is_static_array = manifest.col_chunk_is_static_raw()?;
        let chunk_num_rows_array = manifest.col_chunk_num_rows_raw()?;
        let chunk_byte_offsets_array = manifest.col_chunk_byte_offset_raw()?;
        let chunk_byte_sizes_array = manifest.col_chunk_byte_size_raw()?;
        let chunk_byte_sizes_uncompressed_array =
            manifest.col_chunk_byte_size_uncompressed_raw()?;

        // Validate that required arrays have no nulls
        if chunk_ids.null_count() > 0 {
            return Err(crate::CodecError::FrameDecoding(format!(
                "chunk_id column has {} nulls",
                chunk_ids.null_count()
            )));
        }
        if chunk_entity_paths.null_count() > 0 {
            return Err(crate::CodecError::FrameDecoding(format!(
                "chunk_entity_path column has {} nulls",
                chunk_entity_paths.null_count()
            )));
        }
        if chunk_is_static_array.null_count() > 0 {
            return Err(crate::CodecError::FrameDecoding(format!(
                "chunk_is_static column has {} nulls",
                chunk_is_static_array.null_count()
            )));
        }
        if chunk_num_rows_array.null_count() > 0 {
            return Err(crate::CodecError::FrameDecoding(format!(
                "chunk_num_rows column has {} nulls",
                chunk_num_rows_array.null_count()
            )));
        }
        if chunk_byte_offsets_array.null_count() > 0 {
            return Err(crate::CodecError::FrameDecoding(format!(
                "chunk_byte_offset column has {} nulls",
                chunk_byte_offsets_array.null_count()
            )));
        }
        if chunk_byte_sizes_array.null_count() > 0 {
            return Err(crate::CodecError::FrameDecoding(format!(
                "chunk_byte_size column has {} nulls",
                chunk_byte_sizes_array.null_count()
            )));
        }
        if chunk_byte_sizes_uncompressed_array.null_count() > 0 {
            return Err(crate::CodecError::FrameDecoding(format!(
                "chunk_byte_size_uncompressed column has {} nulls",
                chunk_byte_sizes_uncompressed_array.null_count()
            )));
        }

        // Extract scalar buffers (safe after null validation)
        let chunk_is_static = chunk_is_static_array.values().clone();
        let chunk_num_rows = chunk_num_rows_array.values().clone();
        let chunk_byte_offsets = chunk_byte_offsets_array.values().clone();
        let chunk_byte_sizes = chunk_byte_sizes_array.values().clone();
        let chunk_byte_sizes_uncompressed = chunk_byte_sizes_uncompressed_array.values().clone();

        let chunk_keys = if manifest
            .data
            .schema_ref()
            .column_with_name(RawRrdManifest::FIELD_CHUNK_KEY)
            .is_some()
        {
            Some(manifest.col_chunk_key_raw()?.clone())
        } else {
            None
        };

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
            recording_schema,
            sorbet_schema: manifest.sorbet_schema.clone(),
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

    pub fn concat(manifests: &[&Self]) -> CodecResult<Self> {
        re_tracing::profile_function!();

        let first = manifests
            .first()
            .ok_or_else(|| CodecError::FrameDecoding("No manifests to concatenate".to_owned()))?;

        let any_has_chunk_keys = manifests.iter().any(|m| m.chunk_keys.is_some());

        // Concatenate the (already pruned) raw manifests — used for `take_record_batch`.
        //
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
        let combined_batches = arrow::compute::concat_batches(&combined_schema, batches_to_concat)
            .map_err(|err| {
                CodecError::FrameDecoding(format!(
                    "Failed to concatenate RRD manifest parts: {err}"
                ))
            })?;

        // Concatenate pre-extracted Arrow arrays directly, avoiding a round-trip
        // through `try_new` which would fail on pruned data (missing sparse columns).
        let chunk_ids = {
            let arrays: Vec<&dyn arrow::array::Array> =
                manifests.iter().map(|m| &m.chunk_ids as _).collect();
            re_arrow_util::concat_arrays(&arrays)
                .map_err(|err| CodecError::FrameDecoding(format!("concat chunk_ids: {err}")))?
                .as_any()
                .downcast_ref::<FixedSizeBinaryArray>()
                .expect("concat of FixedSizeBinaryArray should yield FixedSizeBinaryArray")
                .clone()
        };
        let chunk_entity_paths = {
            let arrays: Vec<&dyn arrow::array::Array> = manifests
                .iter()
                .map(|m| &m.chunk_entity_paths as _)
                .collect();
            re_arrow_util::concat_arrays(&arrays)
                .map_err(|err| {
                    CodecError::FrameDecoding(format!("concat chunk_entity_paths: {err}"))
                })?
                .as_any()
                .downcast_ref::<StringArray>()
                .expect("concat of StringArray should yield StringArray")
                .clone()
        };
        let chunk_is_static = manifests
            .iter()
            .flat_map(|m| m.chunk_is_static.iter())
            .collect::<BooleanBuffer>();
        let chunk_num_rows = ScalarBuffer::from(
            manifests
                .iter()
                .flat_map(|m| m.chunk_num_rows.iter().copied())
                .collect::<Vec<_>>(),
        );
        let chunk_byte_offsets = ScalarBuffer::from(
            manifests
                .iter()
                .flat_map(|m| m.chunk_byte_offsets.iter().copied())
                .collect::<Vec<_>>(),
        );
        let chunk_byte_sizes = ScalarBuffer::from(
            manifests
                .iter()
                .flat_map(|m| m.chunk_byte_sizes.iter().copied())
                .collect::<Vec<_>>(),
        );
        let chunk_byte_sizes_uncompressed = ScalarBuffer::from(
            manifests
                .iter()
                .flat_map(|m| m.chunk_byte_sizes_uncompressed.iter().copied())
                .collect::<Vec<_>>(),
        );
        // When some manifests have chunk_keys and others don't, create all-null
        // BinaryArrays for the keyless manifests to maintain row alignment.
        let chunk_keys = if any_has_chunk_keys {
            let null_arrays: Vec<BinaryArray> = manifests
                .iter()
                .filter(|m| m.chunk_keys.is_none())
                .map(|m| BinaryArray::new_null(m.num_chunks()))
                .collect();
            let mut null_idx = 0;
            let arrays: Vec<&dyn arrow::array::Array> = manifests
                .iter()
                .map(|m| {
                    if let Some(keys) = &m.chunk_keys {
                        keys as &dyn arrow::array::Array
                    } else {
                        let arr = &null_arrays[null_idx] as &dyn arrow::array::Array;
                        null_idx += 1;
                        arr
                    }
                })
                .collect();
            Some(
                re_arrow_util::concat_arrays(&arrays)
                    .map_err(|err| CodecError::FrameDecoding(format!("concat chunk_keys: {err}")))?
                    .as_any()
                    .downcast_ref::<BinaryArray>()
                    .expect("concat of BinaryArray should yield BinaryArray")
                    .clone(),
            )
        } else {
            None
        };

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
            recording_schema: first.recording_schema.clone(),
            sorbet_schema: first.sorbet_schema.clone(),
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
        #[expect(clippy::unwrap_used)] // Validated in constructor
        ChunkId::try_slice_from_arrow(&self.chunk_ids).unwrap()
    }

    /// Returns the raw Arrow array for entity paths.
    #[inline]
    pub fn col_chunk_entity_path_raw(&self) -> &StringArray {
        &self.chunk_entity_paths
    }

    /// Returns an iterator over the decoded Arrow data for the entity path column.
    ///
    /// This might incur interning costs, but is otherwise basically free.
    pub fn col_chunk_entity_path(&self) -> impl Iterator<Item = EntityPath> {
        self.chunk_entity_paths
            .iter()
            .flatten()
            .map(EntityPath::parse_forgiving)
    }

    /// Returns the buffer for the is-static column.
    #[inline]
    pub fn col_chunk_is_static_raw(&self) -> &BooleanBuffer {
        &self.chunk_is_static
    }

    /// Returns an iterator over the is-static values.
    #[inline]
    pub fn col_chunk_is_static(&self) -> impl Iterator<Item = bool> + '_ {
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

    /// Returns the raw Arrow array for chunk keys, if present.
    ///
    /// Chunk keys are backend-specific identifiers that can be used to fetch chunk data.
    #[inline]
    pub fn col_chunk_key_raw(&self) -> Option<&BinaryArray> {
        self.chunk_keys.as_ref()
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
    /// Used by [`Self::concat`] to normalize schemas when some manifests have chunk keys
    /// and others don't.
    fn add_null_chunk_key_column(batch: &RecordBatch) -> RecordBatch {
        let num_rows = batch.num_rows();
        let null_keys = BinaryArray::new_null(num_rows);

        let schema = batch.schema();
        let mut fields: Vec<_> = schema.fields().iter().cloned().collect();
        let mut columns: Vec<_> = batch.columns().to_vec();

        fields.push(Arc::new(Field::new(
            Self::FIELD_CHUNK_KEY,
            arrow::datatypes::DataType::Binary,
            true,
        )));
        columns.push(Arc::new(null_keys));

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
