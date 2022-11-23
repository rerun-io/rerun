#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct MemoryUse {
    /// Bytes allocated by the application according to operating system.
    ///
    /// Resident Set Size (RSS) on Linux, Android, Mac, iOS.
    /// Working Set on Windows.
    ///
    /// `None` if unknown.
    pub resident: Option<i64>,

    /// Bytes used by the application according to our own memory allocator's accounting.
    ///
    /// This will be smaller than [`Self::resident`] because our memory allocator may not
    /// return all the memory we free to the OS.
    ///
    /// `None` if [`crate::TrackingAllocator`] is not used.
    pub net: Option<i64>,
}

impl MemoryUse {
    pub fn capture() -> Self {
        Self {
            resident: bytes_resident(),
            net: bytes_used_net(),
        }
    }
}

impl std::ops::Sub for MemoryUse {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        fn sub(a: Option<i64>, b: Option<i64>) -> Option<i64> {
            Some(a? - b?)
        }

        MemoryUse {
            resident: sub(self.resident, rhs.resident),
            net: sub(self.net, rhs.net),
        }
    }
}

// ----------------------------------------------------------------------------

/// According to the OS. This is what matters.
///
/// Resident Set Size (RSS) on Linux, Android, Mac, iOS.
/// Working Set on Windows.
#[cfg(not(target_arch = "wasm32"))]
fn bytes_resident() -> Option<i64> {
    memory_stats::memory_stats().map(|usage| usage.physical_mem as i64)
}

#[cfg(target_arch = "wasm32")]
fn bytes_resident() -> Option<i64> {
    // blocked on https://github.com/Arc-blroth/memory-stats/issues/1 and https://github.com/rustwasm/wasm-bindgen/issues/3159
    None
}

/// The amount of memory in use.
///
/// The difference to [`bytes_resident`] is memory allocated by `MiMalloc`.
/// that hasn't been returned to the OS.
///
/// `None` if [`crate::TrackingAllocator`] is not used.
fn bytes_used_net() -> Option<i64> {
    let num_bytes = crate::tracking_allocator::global_allocs().size;
    if num_bytes == 0 {
        None
    } else {
        Some(num_bytes as _)
    }
}
