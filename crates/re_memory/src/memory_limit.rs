/// Represents a limit in how much RAM to use for the entire process.
///
/// Different systems can chose to heed the memory limit in different ways,
/// e.g. by dropping old data when it is exceeded.
///
/// It is recommended that they log using [`re_log::info_once`] when they
/// drop data because a memory limit is reached.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct MemoryLimit {
    /// Limit in bytes.
    ///
    /// This is primarily compared to what is reported by [`crate::AccountingAllocator`] ('counted').
    /// We limit based on this instead of `resident` (RSS) because `counted` is what we have immediate
    /// control over, while RSS depends on what our allocator (MiMalloc) decides to do.
    pub limit: Option<i64>,
}

impl MemoryLimit {
    /// No limit.
    pub const UNLIMITED: Self = Self { limit: None };

    /// Set the limit to some number of bytes.
    pub fn from_bytes(max_bytes: u64) -> Self {
        Self {
            limit: Some(max_bytes as _),
        }
    }

    /// Set the limit to some fraction (0-1) of the total available RAM.
    pub fn from_fraction_of_total(fraction: f32) -> Self {
        let total_memory = crate::total_ram_in_bytes();
        if total_memory == 0 {
            re_log::info!("Couldn't determine total available memory. Setting no memory limit.");
            Self { limit: None }
        } else {
            let limit = (fraction as f64 * total_memory as f64).round();

            re_log::debug!(
                "Setting memory limit to {}, which is {}% of total available memory ({}).",
                re_format::format_bytes(limit),
                100.0 * fraction,
                re_format::format_bytes(total_memory as _),
            );

            Self {
                limit: Some(limit as _),
            }
        }
    }

    /// The limit can either be absolute (e.g. "16GB") or relative (e.g. "50%").
    pub fn parse(limit: &str) -> Result<Self, String> {
        if let Some(percentage) = limit.strip_suffix('%') {
            let percentage = percentage
                .parse::<f32>()
                .map_err(|_err| format!("expected e.g. '50%', got {limit:?}"))?;
            let fraction = percentage / 100.0;
            Ok(Self::from_fraction_of_total(fraction))
        } else {
            re_format::parse_bytes(limit)
                .map(|limit| Self { limit: Some(limit) })
                .ok_or_else(|| format!("expected e.g. '16GB', got {limit:?}"))
        }
    }

    #[inline]
    pub fn is_limited(&self) -> bool {
        self.limit.is_some()
    }

    #[inline]
    pub fn is_unlimited(&self) -> bool {
        self.limit.is_none()
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
