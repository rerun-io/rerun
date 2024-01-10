use std::{collections::BTreeMap, sync::atomic::AtomicBool};

use re_log_types::EntityPath;
use re_types_core::ComponentName;

use crate::Caches;

// ---

/// If `true`, enables the much-more-costly-to-compute per-component stats.
static ENABLE_DETAILED_STATS: AtomicBool = AtomicBool::new(false);

#[inline]
pub fn detailed_stats() -> bool {
    ENABLE_DETAILED_STATS.load(std::sync::atomic::Ordering::Relaxed)
}

#[inline]
pub fn set_detailed_stats(b: bool) {
    ENABLE_DETAILED_STATS.store(b, std::sync::atomic::Ordering::Relaxed);
}

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
    pub total_times: u64,
    pub total_size_bytes: u64,

    /// Only if [`detailed_stats`] returns `true` (see [`set_detailed_stats`]).
    pub per_component: Option<BTreeMap<ComponentName, CachedComponentStats>>,
}

/// Stats for a cached component.
#[derive(Default, Debug, Clone)]
pub struct CachedComponentStats {
    pub total_times: u64,
    pub total_values: u64,
}

impl Caches {
    /// Computes the stats for all primary caches.
    ///
    /// `per_component` toggles per-component stats.
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
                        let mut total_times = 0u64;
                        let mut per_component = detailed_stats().then(BTreeMap::default);

                        for latest_at_cache in
                            caches_per_arch.latest_at_per_archetype.read().values()
                        {
                            let latest_at_cache = latest_at_cache.read();
                            total_size_bytes += latest_at_cache.total_size_bytes;
                            total_times = latest_at_cache.per_data_time.len() as _;

                            if let Some(per_component) = per_component.as_mut() {
                                for bucket in latest_at_cache.per_data_time.values() {
                                    for (component_name, data) in &bucket.read().components {
                                        let stats: &mut CachedComponentStats =
                                            per_component.entry(*component_name).or_default();
                                        stats.total_times += data.dyn_num_entries() as u64;
                                        stats.total_values += data.dyn_num_values() as u64;
                                    }
                                }
                            }
                        }

                        CachedEntityStats {
                            total_size_bytes,
                            total_times,

                            per_component,
                        }
                    })
                })
                .collect();

            CachesStats { latest_at }
        })
    }
}
