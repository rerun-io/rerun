#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct MemoryLimit {
    /// Limit in bytes.
    ///
    /// This is primarily compared to what is reported by [`crate::AccountingAllocator`] ('counted').
    /// We limit based on this instead of `resident` (RSS) because `counted` is what we have immediate
    /// control over, while RSS depends on what our allocator (MiMalloc) decides to do.
    /// Default is Some(100MB)
    pub limit: Option<i64>,
}

impl Default for MemoryLimit {
    fn default() -> Self {
        Self {
            limit: re_format::parse_bytes("100MB"),
        }
    }
}

impl MemoryLimit {
    pub fn parse(limit: &str) -> Result<Self, String> {
        re_format::parse_bytes(limit)
            .map(|limit| Self { limit: Some(limit) })
            .ok_or_else(|| format!("expected e.g. '16GB', got {limit:?}"))
    }

    /// Returns how large fraction of memory we should free to go down to the exact limit.
    pub fn is_exceeded_by(&self, mem_use: &crate::MemoryUse) -> Option<f32> {
        let limit = self.limit?;

        if let Some(counted_use) = mem_use.counted {
            if limit < counted_use {
                return Some((counted_use - limit) as f32 / counted_use as f32);
            }
        } else if let Some(resident_use) = mem_use.resident {
            re_log::warn_once!("Using resident memory use (RSS) for memory limiting, because a memory tracker was not available.");
            if limit < resident_use {
                return Some((resident_use - limit) as f32 / resident_use as f32);
            }
        }

        None
    }
}
