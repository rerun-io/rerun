/// Amount of available RAM on this machine.
#[cfg(not(target_arch = "wasm32"))]
pub fn total_ram_in_bytes() -> u64 {
    use sysinfo::SystemExt as _;

    let mut sys = sysinfo::System::new_all();
    sys.refresh_all();

    let total_memory = sys.total_memory();

    re_log::debug!(
        "Total RAM: {}",
        re_format::format_bytes(sys.total_memory() as _)
    );

    total_memory
}

/// Amount of available RAM on this machine.
#[cfg(target_arch = "wasm32")]
pub fn total_ram_in_bytes() -> u64 {
    1_u64 << 32
}

// ----------------------------------------------------------------------------

pub struct RamLimitWarner {
    total_ram_in_bytes: u64,
    limit: u64,
    has_warned: bool,
}

impl RamLimitWarner {
    pub fn warn_at_fraction_of_max(fraction: f32) -> Self {
        let total_ram_in_bytes = total_ram_in_bytes();
        let limit = (fraction as f64 * total_ram_in_bytes as f64).round() as _;
        Self {
            total_ram_in_bytes,
            limit,
            has_warned: false,
        }
    }

    /// Warns if we have exceeded the limit.
    pub fn update(&mut self) {
        if !self.has_warned {
            let used = crate::MemoryUse::capture();
            let used = used.counted.or(used.resident);
            if let Some(used) = used {
                if 0 <= used && self.limit <= used as u64 {
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
}
