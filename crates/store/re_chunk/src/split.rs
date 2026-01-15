use std::sync::Arc;

use re_byte_size::SizeBytes;
use re_types_core::ChunkId;

use crate::Chunk;

/// See [`Chunk::split_chunk_if_needed`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChunkSplitConfig {
    /// Split chunks larger than this.
    pub chunk_max_bytes: u64,

    /// Split chunks with more rows than this.
    ///
    /// This specifically applies to time-sorted chunks.
    /// See also [`Self::chunk_max_rows_if_unsorted`].
    pub chunk_max_rows: u64,

    /// Split chunks with more rows than this.
    ///
    /// This specifically applies to _non_ time-sorted chunks.
    /// See also [`Self::chunk_max_rows`].
    pub chunk_max_rows_if_unsorted: u64,
}

impl Chunk {
    /// Naively splits a chunk if it exceeds the configured thresholds.
    ///
    /// The resulting pieces may still be larger than [`ChunkSplitConfig::chunk_max_bytes`].
    ///
    /// The Chunk is *deeply* sliced, as opposed to shallowly. Refer to [`Chunk::row_sliced_deep`]
    /// to learn more about that and why it matters.
    pub fn split_chunk_if_needed(chunk: Arc<Self>, cfg: &ChunkSplitConfig) -> Vec<Arc<Self>> {
        let ChunkSplitConfig {
            chunk_max_bytes,
            chunk_max_rows,
            chunk_max_rows_if_unsorted,
        } = *cfg;

        let chunk_size_bytes = <Self as SizeBytes>::total_size_bytes(chunk.as_ref());
        let chunk_num_rows = chunk.num_rows() as u64;

        if chunk_num_rows <= 1 {
            // Can't split even if we wanted to.
            return vec![chunk];
        }

        // Check if we need to split based on size or row count
        let needs_split_bytes = chunk_max_bytes > 0 && chunk_size_bytes > chunk_max_bytes;
        let needs_split_rows = chunk_max_rows > 0 && chunk_num_rows > chunk_max_rows;
        let needs_split_unsorted = chunk_max_rows_if_unsorted > 0
            && chunk_num_rows > chunk_max_rows_if_unsorted
            && !chunk.is_time_sorted();

        if !needs_split_bytes && !needs_split_rows && !needs_split_unsorted {
            return vec![chunk];
        }

        re_tracing::profile_scope!("split_chunk");

        // Determine the target number of rows per split chunk
        let target_rows = if needs_split_unsorted {
            chunk_max_rows_if_unsorted
        } else if needs_split_rows {
            chunk_max_rows
        } else {
            // For byte-based splitting, estimate rows per split chunk based on current density
            let bytes_per_row = chunk_size_bytes / chunk_num_rows.max(1);
            chunk_max_bytes / bytes_per_row.max(1)
        };

        let target_rows = target_rows.max(1) as usize; // Ensure at least 1 row per chunk

        let mut result = Vec::with_capacity(chunk.num_rows().div_ceil(target_rows));
        let mut start_idx = 0;

        while start_idx < chunk.num_rows() {
            let remaining_rows = chunk.num_rows() - start_idx;
            let chunk_size = remaining_rows.min(target_rows);

            let split_chunk = chunk
                .row_sliced_deep(start_idx, chunk_size)
                .with_id(ChunkId::new());

            result.push(Arc::new(split_chunk));

            start_idx += chunk_size;
        }

        re_log::trace!(
            entity_path = %chunk.entity_path(),
            original_rows = chunk.num_rows(),
            original_bytes = %re_format::format_bytes(chunk_size_bytes as _),
            split_into = result.len(),
            target_rows,
            "split chunk"
        );

        result
    }
}
