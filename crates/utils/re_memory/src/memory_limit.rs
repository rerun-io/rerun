/// Represents a limit in how much RAM to use.
///
/// Different systems can chose to heed the memory limit in different ways,
/// e.g. by dropping old data when it is exceeded.
///
/// It is recommended that they log using [`re_log::info_once`] when they
/// drop data because a memory limit is reached.
#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(transparent)]
pub struct MemoryLimit {
    /// Limit in bytes.
    ///
    /// This is primarily compared to what is reported by [`crate::AccountingAllocator`] ('counted').
    /// We limit based on this instead of `resident` (RSS) because `counted` is what we have immediate
    /// control over, while RSS depends on what our allocator (MiMalloc) decides to do.
    ///
    /// None means "unlimited".
    max_bytes: Option<u64>,
}

/// Failed to parse a [`MemoryLimit`] with [`MemoryLimit::parse`].
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum MemoryLimitParseError {
    /// A percentage limit has an invalid number.
    #[error("expected e.g. '50%', got {limit:?}: {source}")]
    InvalidPercentage {
        /// The limit that was parsed.
        limit: String,

        /// Why the number is invalid.
        source: std::num::ParseFloatError,
    },

    /// A percentage limit is outside `[0, 100]`.
    #[error("percentage must be between 0 and 100, got {0}")]
    PercentageOutOfRange(f32),

    /// An absolute limit is not a valid byte size.
    #[error("expected e.g. '16GB', got {0:?}")]
    InvalidSize(String),

    /// An absolute limit is negative.
    #[error("memory limit must be non-negative, got {0}")]
    Negative(i64),
}

impl std::fmt::Display for MemoryLimit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.max_bytes {
            Some(max_bytes) => write!(f, "{}", re_format::format_bytes(max_bytes as _)),
            None => write!(f, "unlimited"),
        }
    }
}

impl MemoryLimit {
    /// Lowest possible limit: use as little memory as possible.
    pub const ZERO: Self = Self { max_bytes: Some(0) };

    /// No limit.
    pub const UNLIMITED: Self = Self { max_bytes: None };

