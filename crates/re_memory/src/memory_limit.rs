#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct MemoryLimit {
    /// Limit in bytes based compared to what is reported by [`crate::TrackingAllocator`].
    ///
    /// We limit based on this instead of `gross` (RSS) because `net` is what we have immediate
    /// control over, while RSS depends on what our allocator (MiMalloc) decides to do.
    pub net: Option<i64>,
}

impl MemoryLimit {
    /// Read from the given environment variable.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_env_var(env_var: &str) -> Self {
        let limit = std::env::var(env_var).ok().map(|limit| {
            parse_bytes(&limit)
                .unwrap_or_else(|| panic!("{env_var}: expected e.g. '16GB', got {limit:?}"))
        });

        Self { net: limit }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn from_env_var(_env_var: &str) -> Self {
        // TODO(emilk): some way to have memory limits on web.
        Self { net: None }
    }

    /// Returns how large fraction of memory we should free to go down to the exact limit.
    pub fn is_exceeded_by(&self, mem_use: &crate::MemoryUse) -> Option<f32> {
        if let (Some(net_limit), Some(net_use)) = (self.net, mem_use.net) {
            if net_limit < net_use {
                return Some((net_use - net_limit) as f32 / net_use as f32);
            }
        }

        None
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_bytes(limit: &str) -> Option<i64> {
    if let Some(kb) = limit.strip_suffix("kB") {
        Some(kb.parse::<i64>().ok()? * 1_000)
    } else if let Some(mb) = limit.strip_suffix("MB") {
        Some(mb.parse::<i64>().ok()? * 1_000_000)
    } else if let Some(gb) = limit.strip_suffix("GB") {
        Some(gb.parse::<i64>().ok()? * 1_000_000_000)
    } else if let Some(tb) = limit.strip_suffix("TB") {
        Some(tb.parse::<i64>().ok()? * 1_000_000_000_000)
    } else {
        None
    }
}

#[test]
fn test_parse_bytes() {
    assert_eq!(parse_bytes("10MB"), Some(10_000_000));
}
