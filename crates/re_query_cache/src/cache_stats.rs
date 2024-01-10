use std::collections::BTreeMap;

use re_log_types::EntityPath;

use crate::Caches;

// ---

/// Stats for all primary caches.
///
/// Fetch them via [`Caches::stats`].
#[derive(Default, Debug, Clone)]
pub struct CachesStats {
    pub latest_at: BTreeMap<EntityPath, CachedEntityStats>,
}

impl CachesStats {
    #[inline]
    pub fn total_size_bytes(&self) -> u64 {
        re_tracing::profile_function!();

        let Self { latest_at } = self;
        latest_at.values().map(|stats| stats.total_size_bytes).sum()
    }
}

/// Stats for a cached entity.
#[derive(Debug, Clone)]
pub struct CachedEntityStats {
    pub total_size_bytes: u64,
    pub num_cached_timestamps: u64,
}

impl Caches {
    /// Computes the stats for all primary caches.
    pub fn stats() -> CachesStats {
        re_tracing::profile_function!();

        Self::with(|caches| {
            let latest_at = caches
                .0
                .read()
                .iter()
                .map(|(key, caches_per_arch)| {
                    (key.entity_path.clone(), {
                        let mut total_size_bytes = 0u64;
                        let mut num_cached_timestamps = 0u64;

                        for latest_at_cache in
                            caches_per_arch.latest_at_per_archetype.read().values()
                        {
                            let latest_at_cache = latest_at_cache.read();
                            total_size_bytes += latest_at_cache.total_size_bytes;
                            num_cached_timestamps = latest_at_cache.per_data_time.len() as _;
                        }

                        CachedEntityStats {
                            total_size_bytes,
                            num_cached_timestamps,
                        }
                    })
                })
                .collect();

            CachesStats { latest_at }
        })
    }
}
