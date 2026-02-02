/// Amount of available RAM on this machine.
#[cfg(not(target_arch = "wasm32"))]
pub fn total_ram_in_bytes() -> Option<u64> {
    re_tracing::profile_function!();

    let mut sys = sysinfo::System::new_with_specifics(
        sysinfo::RefreshKind::new().with_memory(sysinfo::MemoryRefreshKind::new().with_ram()),
    );

    {
        re_tracing::profile_scope!("refresh_memory");
        sys.refresh_memory();
    }

    let bytes = sys.total_memory();
    if bytes == 0 { None } else { Some(bytes) }
}

/// Amount of available RAM on this machine.
#[cfg(target_arch = "wasm32")]
pub fn total_ram_in_bytes() -> Option<u64> {
    #![expect(clippy::unnecessary_wraps)]
    Some(1_u64 << 32)
}

// ----------------------------------------------------------------------------

/// Helper to warn if we are using too much RAM.
///
/// You need to call [`RamLimitWarner::update`] regularly.
pub struct RamLimitWarner {
    total_ram_in_bytes: u64,
    warn_limit: u64,
    has_warned: bool,
}

impl RamLimitWarner {
    pub fn warn_at_fraction_of_max(fraction: f32) -> Self {
        if let Some(total_ram_in_bytes) = total_ram_in_bytes() {
            let limit = (fraction as f64 * total_ram_in_bytes as f64).round() as _;
            Self {
                total_ram_in_bytes,
                warn_limit: limit,
                has_warned: false,
            }
        } else {
            re_log::warn_once!("Failed to figure out how much RAM this machine has");
            Self {
                total_ram_in_bytes: 0,
                warn_limit: 0,
                has_warned: true,
            }
        }
    }

    /// Warns if we have exceeded the limit.
    pub fn update(&mut self) {
        if !self.has_warned {
            let used = crate::MemoryUse::capture();
            let used = used.counted.or(used.resident);
            if let Some(used) = used
                && self.warn_limit <= used
            {
                self.has_warned = true;
                re_log::warn!(
                    "RAM usage is {} (with a total of {} system RAM). You may want to start Rerun with the --memory-limit flag to limit RAM usage.",
                    re_format::format_bytes(used as _),
                    re_format::format_bytes(self.total_ram_in_bytes as _),
                );
            }
        }
    }
}
