#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct MemoryUse {
    /// Bytes allocated by the application according to operating system.
    ///
    /// Resident Set Size (RSS) on Linux, Android, Mac, iOS.
    /// Working Set on Windows.
    ///
    /// `None` if unknown.
    pub gross: Option<i64>,

    /// Bytes used by the application according to our own memory allocator's accounting.
    ///
    /// This will be smaller than [`Self::gross`] because our memory allocator may not
    /// return all the memory we free to the OS.
    ///
    /// `None` if [`crate::mem_tracker::TrackingAllocator`] is not used.
    pub net: Option<i64>,
}

impl MemoryUse {
    pub fn capture() -> Self {
        crate::profile_function!();
        Self {
            gross: bytes_used_gross(),
            net: bytes_used_net(),
        }
    }
}

impl std::ops::Sub for MemoryUse {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        fn sub(a: Option<i64>, b: Option<i64>) -> Option<i64> {
            Some(a? - b?)
        }

        MemoryUse {
            gross: sub(self.gross, rhs.gross),
            net: sub(self.net, rhs.net),
        }
    }
}

// ----------------------------------------------------------------------------

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct MemoryLimit {
    /// Limit in bytes compared to what is reported by OS.
    ///
    /// Resident Set Size (RSS) on Linux, Android, Mac, iOS.
    /// Working Set on Windows.
    pub gross: Option<i64>,

    /// Limit in bytes based compared to what is reported by [`crate::mem_tracker::TrackingAllocator`].
    pub net: Option<i64>,
}

impl MemoryLimit {
    /// Read from `RERUN_MEMORY_LIMIT`.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_env_vars() -> Self {
        fn parse_limit(limit: &str) -> Option<i64> {
            Some(limit.strip_suffix("GB")?.parse::<i64>().ok()? * 1_000_000_000)
        }

        let gross_limit = std::env::var("RERUN_MEMORY_LIMIT").ok().map(|limit| {
            parse_limit(&limit)
                .unwrap_or_else(|| panic!("RERUN_MEMORY_LIMIT: expected e.g. '16GB'"))
        });

