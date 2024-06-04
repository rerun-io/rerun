use emath::History;

// ----------------------------------------------------------------------------

/// Tracks memory use over time.
pub struct MemoryHistory {
    /// Bytes allocated by the application according to operating system.
    ///
    /// Resident Set Size (RSS) on Linux, Android, Mac, iOS.
    /// Working Set on Windows.
    pub resident: History<i64>,

    /// Bytes used by the application according to our own memory allocator's accounting.
    ///
    /// This can be smaller than [`Self::resident`] because our memory allocator may not
    /// return all the memory we free to the OS.
    pub counted: History<i64>,

    /// VRAM bytes used by the application according to its own accounting if a tracker was installed.
    ///
    /// Values are usually a rough estimate as the actual amount of VRAM used depends a lot
    /// on the specific GPU and driver. Accounted typically only raw buffer & texture sizes.
    pub counted_gpu: History<i64>,

    /// Bytes used by the datastore according to its own accounting.
    pub counted_store2: History<i64>,

    /// Bytes used by the primary caches according to their own accounting.
    pub counted_primary_caches: History<i64>,

    /// Bytes used by the blueprint store according to its own accounting.
    pub counted_blueprint: History<i64>,
}

impl Default for MemoryHistory {
    fn default() -> Self {
        let max_elems = 32 * 1024;
        let max_seconds = f32::INFINITY;
        Self {
            resident: History::new(0..max_elems, max_seconds),
            counted: History::new(0..max_elems, max_seconds),
            counted_gpu: History::new(0..max_elems, max_seconds),
            counted_store2: History::new(0..max_elems, max_seconds),
            counted_primary_caches: History::new(0..max_elems, max_seconds),
            counted_blueprint: History::new(0..max_elems, max_seconds),
        }
    }
}

impl MemoryHistory {
    pub fn is_empty(&self) -> bool {
        let Self {
            resident,
            counted,
            counted_gpu,
            counted_store2,
            counted_primary_caches,
            counted_blueprint,
        } = self;
        resident.is_empty()
            && counted.is_empty()
            && counted_gpu.is_empty()
            && counted_store2.is_empty()
            && counted_primary_caches.is_empty()
            && counted_blueprint.is_empty()
    }

    /// Add data to history
    pub fn capture(
        &mut self,
        updated_counted_gpu: Option<i64>,
        updated_counted_store2: Option<i64>,
        updated_counted_primary_caches: Option<i64>,
        updated_counted_blueprint: Option<i64>,
    ) {
        let mem_use = crate::MemoryUse::capture();
        let now = crate::util::sec_since_start();

        let Self {
            resident,
            counted,
            counted_gpu,
            counted_store2,
            counted_primary_caches,
            counted_blueprint,
        } = self;

        if let Some(updated_resident) = mem_use.resident {
            resident.add(now, updated_resident);
        }
        if let Some(updated_counted) = mem_use.counted {
            counted.add(now, updated_counted);
        }
        if let Some(updated_counted_gpu) = updated_counted_gpu {
            counted_gpu.add(now, updated_counted_gpu);
        }
        if let Some(updated_counted_store2) = updated_counted_store2 {
            counted_store2.add(now, updated_counted_store2);
        }
        if let Some(updated_counted_primary_caches) = updated_counted_primary_caches {
            counted_primary_caches.add(now, updated_counted_primary_caches);
        }
        if let Some(updated_counted_blueprint) = updated_counted_blueprint {
            counted_blueprint.add(now, updated_counted_blueprint);
        }
    }
}
