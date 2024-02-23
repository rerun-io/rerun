use std::{collections::BTreeMap, sync::atomic::Ordering::Relaxed};

use re_log_types::{EntityPath, TimeRange, Timeline};
use re_types_core::{components::InstanceKey, ComponentName, Loggable as _, SizeBytes as _};

use crate::{cache::CacheBucket, Caches, LatestAtCache, RangeCache};

// ---

/// Stats for all primary caches.
///
/// Fetch them via [`Caches::stats`].
#[derive(Default, Debug, Clone)]
pub struct CachesStats {
    pub total_num_entries: u64,
    pub total_size_bytes: u64,

    pub latest_at: BTreeMap<EntityPath, CachedEntityStats>,
    pub range: BTreeMap<EntityPath, Vec<(Timeline, TimeRange, CachedEntityStats)>>,
}

impl CachesStats {
    #[inline]
    pub fn total_size_bytes(&self) -> u64 {
        re_tracing::profile_function!();

        let Self {
            total_num_entries: _,
            total_size_bytes,
            latest_at: _,
            range: _,
        } = self;

        *total_size_bytes
    }
}

#[derive(Debug, Clone, Copy)]
pub enum CachesStatsKind {
    Overview,
    PerEntityPath,
    PerEntityPathPerComponent,
}

/// Stats for a cached entity.
#[derive(Debug, Clone, Default)]
pub struct CachedEntityStats {
    pub total_num_entries: u64,
    pub total_size_bytes: u64,

    /// Only if `detailed_stats` is `true` (see [`Caches::stats`]).
    pub per_component: Option<BTreeMap<ComponentName, CachedComponentStats>>,
}

impl CachedEntityStats {
    #[inline]
    pub fn is_empty(&self) -> bool {
        // NOTE: That looks non-sensical, but it can happen if the cache is bugged, which we'd like
        // to know.
        self.total_num_entries == 0 && self.total_size_bytes == 0
    }
}

/// Stats for a cached component.
#[derive(Default, Debug, Clone)]
pub struct CachedComponentStats {
    pub total_num_entries: u64,
    pub total_instances: u64,
    pub total_size_bytes: u64,
}

