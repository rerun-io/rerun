use crate::{ChunkStore, ChunkStoreConfig, ChunkStoreError};

impl ChunkStore {
    /// Return a new, compacted version of this store.
    ///
    /// Compaction merges small neighboring chunks that share the same entity path, timelines, and
    /// datatypes, up to the thresholds in `compaction_config`. Large chunks may be split.
    /// Up to `num_extra_passes` extra passes are run until the chunk count converges (defaults to
    /// 50).
    ///
    /// The returned store has compaction **disabled** ([`ChunkStoreConfig::ALL_DISABLED`]).
    // TODO(RR-4328): we should improve this by exploiting the chunk index, hopefully making it
    //   memory-bounded
    pub fn compacted(
        &self,
        compaction_config: &ChunkStoreConfig,
        num_extra_passes: Option<usize>,
    ) -> Result<Self, ChunkStoreError> {
        re_tracing::profile_function!();

        let num_extra_passes = num_extra_passes.unwrap_or(50);

        // Initial pass: re-insert all chunks into a compaction-enabled store.
        let mut store = Self::new(self.id().clone(), compaction_config.clone());
        for chunk in self.iter_physical_chunks() {
            store.insert_chunk(chunk)?;
        }

        // Extra passes until convergence.
        for pass in 0..num_extra_passes {
            let now = web_time::Instant::now();
            let num_before = store.num_physical_chunks();
            let chunks: Vec<_> = store.iter_physical_chunks().cloned().collect();
            let mut new_store = Self::new(store.id().clone(), compaction_config.clone());
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

        // Return with compaction disabled so the result is inert.
        store.config = ChunkStoreConfig::ALL_DISABLED;
        Ok(store)
    }
}