    /// The default memory limit for native; 75% of reported
    /// system memory.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn default_for_current_platform() -> Self {
        Self::from_fraction_of_total(0.75)
    }

    /// The default memory for web, where we try to be extra careful
    /// to not oom.
    #[cfg(target_arch = "wasm32")]
    pub fn default_for_current_platform() -> Self {
        Self::from_bytes(2_500_000_000)
    }

    /// Set the limit to some number of bytes.
    pub fn from_bytes(max_bytes: u64) -> Self {
        Self {
            max_bytes: Some(max_bytes),
        }
    }

    /// Set the limit to some fraction (0-1) of the total available RAM.
    pub fn from_fraction_of_total(fraction: f32) -> Self {
        let total_memory = crate::total_ram_in_bytes();
        if let Some(total_memory) = total_memory {
            let max_bytes = (fraction as f64 * total_memory as f64).round();

            re_log::debug_once!(
                "Setting memory limit to {}, which is {}% of total available memory ({}).",
                re_format::format_bytes(max_bytes),
                100.0 * fraction,
                re_format::format_bytes(total_memory as _),
            );

            Self {
                max_bytes: Some(max_bytes as _),
            }
        } else {
            re_log::info_once!(
                "Couldn't determine total available memory. Setting no memory limit."
            );
            Self { max_bytes: None }
        }
    }

    /// The limit can either be absolute (e.g. "16GB") or relative (e.g. "50%").
    pub fn parse(limit: &str) -> Result<Self, MemoryLimitParseError> {
        if limit == "0" {
            // Let's be explicit: zero means zero,
            // not "unlimited" or any such shenanigans.
            Ok(Self::from_bytes(0))
        } else if matches!(limit, "unlimited" | "none" | "max" | "∞") {
            Ok(Self::UNLIMITED)
        } else if let Some(percentage) = limit.strip_suffix('%') {
            let percentage = percentage.parse::<f32>().map_err(|source| {
                MemoryLimitParseError::InvalidPercentage {
                    limit: limit.to_owned(),
                    source,
                }
            })?;

            if percentage < 0.0 || 100.0 < percentage {
                return Err(MemoryLimitParseError::PercentageOutOfRange(percentage));
            }

            let fraction = percentage / 100.0;
            Ok(Self::from_fraction_of_total(fraction))
        } else {
            let num_bytes = re_format::parse_bytes(limit)
                .ok_or_else(|| MemoryLimitParseError::InvalidSize(limit.to_owned()))?;

            if num_bytes < 0 {
                return Err(MemoryLimitParseError::Negative(num_bytes));
            }

            Ok(Self {
                max_bytes: Some(num_bytes as u64),
            })
        }
    }

    /// Returns [`u64::MAX`] if unlimited.
    pub fn as_bytes(&self) -> u64 {
        self.max_bytes.unwrap_or(u64::MAX)
    }

    #[inline]
    pub fn is_limited(&self) -> bool {
        self.max_bytes.is_some()
    }

    #[inline]
    pub fn is_unlimited(&self) -> bool {
        self.max_bytes.is_none()
    }

    /// Take the max of self and the given argument.
    #[must_use]
    pub fn at_least(self, min_bytes: u64) -> Self {
        if let Some(max_bytes) = self.max_bytes {
            Self::from_bytes(max_bytes.max(min_bytes))
        } else {
            Self::UNLIMITED
        }
    }

    #[must_use]
    pub fn saturating_sub(self, rhs: u64) -> Self {
        if let Some(max_bytes) = self.max_bytes {
            let new_max = max_bytes.saturating_sub(rhs);
            Self::from_bytes(new_max)
        } else {
            Self::UNLIMITED
        }
    }

    #[must_use]
    pub fn saturating_add(self, rhs: u64) -> Self {
        if let Some(max_bytes) = self.max_bytes {
            let new_max = max_bytes.saturating_add(rhs);
            Self::from_bytes(new_max)
        } else {
            Self::UNLIMITED
        }
    }

    #[must_use]
    pub fn checked_sub(self, rhs: u64) -> Option<Self> {
        if let Some(max_bytes) = self.max_bytes {
            let new_max = max_bytes.checked_sub(rhs)?;
            Some(Self::from_bytes(new_max))
        } else {
            Some(Self::UNLIMITED)
        }
    }

    /// Split the memory limit into two limits according to the given fraction.
    ///
    /// The first returned limit will have `fraction` of the bytes,
    /// and the second will have the rest.
    ///
    /// `fraction` should be between 0.0 and 1.0.
    pub fn split(self, fraction: f32) -> (Self, Self) {
        re_log::debug_assert!(
            (0.0..=1.0).contains(&fraction),
            "fraction must be between 0.0 and 1.0, got {fraction}"
        );
        if let Some(max_bytes) = self.max_bytes {
            let first_bytes = (fraction as f64 * max_bytes as f64).round() as u64;
            let second_bytes = max_bytes - first_bytes;
            (
                Self::from_bytes(first_bytes),
                Self::from_bytes(second_bytes),
            )
        } else {
            (Self::UNLIMITED, Self::UNLIMITED)
        }
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
    type Err = MemoryLimitParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_unlimited() {
        assert_eq!(MemoryLimit::parse("unlimited"), Ok(MemoryLimit::UNLIMITED));
        assert_eq!(MemoryLimit::parse("none"), Ok(MemoryLimit::UNLIMITED));
        assert_eq!(MemoryLimit::parse("max"), Ok(MemoryLimit::UNLIMITED));
        assert_eq!(MemoryLimit::parse("∞"), Ok(MemoryLimit::UNLIMITED));
    }

    #[test]
    fn test_parse_zero() {
        assert_eq!(MemoryLimit::parse("0"), Ok(MemoryLimit::from_bytes(0)));
    }

    #[test]
    fn test_parse_bytes() {
        assert_eq!(MemoryLimit::parse("123B"), Ok(MemoryLimit::from_bytes(123)));
        assert_eq!(MemoryLimit::parse("1kB"), Ok(MemoryLimit::from_bytes(1000)));
        assert_eq!(
            MemoryLimit::parse("1KiB"),
            Ok(MemoryLimit::from_bytes(1024))
        );
        assert_eq!(
            MemoryLimit::parse("1MB"),
            Ok(MemoryLimit::from_bytes(1000 * 1000))
        );
        assert_eq!(
            MemoryLimit::parse("1GB"),
            Ok(MemoryLimit::from_bytes(1000 * 1000 * 1000))
        );
        assert_eq!(
            MemoryLimit::parse("1GiB"),
            Ok(MemoryLimit::from_bytes(1024 * 1024 * 1024))
        );
    }

    #[test]
    fn test_parse_percentage() {
        let limit_50 = MemoryLimit::parse("50%");
        assert!(limit_50.is_ok());
        let limit_100 = MemoryLimit::parse("100%");
        assert!(limit_100.is_ok());
    }

    #[test]
    fn test_parse_invalid() {
        assert!(MemoryLimit::parse("").is_err());
        assert!(MemoryLimit::parse("foobar").is_err());
        assert!(MemoryLimit::parse("1023").is_err(), "Missing unit");
        assert!(MemoryLimit::parse("-1").is_err());
        assert!(MemoryLimit::parse("-1GB").is_err());
        assert!(MemoryLimit::parse("-10%").is_err());
    }
}
