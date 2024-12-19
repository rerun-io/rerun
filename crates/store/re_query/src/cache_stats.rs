use std::collections::BTreeMap;

use re_byte_size::SizeBytes as _;

use crate::{QueryCache, QueryCacheKey};

// ---

/// Stats for all primary caches.
///
/// Fetch them via [`QueryCache::stats`].
#[derive(Default, Debug, Clone)]
pub struct CachesStats {
    pub latest_at: BTreeMap<QueryCacheKey, CacheStats>,
    pub range: BTreeMap<QueryCacheKey, CacheStats>,
}

impl CachesStats {
    #[inline]
    pub fn total_size_bytes(&self) -> u64 {
        re_tracing::profile_function!();

        let Self { latest_at, range } = self;

        let latest_at_size_bytes: u64 = latest_at
            .values()
            .map(|stats| stats.total_actual_size_bytes)
            .sum();
        let range_size_bytes: u64 = range
            .values()
            .map(|stats| stats.total_actual_size_bytes)
            .sum();

        latest_at_size_bytes + range_size_bytes
    }
}

/// Stats for a single `crate::RangeCache`.
#[derive(Default, Debug, Clone)]
pub struct CacheStats {
    /// How many chunks in the cache?
    pub total_chunks: u64,

    /// What would be the size of this cache in the worst case, i.e. if all chunks had
    /// been fully copied?
    pub total_effective_size_bytes: u64,

    /// What is the actual size of this cache after deduplication?
    pub total_actual_size_bytes: u64,
}

impl QueryCache {
    /// Computes the stats for all primary caches.
    pub fn stats(&self) -> CachesStats {
        re_tracing::profile_function!();

        let latest_at = {
            let latest_at = self.latest_at_per_cache_key.read().clone();
            // Implicitly releasing top-level cache mappings -- concurrent queries can run once again.

            latest_at
                .iter()
                .map(|(key, cache)| {
                    let cache = cache.read();
                    (
                        key.clone(),
                        CacheStats {
                            total_chunks: cache.per_query_time.len() as _,
                            total_effective_size_bytes: cache
                                .per_query_time
                                .values()
                                .map(|cached| cached.unit.total_size_bytes())
                                .sum(),
                            total_actual_size_bytes: cache.per_query_time.total_size_bytes(),
                        },
                    )
                })
                .collect()
        };

        let range = {
            let range = self.range_per_cache_key.read().clone();
            // Implicitly releasing top-level cache mappings -- concurrent queries can run once again.

            range
                .iter()
                .map(|(key, cache)| {
                    let cache = cache.read();

                    (
                        key.clone(),
                        CacheStats {
                            total_chunks: cache.chunks.len() as _,
                            total_effective_size_bytes: cache
                                .chunks
                                .values()
                                .map(|cached| cached.chunk.total_size_bytes())
                                .sum(),
                            total_actual_size_bytes: cache.chunks.total_size_bytes(),
                        },
                    )
                })
                .collect()
        };

        CachesStats { latest_at, range }
    }
}
