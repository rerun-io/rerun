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
}

impl Default for MemoryHistory {
    fn default() -> Self {
        let max_elems = 32 * 1024;
        let max_seconds = f32::INFINITY;
        Self {
            resident: History::new(0..max_elems, max_seconds),
            counted: History::new(0..max_elems, max_seconds),
            counted_gpu: History::new(0..max_elems, max_seconds),
        }
    }
}

impl MemoryHistory {
    pub fn is_empty(&self) -> bool {
        let Self {
            resident,
            counted,
            counted_gpu,
        } = self;
        resident.is_empty() && counted.is_empty() && counted_gpu.is_empty()
    }

    /// Add data to history
    pub fn capture(&mut self, counted_gpu: Option<i64>) {
        let mem_use = crate::MemoryUse::capture();
        let now = crate::util::sec_since_start();
        if mem_use.resident.is_some() || mem_use.counted.is_some() {
            if let Some(resident) = mem_use.resident {
                self.resident.add(now, resident);
            }
            if let Some(counted) = mem_use.counted {
                self.counted.add(now, counted);
            }
        }

        if let Some(counted_gpu) = counted_gpu {
            self.counted_gpu.add(now, counted_gpu);
        }
    }
}
