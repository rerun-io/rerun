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
    /// This will be smaller than [`Self::resident`] because our memory allocator may not
    /// return all the memory we free to the OS.
    pub net: History<i64>,
}

impl Default for MemoryHistory {
    fn default() -> Self {
        let max_elems = 32 * 1024;
        let max_seconds = f32::INFINITY;
        Self {
            resident: History::new(0..max_elems, max_seconds),
            net: History::new(0..max_elems, max_seconds),
        }
    }
}

impl MemoryHistory {
    pub fn is_empty(&self) -> bool {
        let Self { resident, net } = self;
        resident.is_empty() && net.is_empty()
    }

    /// Add data to history
    pub fn capture(&mut self) {
        let mem_use = crate::MemoryUse::capture();
        if mem_use.resident.is_some() || mem_use.net.is_some() {
            let now = crate::util::sec_since_start();
            if let Some(resident) = mem_use.resident {
                self.resident.add(now, resident);
            }
            if let Some(net) = mem_use.net {
                self.net.add(now, net);
            }
        }
    }
}