impl Caches {
    /// Computes the stats for all primary caches.
    ///
    /// `per_component` toggles per-component stats.
    pub fn stats(&self, kind: CachesStatsKind) -> CachesStats {
        re_tracing::profile_function!();

        fn upsert_bucket_stats(
            per_component: &mut BTreeMap<ComponentName, CachedComponentStats>,
            bucket: &CacheBucket,
        ) {
            let CacheBucket {
                data_times,
                pov_instance_keys,
                components,
                total_size_bytes: _,
            } = bucket;

            {
                let stats: &mut CachedComponentStats =
                    per_component.entry("<timepoints>".into()).or_default();
                stats.total_num_entries += data_times.len() as u64;
                stats.total_instances += data_times.len() as u64;
                stats.total_size_bytes += data_times.total_size_bytes();
            }

            {
                let stats: &mut CachedComponentStats =
                    per_component.entry(InstanceKey::name()).or_default();
                stats.total_num_entries += pov_instance_keys.num_entries() as u64;
                stats.total_instances += pov_instance_keys.num_values() as u64;
                stats.total_size_bytes += pov_instance_keys.total_size_bytes();
            }

            for (component_name, data) in components {
                let stats: &mut CachedComponentStats =
                    per_component.entry(*component_name).or_default();
                stats.total_num_entries += data.dyn_num_entries() as u64;
                stats.total_instances += data.dyn_num_values() as u64;
                stats.total_size_bytes += data.dyn_total_size_bytes();
            }
        }

        let caches = self.read().clone();
        // Implicitly releasing top-level cache mappings -- concurrent queries can run once again.

        let global_total_num_entries = caches
            .values()
            .map(|caches_per_arch| caches_per_arch.read().total_num_entries.load(Relaxed))
            .sum();
        let global_total_size_bytes = caches
            .values()
            .map(|caches_per_arch| caches_per_arch.read().total_size_bytes.load(Relaxed))
            .sum();

        match kind {
            CachesStatsKind::Overview => {
                re_tracing::profile_scope!("overview");

                CachesStats {
                    total_num_entries: global_total_num_entries,
                    total_size_bytes: global_total_size_bytes,
                    latest_at: Default::default(),
                    range: Default::default(),
                }
            }
            CachesStatsKind::PerEntityPath => {
                re_tracing::profile_scope!("per_entity_path");

                let latest_at = caches
                    .iter()
                    .map(|(key, caches_per_arch)| {
                        (key.entity_path.clone(), {
                            let caches_per_arch = caches_per_arch.read();

                            let total_num_entries = caches_per_arch.total_num_entries.load(Relaxed);
                            let total_size_bytes = caches_per_arch.total_size_bytes.load(Relaxed);

                            CachedEntityStats {
                                total_size_bytes,
                                total_num_entries,
                                per_component: None,
                            }
                        })
                    })
                    .collect();

                let range = caches
                    .iter()
                    .map(|(key, caches_per_arch)| {
                        (key.entity_path.clone(), {
                            let caches_per_arch = caches_per_arch.read();

                            let total_size_bytes = caches_per_arch.total_size_bytes.load(Relaxed);
                            let total_num_entries = caches_per_arch.total_num_entries.load(Relaxed);

                            vec![(
                                key.timeline,
                                TimeRange::EVERYTHING, // TODO
                                CachedEntityStats {
                                    total_size_bytes,
                                    total_num_entries,
                                    per_component: None,
                                },
                            )]
                        })
                    })
                    .collect();

                CachesStats {
                    total_num_entries: global_total_num_entries,
                    total_size_bytes: global_total_size_bytes,
                    latest_at,
                    range,
                }
            }
            CachesStatsKind::PerEntityPathPerComponent => {
                re_tracing::profile_scope!("per_entity_path_per_component");

                let latest_at = caches
                    .iter()
                    .map(|(key, caches_per_arch)| {
                        (key.entity_path.clone(), {
                            re_tracing::profile_scope!("detailed");

                            let mut total_size_bytes = 0u64;
                            let mut total_num_entries = 0u64;
                            let mut per_component = BTreeMap::default();

                            for latest_at_cache in caches_per_arch
                                .read()
                                .latest_at_per_archetype
                                .read()
                                .values()
                            {
                                let latest_at_cache @ LatestAtCache {
                                    per_query_time: _,
                                    per_data_time,
                                    timeless,
                                    ..
                                } = &*latest_at_cache.read();

                                total_size_bytes += latest_at_cache.total_size_bytes();
                                total_num_entries =
                                    per_data_time.len() as u64 + timeless.is_some() as u64;

                                if let Some(bucket) = &timeless {
                                    upsert_bucket_stats(&mut per_component, bucket);
                                }

                                for bucket in per_data_time.values() {
                                    upsert_bucket_stats(&mut per_component, bucket);
                                }
                            }

                            CachedEntityStats {
                                total_size_bytes,
                                total_num_entries,

                                per_component: Some(per_component),
                            }
                        })
                    })
                    .collect();

                let range = caches
                    .iter()
                    .map(|(key, caches_per_arch)| {
                        (key.entity_path.clone(), {
                            caches_per_arch
                                .read()
                                .range_per_archetype
                                .read()
                                .values()
                                .map(|range_cache| {
                                    let range_cache @ RangeCache {
                                        per_data_time,
                                        timeless,
                                        timeline: _,
                                    } = &*range_cache.read();

                                    let total_num_entries = per_data_time.data_times.len() as u64;

                                    let mut per_component = BTreeMap::default();
                                    upsert_bucket_stats(&mut per_component, timeless);
                                    upsert_bucket_stats(&mut per_component, per_data_time);

                                    (
                                        key.timeline,
                                        per_data_time.time_range().unwrap_or(TimeRange::EMPTY),
                                        CachedEntityStats {
                                            total_size_bytes: range_cache.total_size_bytes(),
                                            total_num_entries,

                                            per_component: Some(per_component),
                                        },
                                    )
                                })
                                .collect()
                        })
                    })
                    .collect();

                CachesStats {
                    total_num_entries: global_total_num_entries,
                    total_size_bytes: global_total_size_bytes,
                    latest_at,
                    range,
                }
            }
        }
    }
}
