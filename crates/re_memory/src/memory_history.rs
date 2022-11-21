use emath::History;

/// Returns monotonically increasing time in seconds.
#[inline]
fn now_sec() -> f64 {
    use instant::Instant;
    use once_cell::sync::Lazy;

    static START_INSTANT: Lazy<Instant> = Lazy::new(Instant::now);
    START_INSTANT.elapsed().as_nanos() as f64 / 1e9
}

// ----------------------------------------------------------------------------

/// Tracks memory use over time.
pub struct MemoryHistory {
    /// Bytes allocated by the application according to operating system.
    ///
    /// Resident Set Size (RSS) on Linux, Android, Mac, iOS.
    /// Working Set on Windows.
    pub gross: History<i64>,

    /// Bytes used by the application according to our own memory allocator's accounting.
    ///
    /// This will be smaller than [`Self::gross`] because our memory allocator may not
    /// return all the memory we free to the OS.
    pub net: History<i64>,
}

impl Default for MemoryHistory {
    fn default() -> Self {
        let max_elems = 128 * 1024;
        let max_seconds = f32::INFINITY;
        Self {
            gross: History::new(0..max_elems, max_seconds),
            net: History::new(0..max_elems, max_seconds),
        }
    }
}

impl MemoryHistory {
    pub fn is_empty(&self) -> bool {
        let Self { gross, net } = self;
        gross.is_empty() && net.is_empty()
    }

    /// Add data to history
    pub fn capture(&mut self) {
        let mem_use = crate::MemoryUse::capture();
        if mem_use.gross.is_some() || mem_use.net.is_some() {
            let now = now_sec();
            if let Some(gross) = mem_use.gross {
                self.gross.add(now, gross);
            }
            if let Some(net) = mem_use.net {
                self.net.add(now, net);
            }
        }
    }
}
