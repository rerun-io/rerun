#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct MemoryLimit {
    /// Limit in bytes.
    ///
    /// This is primarily compared to what is reported by [`crate::TrackingAllocator`] ('net').
    /// We limit based on this instead of `gross` (RSS) because `net` is what we have immediate
    /// control over, while RSS depends on what our allocator (MiMalloc) decides to do.
    pub limit: Option<i64>,
}

impl MemoryLimit {
    /// Read from the given environment variable.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_env_var(env_var: &str) -> Self {
        let limit = std::env::var(env_var).ok().map(|limit| {
            parse_bytes(&limit)
                .unwrap_or_else(|| panic!("{env_var}: expected e.g. '16GB', got {limit:?}"))
        });

        Self { limit }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn from_env_var(_env_var: &str) -> Self {
        // TODO(emilk): some way to have memory limits on web.
        Self { limit: None }
    }

    /// Returns how large fraction of memory we should free to go down to the exact limit.
    pub fn is_exceeded_by(&self, mem_use: &crate::MemoryUse) -> Option<f32> {
        let limit = self.limit?;

        if let Some(net_use) = mem_use.net {
            if limit < net_use {
                return Some((net_use - limit) as f32 / net_use as f32);
            }
        } else if let Some(gross_use) = mem_use.gross {
            re_log::warn_once!("Using gross memory use (RSS) for memory limiting, because a memory tracker was not available.");
            if limit < gross_use {
                return Some((gross_use - limit) as f32 / gross_use as f32);
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
