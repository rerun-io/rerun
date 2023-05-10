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
    /// This can be smaller than [`Self::resident`] because our memory allocator may not
    /// return all the memory we free to the OS.
    ///
    /// `None` if [`crate::AccountingAllocator`] is not used.
    pub counted: Option<i64>,
}

impl MemoryUse {
    pub fn capture() -> Self {
        Self {
            resident: bytes_resident(),
            counted: crate::accounting_allocator::global_allocs().map(|c| c.size as _),
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
            counted: sub(self.counted, rhs.counted),
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
    // blocked on https://github.com/Arc-blroth/memory-stats/issues/1
    None
}