        Self {
            gross: gross_limit,

            // Start freeing a bit before we reach OS limit:
            net: gross_limit.map(|g| g / 4 * 3),
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn from_env_vars() -> Self {
        // TODO(emilk): some way to have memory limits on web.
        Self {
            gross: None,
            net: None,
        }
    }

    pub fn is_exceeded_by(&self, mem_use: &MemoryUse) -> bool {
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

// ----------------------------------------------------------------------------

/// According to the OS. This is what matters.
///
/// Resident Set Size (RSS) on Linux, Android, Mac, iOS.
/// Working Set on Windows.
#[cfg(not(target_arch = "wasm32"))]
fn bytes_used_gross() -> Option<i64> {
    memory_stats::memory_stats().map(|usage| usage.physical_mem as i64)
}

#[cfg(target_arch = "wasm32")]
fn bytes_used_gross() -> Option<i64> {
    // blocked on https://github.com/Arc-blroth/memory-stats/issues/1 and https://github.com/rustwasm/wasm-bindgen/issues/3159
    None
}

/// The amount of memory in use.
///
/// The difference to [`bytes_used_gross`] is memory allocated by `MiMalloc`.
/// that hasn't been returned to the OS.
///
/// `None` if [`crate::mem_tracker::TrackingAllocator`] is not used.
fn bytes_used_net() -> Option<i64> {
    let num_bytes = crate::mem_tracker::global_allocs_and_bytes().1;
    if num_bytes == 0 {
        None
    } else {
        Some(num_bytes as _)
    }
}

// ----------------------------------------------------------------------------

/// Returns monotonically increasing time in seconds.
#[inline]
fn now_sec() -> f64 {
    use instant::Instant;
    use once_cell::sync::Lazy;

    static START_INSTANT: Lazy<Instant> = Lazy::new(Instant::now);
    START_INSTANT.elapsed().as_nanos() as f64 / 1e9
}

// ----------------------------------------------------------------------------

/// Tracks memory use over time.
struct MemoryHistory {
    pub gross: egui::util::History<i64>,
    pub net: egui::util::History<i64>,
}

impl Default for MemoryHistory {
    fn default() -> Self {
        let max_elems = 128 * 1024;
        let max_seconds = f32::INFINITY;
        Self {
            gross: egui::util::History::new(0..max_elems, max_seconds),
            net: egui::util::History::new(0..max_elems, max_seconds),
        }
    }
}

impl MemoryHistory {
    pub fn is_empty(&self) -> bool {
        let Self { gross, net } = self;
        gross.is_empty() && net.is_empty()
    }

    /// Add data to history
    pub fn capture(&mut self) {
        let mem_use = MemoryUse::capture();
        if mem_use.gross.is_some() || mem_use.net.is_some() {
            let now = now_sec();
            if let Some(gross) = mem_use.gross {
                self.gross.add(now, gross);
            }
            if let Some(net) = mem_use.net {
                self.net.add(now, net);
            }
        }
    }
}

// ----------------------------------------------------------------------------

#[derive(Default)]
pub struct MemoryPanel {
    history: MemoryHistory,
}

impl MemoryPanel {
    /// Call once per frame
    pub fn update(&mut self) {
        crate::profile_function!();
        self.history.capture();
    }

    pub fn ui(&self, ui: &mut egui::Ui) {
        // We show realtime stats, so keep showing the latest!
        ui.ctx().request_repaint();

        let mem_use = MemoryUse::capture();

        if mem_use.gross.is_some() || mem_use.net.is_some() {
            if let Some(gross) = mem_use.gross {
                ui.label(format!(
                    "Gross: {:.2} GB (allocated from OS)",
                    gross as f32 / 1e9
                ));
            }

            if let Some(net) = mem_use.net {
                ui.label(format!("Net: {:.2} GB (actually used)", net as f32 / 1e9));
            } else if cfg!(debug_assertions) {
                ui.label("Memory tracking allocator not installed.");
            }

            if false {
                ui.label(format!(
                    "{:.2} MB used by the string interner",
                    re_string_interner::bytes_used() as f32 / 1e6
                ));

                // TODO(emilk): show usage by different parts of the system
            }
        }

        let limit = MemoryLimit::from_env_vars();
        if let Some(gross_limit) = limit.gross {
            ui.label(format!(
                "Gross limit: {:.2} GB (set by RERUN_MEMORY_LIMIT)",
                gross_limit as f32 / 1e9
            ));
        } else {
            ui.label("You can use the environment variable RERUN_MEMORY_LIMIT to set an upper limit of memory use. For instance: 'RERUN_MEMORY_LIMIT=16GB'.");
        }

        if !self.history.is_empty() {
            self.plot(ui);
        }
    }

    fn plot(&self, ui: &mut egui::Ui) {
        use itertools::Itertools as _;

        fn to_line(history: &egui::util::History<i64>) -> egui::plot::Line {
            egui::plot::Line::new(
                history
                    .iter()
                    .map(|(time, bytes)| [time, bytes as f64 / 1e9])
                    .collect_vec(),
            )
        }

        egui::plot::Plot::new("mem_history_plot")
            .include_y(0.0)
            .label_formatter(|name, value| format!("{}: {:.2} GB", name, value.y))
            .x_axis_formatter(|time, _| format!("{} s", time))
            .y_axis_formatter(|gb, _| format!("{} GB", gb))
            .show_x(false)
            .legend(egui::plot::Legend::default().position(egui::plot::Corner::LeftTop))
            .show(ui, |plot_ui| {
                plot_ui.line(to_line(&self.history.gross).name("gross (GB)"));
                plot_ui.line(to_line(&self.history.net).name("net (GB)"));
            });
    }
}
