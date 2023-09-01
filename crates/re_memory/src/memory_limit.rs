#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct MemoryLimit {
    /// Limit in bytes.
    ///
    /// This is primarily compared to what is reported by [`crate::AccountingAllocator`] ('counted').
    /// We limit based on this instead of `resident` (RSS) because `counted` is what we have immediate
    /// control over, while RSS depends on what our allocator (MiMalloc) decides to do.
    pub limit: Option<i64>,
}

impl MemoryLimit {
    /// The limit can either be absolute (e.g. "16GB") or relative (e.g. "50%").
    pub fn parse(limit: &str) -> Result<Self, String> {
        if let Some(percentage) = limit.strip_suffix('%') {
            let percentage = percentage
                .parse::<f32>()
                .map_err(|_err| format!("expected e.g. '50%', got {limit:?}"))?;

            let total_memory = crate::total_ram_in_bytes();
            if total_memory == 0 {
                re_log::info!(
                    "Couldn't determine total available memory. Setting no memory limit."
                );
                Ok(Self { limit: None })
            } else {
                let limit = (total_memory as f64 * (percentage as f64 / 100.0)).round();

                re_log::debug!(
                    "Setting memory limit to {}, which is {percentage}% of total available memory ({}).",
                    re_format::format_bytes(limit),
                    re_format::format_bytes(total_memory as _),
            );

                Ok(Self {
                    limit: Some(limit as _),
                })
            }
        } else {
            re_format::parse_bytes(limit)
                .map(|limit| Self { limit: Some(limit) })
                .ok_or_else(|| format!("expected e.g. '16GB', got {limit:?}"))
        }
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
