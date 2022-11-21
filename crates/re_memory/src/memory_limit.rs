#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct MemoryLimit {
    /// Limit in bytes compared to what is reported by OS.
    ///
    /// Resident Set Size (RSS) on Linux, Android, Mac, iOS.
    /// Working Set on Windows.
    pub gross: Option<i64>,

    /// Limit in bytes based compared to what is reported by [`crate::TrackingAllocator`].
    pub net: Option<i64>,
}

impl MemoryLimit {
    /// Read from the given environment variable.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_env_var(env_var: &str) -> Self {
        fn parse_limit(limit: &str) -> Option<i64> {
            Some(limit.strip_suffix("GB")?.parse::<i64>().ok()? * 1_000_000_000)
        }

        let gross_limit = std::env::var(env_var).ok().map(|limit| {
            parse_limit(&limit).unwrap_or_else(|| panic!("{env_var}: expected e.g. '16GB'"))
        });

        Self {
            gross: gross_limit,

            // Start freeing a bit before we reach OS limit:
            net: gross_limit.map(|g| g / 4 * 3),
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn from_env_var(_env_var: &str) -> Self {
        // TODO(emilk): some way to have memory limits on web.
        Self {
            gross: None,
            net: None,
        }
    }

    pub fn is_exceeded_by(&self, mem_use: &crate::MemoryUse) -> bool {
        if let (Some(gross_limit), Some(gross_use)) = (self.gross, mem_use.gross) {
            if gross_limit < gross_use {
                return true;
            }
        }

        if let (Some(net_limit), Some(net_use)) = (self.net, mem_use.net) {
            if net_limit < net_use {
                return true;
            }
        }

        false
    }
}
