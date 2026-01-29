use std::sync::Arc;

use arrow::array::{Array as _, BinaryArray, FixedSizeBinaryArray, StringArray};
use arrow::buffer::{BooleanBuffer, ScalarBuffer};
use re_chunk::ChunkId;
use re_chunk::external::re_byte_size;
use re_log_types::StoreId;

use super::{RawRrdManifest, RrdManifestSha256, RrdManifestStaticMap, RrdManifestTemporalMap};
use crate::CodecResult;

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
#[derive(Debug, Clone, PartialEq)]
pub struct RrdManifest {
    raw: RawRrdManifest,

    chunk_ids: FixedSizeBinaryArray,
    chunk_entity_paths: StringArray,
    chunk_is_static: BooleanBuffer,
    chunk_num_rows: ScalarBuffer<u64>,
    chunk_byte_offsets: ScalarBuffer<u64>,
    chunk_byte_sizes: ScalarBuffer<u64>,
    chunk_byte_sizes_uncompressed: ScalarBuffer<u64>,
    chunk_keys: Option<BinaryArray>,
}

impl re_byte_size::SizeBytes for RrdManifest {
    fn heap_size_bytes(&self) -> u64 {
        // The Arrow arrays are clones (Arc-based), so they share memory with the manifest.
        self.raw.heap_size_bytes()
    }
}

impl RrdManifest {
    /// Creates a new [`RrdManifest`].
    ///
    /// This validates the manifest and extracts all columns. If validation fails
    /// or any required column is missing/malformed, an error is returned.
    ///
    /// All arrays must be non-null (no missing values).
    pub fn try_new(manifest: RawRrdManifest) -> CodecResult<Self> {
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

        Ok(Self {
            raw: manifest,
            chunk_ids,
            chunk_entity_paths,
            chunk_is_static,
            chunk_num_rows,
            chunk_byte_offsets,
            chunk_byte_sizes,
            chunk_byte_sizes_uncompressed,
            chunk_keys,
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
        Ok(Arc::new(Self::try_new(raw)?))
    }

    /// Returns a reference to the underlying [`RawRrdManifest`].
    #[inline]
    pub fn raw(&self) -> &RawRrdManifest {
        &self.raw
    }

    /// Returns the number of chunks (rows) in this manifest.
    #[inline]
    pub fn num_chunks(&self) -> usize {
        self.chunk_ids.len()
    }

    /// Returns the recording ID that was used to identify the original recording.
    #[inline]
    pub fn store_id(&self) -> &StoreId {
        &self.raw.store_id
    }

    /// Returns the Sorbet schema of the recording.
    #[inline]
    pub fn sorbet_schema(&self) -> &arrow::datatypes::Schema {
        &self.raw.sorbet_schema
    }

    /// Returns the SHA256 hash of the Sorbet schema.
    #[inline]
    pub fn sorbet_schema_sha256(&self) -> &[u8; 32] {
        &self.raw.sorbet_schema_sha256
    }

    /// Returns the actual manifest data as a `RecordBatch`.
    #[inline]
    pub fn data(&self) -> &arrow::array::RecordBatch {
        &self.raw.data
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

    /// Computes the sha256 hash of the manifest's data, which can be used as a unique ID.
    ///
    /// Note: This is expensive to compute and delegates to [`RawRrdManifest::compute_sha256`].
    pub fn compute_sha256(&self) -> Result<RrdManifestSha256, arrow::error::ArrowError> {
        self.raw.compute_sha256()
    }

    /// Computes a map-based representation of the static data in this RRD manifest.
    ///
    /// Note: This delegates to [`RawRrdManifest::get_static_data_as_a_map`].
    pub fn get_static_data_as_a_map(&self) -> CodecResult<RrdManifestStaticMap> {
        self.raw.get_static_data_as_a_map()
    }

    /// Computes a map-based representation of the temporal data in this RRD manifest.
    ///
    /// Note: This delegates to [`RawRrdManifest::get_temporal_data_as_a_map`].
    pub fn get_temporal_data_as_a_map(&self) -> CodecResult<RrdManifestTemporalMap> {
        self.raw.get_temporal_data_as_a_map()
    }
}
