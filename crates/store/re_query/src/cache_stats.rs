use std::collections::BTreeMap;

use re_log_types::ResolvedTimeRange;
use re_types_core::SizeBytes as _;

use crate::{CacheKey, Caches};

// ---

/// Stats for all primary caches.
///
/// Fetch them via [`Caches::stats`].
#[derive(Default, Debug, Clone)]
pub struct CachesStats {
    pub latest_at: BTreeMap<CacheKey, CachedComponentStats>,
    pub range: BTreeMap<CacheKey, (Option<ResolvedTimeRange>, CachedComponentStats)>,
}

impl CachesStats {
    #[inline]
    pub fn total_size_bytes(&self) -> u64 {
        re_tracing::profile_function!();

        let Self { latest_at, range } = self;

        let latest_at_size_bytes: u64 =
            latest_at.values().map(|stats| stats.total_size_bytes).sum();
        let range_size_bytes: u64 = range
            .values()
            .map(|(_, stats)| stats.total_size_bytes)
            .sum();

        latest_at_size_bytes + range_size_bytes
    }
}

/// Stats for a cached component.
#[derive(Default, Debug, Clone)]
pub struct CachedComponentStats {
    pub total_indices: u64,
    pub total_instances: u64,
    pub total_size_bytes: u64,
}

impl Caches {
    /// Computes the stats for all primary caches.
    pub fn stats(&self) -> CachesStats {
        re_tracing::profile_function!();

        let latest_at = {
            let latest_at = self.latest_at_per_cache_key.read_recursive().clone();
            // Implicitly releasing top-level cache mappings -- concurrent queries can run once again.

            latest_at
                .iter()
                .map(|(key, cache)| {
                    let cache = cache.read_recursive();
                    (
                        key.clone(),
                        CachedComponentStats {
                            total_indices: cache.per_data_time.len() as _,
                            total_instances: cache
                                .per_data_time
                                .values()
                                .map(|results| results.num_instances())
                                .sum(),
                            total_size_bytes: cache.total_size_bytes(),
                        },
                    )
                })
                .collect()
        };

        let range = {
            let range = self.range_per_cache_key.read_recursive().clone();
            // Implicitly releasing top-level cache mappings -- concurrent queries can run once again.

            range
                .iter()
                .map(|(key, cache)| {
                    let cache = cache.read_recursive();
                    let cache = cache.per_data_time.read_recursive();
                    (
                        key.clone(),
                        (
                            cache.pending_time_range(),
                            CachedComponentStats {
                                total_indices: cache.indices.len() as _,
                                total_instances: cache.num_instances(),
                                total_size_bytes: cache.total_size_bytes(),
                            },
                        ),
                    )
                })
                .collect()
        };

        CachesStats { latest_at, range }
    }
}
