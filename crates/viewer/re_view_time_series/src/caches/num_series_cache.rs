use ahash::HashMap;
use re_byte_size::SizeBytes as _;
use re_log_types::hash::Hash64;
use re_viewer_context::Cache;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct NumSeriesCacheKey {
    /// Hash identifying these query results.
    ///
    /// This is expected to change when the set of scalar values we query changes, which naturally
    /// invalidates the cached series count.
    pub query_result_hash: Hash64,

    /// Whether the scalar component is mapped directly (identity mapping) for this instruction.
    ///
    /// Remapped scalars may be capped (depending on `limits_enabled`); identity-mapped scalars are not.
    pub is_identity_mapping: bool,

    /// Whether visualizer limits are enabled in the app options.
    ///
    /// This affects whether we cap the number of series for remapped scalars.
    pub limits_enabled: bool,
}

impl re_byte_size::SizeBytes for NumSeriesCacheKey {
    fn heap_size_bytes(&self) -> u64 {
        // NumSeriesCacheKey is only Hash64 + two bools — all Copy, stack-only.
        0
    }
}

/// Per-recording cache for the number of scalar series per query result.
///
/// Shared across all time series views in a recording (keyed by `query_result_hash` and related flags).
#[derive(Default)]
pub struct NumSeriesCache {
    entries: HashMap<NumSeriesCacheKey, usize>,
}

/// Clears the cache once it grows past this many entries (~256 × 24 B ≈ 6 `KiB` of inline payload).
const MAX_ENTRIES: usize = 256;

impl NumSeriesCache {
    pub fn get_or_compute(
        &mut self,
        key: NumSeriesCacheKey,
        compute: impl FnOnce() -> usize,
    ) -> usize {
        if let Some(&num_series) = self.entries.get(&key) {
            return num_series;
        }

        let num_series = compute();
        if self.entries.len() >= MAX_ENTRIES {
            self.entries.clear();
        }
        self.entries.insert(key, num_series);
        num_series
    }
}

impl Cache for NumSeriesCache {
    fn name(&self) -> &'static str {
        "NumSeriesCache"
    }

    fn purge_memory(&mut self) {
        self.entries.clear();
    }
}

impl re_byte_size::SizeBytes for NumSeriesCache {
    fn heap_size_bytes(&self) -> u64 {
        self.entries.heap_size_bytes()
    }
}

impl re_byte_size::MemUsageTreeCapture for NumSeriesCache {
    fn capture_mem_usage_tree(&self) -> re_byte_size::MemUsageTree {
        re_byte_size::MemUsageTree::Bytes(self.heap_size_bytes())
    }
}
