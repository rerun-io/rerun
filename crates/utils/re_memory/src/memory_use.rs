/// How much RAM is the application using?
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct MemoryUse {
    /// Bytes allocated by the application according to operating system.
    ///
    /// Resident Set Size (RSS) on Linux, Android, Mac, iOS.
    /// Working Set on Windows.
    ///
    /// `None` if unknown.
    pub resident: Option<u64>,

    /// Bytes used by the application according to our own memory allocator's accounting.
    ///
    /// This can be smaller than [`Self::resident`] because our memory allocator may not
    /// return all the memory we free to the OS.
    ///
    /// `None` if [`crate::AccountingAllocator`] is not used.
    pub counted: Option<u64>,
}

impl MemoryUse {
    /// Read the current memory of the running application.
    #[inline]
    pub fn capture() -> Self {
        Self {
            resident: bytes_resident(),
            counted: crate::accounting_allocator::global_allocs().map(|c| c.size as _),
        }
    }

    /// Bytes used by the application according to our best estimate.
    ///
    /// This is either [`Self::counted`] if it's available, otherwise fallbacks to
    /// [`Self::resident`] if that's available, otherwise `None`.
    #[inline]
    pub fn used(&self) -> Option<u64> {
        self.counted.or(self.resident)
    }
}

impl std::ops::Mul<f32> for MemoryUse {
    type Output = Self;

    fn mul(self, factor: f32) -> Self::Output {
        Self {
            resident: self.resident.map(|v| (v as f32 * factor) as u64),
            counted: self.counted.map(|v| (v as f32 * factor) as u64),
        }
    }
}

impl std::ops::Sub for MemoryUse {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        fn sub(a: Option<u64>, b: Option<u64>) -> Option<u64> {
            Some(a?.saturating_sub(b?))
        }

        Self {
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
fn bytes_resident() -> Option<u64> {
    memory_stats::memory_stats().map(|usage| usage.physical_mem as u64)
}

#[cfg(target_arch = "wasm32")]
fn bytes_resident() -> Option<u64> {
    // blocked on https://github.com/Arc-blroth/memory-stats/issues/1
    None
}
