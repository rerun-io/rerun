use saturating_cast::SaturatingCast as _;

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
    pub max_bytes: Option<u64>,
}

impl MemoryLimit {
    /// No limit.
    pub const UNLIMITED: Self = Self { max_bytes: None };

    /// Set the limit to some number of bytes.
    pub fn from_bytes(max_bytes: u64) -> Self {
        Self {
            max_bytes: Some(max_bytes.saturating_cast()),
        }
    }

    /// Set the limit to some fraction (0-1) of the total available RAM.
    pub fn from_fraction_of_total(fraction: f32) -> Self {
        let total_memory = crate::total_ram_in_bytes();
        if let Some(total_memory) = total_memory {
            let max_bytes = (fraction as f64 * total_memory as f64).round();

            re_log::debug!(
                "Setting memory limit to {}, which is {}% of total available memory ({}).",
                re_format::format_bytes(max_bytes),
                100.0 * fraction,
                re_format::format_bytes(total_memory as _),
            );

            Self {
                max_bytes: Some(max_bytes as _),
            }
        } else {
            re_log::info!("Couldn't determine total available memory. Setting no memory limit.");
            Self { max_bytes: None }
        }
    }

    /// The limit can either be absolute (e.g. "16GB") or relative (e.g. "50%").
    pub fn parse(limit: &str) -> Result<Self, String> {
        if limit == "0" {
            // Let's be explicit: zero means zero,
            // not "unlimited" or any such shenanigans.
            Ok(Self::from_bytes(0))
        } else if matches!(limit, "unlimited" | "none" | "max" | "∞") {
            Ok(Self::UNLIMITED)
        } else if let Some(percentage) = limit.strip_suffix('%') {
            let percentage = percentage
                .parse::<f32>()
                .map_err(|_err| format!("expected e.g. '50%', got {limit:?}"))?;
            let fraction = percentage / 100.0;
            Ok(Self::from_fraction_of_total(fraction))
        } else {
            re_format::parse_bytes(limit)
                .map(|max_bytes| Self {
                    max_bytes: Some(max_bytes.max(0) as _),
                })
                .ok_or_else(|| format!("expected e.g. '16GB', got {limit:?}"))
        }
    }

    #[inline]
    pub fn is_limited(&self) -> bool {
        self.max_bytes.is_some()
    }

    #[inline]
    pub fn is_unlimited(&self) -> bool {
        self.max_bytes.is_none()
    }

    /// Returns how large fraction of memory we should free to go down to the exact limit.
    pub fn is_exceeded_by(&self, mem_use: &crate::MemoryUse) -> Option<f32> {
        let max_bytes = self.max_bytes?;

        if let Some(counted_use) = mem_use.counted {
            if max_bytes < counted_use {
                return Some((counted_use - max_bytes) as f32 / counted_use as f32);
            }
        } else if let Some(resident_use) = mem_use.resident {
            re_log::warn_once!(
                "Using resident memory use (RSS) for memory limiting, because a memory tracker was not available."
            );
            if max_bytes < resident_use {
                return Some((resident_use - max_bytes) as f32 / resident_use as f32);
            }
        }

        None
    }
}

impl std::str::FromStr for MemoryLimit {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}
