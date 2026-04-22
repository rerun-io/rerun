use emath::History;
use re_renderer::WgpuResourcePoolStatistics;
use re_viewer_context::store_hub::{StoreHubStats, StoreStats};

// ----------------------------------------------------------------------------

/// Tracks memory use over time.
pub struct MemoryHistory {
    /// Bytes allocated by the application according to operating system.
    ///
    /// Resident Set Size (RSS) on Linux, Android, Mac, iOS.
    /// Working Set on Windows.
    pub resident: History<u64>,

    /// Bytes used by the application according to our own memory allocator's accounting.
    ///
    /// This can be smaller than [`Self::resident`] because our memory allocator may not
    /// return all the memory we free to the OS.
    pub counted_allocator: History<u64>,

    /// VRAM bytes used by the application according to its own accounting if a tracker was installed.
    ///
    /// Values are usually a rough estimate as the actual amount of VRAM used depends a lot
    /// on the specific GPU and driver. Accounted typically only raw buffer & texture sizes.
    pub counted_vram: History<u64>,

    /// Bytes used by all blueprints, (according to its own accounting).
    pub counted_blueprints: History<u64>,

    /// Bytes used by all recordings, (according to its own accounting).
    pub counted_recordings: History<u64>,

    /// Bytes used by the primary caches (according to their own accounting).
    pub counted_query_caches: History<u64>,

    /// Bytes used by table stores (according to their own accounting).
    pub counted_table_stores: History<u64>,
}

impl Default for MemoryHistory {
    fn default() -> Self {
        let max_elems = 32 * 1024;
        let max_secs = f32::INFINITY;
        Self {
            resident: History::new(0..max_elems, max_secs),
            counted_allocator: History::new(0..max_elems, max_secs),
            counted_vram: History::new(0..max_elems, max_secs),
            counted_blueprints: History::new(0..max_elems, max_secs),
            counted_recordings: History::new(0..max_elems, max_secs),
            counted_query_caches: History::new(0..max_elems, max_secs),
            counted_table_stores: History::new(0..max_elems, max_secs),
        }
    }
}

impl MemoryHistory {
    /// Add data to history
    pub fn capture(
        &mut self,
        gpu_resource_stats: Option<&WgpuResourcePoolStatistics>,
        store_stats: Option<&StoreHubStats>,
    ) {
        let now = re_memory::util::sec_since_start();
        let mem_use = re_memory::MemoryUse::capture();

        let Self {
            resident,
            counted_allocator,
            counted_vram,
            counted_blueprints,
            counted_recordings,
            counted_query_caches,
            counted_table_stores,
        } = self;

        if let Some(updated_resident) = mem_use.resident {
            resident.add(now, updated_resident);
        }
        if let Some(updated_counted) = mem_use.counted {
            counted_allocator.add(now, updated_counted);
        }
        if let Some(gpu_resource_stats) = gpu_resource_stats {
            counted_vram.add(now, gpu_resource_stats.total_bytes());
        }

        if let Some(store_stats) = store_stats {
            let StoreHubStats {
                store_stats,
                table_stats,
            } = store_stats;

            let sum_table_stores: u64 = table_stats.values().copied().sum();
            let mut sum_recordings = 0;
            let mut sum_blueprints = 0;
            let mut sum_query_caches = 0;

            for (store_id, stats) in store_stats {
                let StoreStats {
                    store_config: _,
                    store_stats,
                    query_cache_stats,
                    cache_vram_usage: _,
                } = stats;

                match store_id.kind() {
                    re_log_types::StoreKind::Blueprint => {
                        sum_blueprints += store_stats.total().total_size_bytes;
                    }
                    re_log_types::StoreKind::Recording => {
                        sum_recordings += store_stats.total().total_size_bytes;
                    }
                }

                sum_query_caches += query_cache_stats.total_size_bytes();
            }

            counted_blueprints.add(now, sum_blueprints);
            counted_recordings.add(now, sum_recordings);
            counted_query_caches.add(now, sum_query_caches);
            counted_table_stores.add(now, sum_table_stores);
        }
    }
}
