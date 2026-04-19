use std::sync::Arc;

use re_sdk_types::components::VideoCodec;

use crate::{ChunkStore, ChunkStoreConfig, ChunkStoreError};

/// Callback to detect whether a video sample is the start of a GoP (keyframe).
pub type IsStartOfGop = Arc<dyn Fn(&[u8], VideoCodec) -> anyhow::Result<bool> + Send + Sync>;

/// Options for [`ChunkStore::compacted`].
#[derive(Clone)]
pub struct CompactionOptions {
    /// Controls chunk size thresholds for both merging and splitting.
    pub config: ChunkStoreConfig,

    /// Maximum number of extra compaction passes to run.
    ///
    /// Compaction is iterative: each pass merges small neighboring chunks.
    /// Stops early if the chunk count converges.
    /// Defaults to 50 if `None`.
    pub num_extra_passes: Option<usize>,

    /// If set, video stream chunks will be rebatched so that each chunk
    /// aligns to GoP (Group of Pictures) boundaries.
    ///
    /// The callback should return `true` if the given sample data is a keyframe
    /// for the given codec. Use `re_video::is_start_of_gop` wrapped in a closure.
    ///
    /// If `None`, no video rebatching is performed.
    ///
    /// **Note:** GoP rebatching never splits a GoP across chunks, so if a single
    /// GoP is larger than [`ChunkStoreConfig::chunk_max_bytes`], it becomes one
    /// oversized chunk regardless of the ceiling. Streams with long keyframe
    /// intervals (e.g. 10+ seconds between I-frames) can therefore produce chunks
    /// that are many megabytes in size.
    pub is_start_of_gop: Option<IsStartOfGop>,
}

impl ChunkStore {
    /// Return a new, compacted version of this store.
    ///
    /// Compaction merges small neighboring chunks that share the same entity path, timelines, and
    /// datatypes, up to the thresholds in the config. Large chunks may be split.
    ///
    /// If `is_start_of_gop` is provided, video stream chunks are rebatched to align
    /// with GoP boundaries after compaction.
    ///
    /// The returned store has compaction **disabled** ([`ChunkStoreConfig::ALL_DISABLED`]).
    // TODO(RR-4328): we should improve this by exploiting the chunk index, hopefully making it
    //   memory-bounded
    pub fn compacted(&self, options: &CompactionOptions) -> Result<Self, ChunkStoreError> {
        re_tracing::profile_function!();

        let CompactionOptions {
            config,
            num_extra_passes,
            is_start_of_gop,
        } = options;

        let num_extra_passes = num_extra_passes.unwrap_or(50);

        // Initial pass: re-insert all chunks into a compaction-enabled store.
        let mut store = Self::new(self.id().clone(), config.clone());
        for chunk in self.iter_physical_chunks() {
            store.insert_chunk(chunk)?;
        }

        // Extra passes until convergence.
        for pass in 0..num_extra_passes {
            let now = web_time::Instant::now();
            let num_before = store.num_physical_chunks();
            let chunks: Vec<_> = store.iter_physical_chunks().cloned().collect();
            let mut new_store = Self::new(store.id().clone(), config.clone());
            for chunk in &chunks {
                new_store.insert_chunk(chunk)?;
            }
            let num_after = new_store.num_physical_chunks();
            store = new_store;

            re_log::info!(
                pass,
                num_before,
                num_after,
                time = ?now.elapsed(),
                "compaction pass completed",
            );

            if num_after >= num_before {
                re_log::info!(pass, "converged, stopping early");
                break;
            }
        }

        // Rebatch video stream chunks along GoP boundaries.
        if let Some(is_start_of_gop) = is_start_of_gop {
            let now = web_time::Instant::now();

            match crate::rebatch_videos::rebatch_video_chunks_to_gops(
                &store,
                config,
                is_start_of_gop.as_ref(),
            ) {
                Ok(new_store) => {
                    store = new_store;
                    re_log::info!(time = ?now.elapsed(), "video GoP rebatching completed");
                }
                Err(err) => {
                    re_log::warn!(%err, "video GoP rebatching failed");
                }
            }
        }

        // Return with compaction disabled so the result is inert.
        store.config = ChunkStoreConfig::ALL_DISABLED;

        Ok(store)
    }
}
