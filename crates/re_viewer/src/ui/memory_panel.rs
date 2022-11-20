pub struct MemoryUse {
    /// Bytes allocated by the application according to operating system.
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

/// According to the OS. This is what matters.
#[cfg(not(target_arch = "wasm32"))]
fn bytes_used_gross() -> Option<i64> {
    memory_stats::memory_stats().map(|usage| usage.physical_mem as i64)
}

#[cfg(target_arch = "wasm32")]
fn bytes_used_gross() -> Option<i64> {
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
    // This can maybe be optimized
    use instant::Instant;
    use once_cell::sync::Lazy;

    static START_INSTANT: Lazy<Instant> = Lazy::new(Instant::now);
    START_INSTANT.elapsed().as_nanos() as f64 / 1e9
}

// ----------------------------------------------------------------------------

/// Tracks memory use over time,.
pub struct MemoryHistory {
    pub gross: egui::util::History<i64>,
    pub net: egui::util::History<i64>,
}

impl Default for MemoryHistory {
    fn default() -> Self {
        let max_elems = 128 * 1024;
        Self {
            gross: egui::util::History::new(0..max_elems, f32::INFINITY),
            net: egui::util::History::new(0..max_elems, f32::INFINITY),
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

        let mem_use = crate::memory_panel::MemoryUse::capture();
        if mem_use.gross.is_some() || mem_use.net.is_some() {
            if let Some(gross) = mem_use.gross {
                ui.label(format!(
                    "Gross: {:.2} GB (allocated from OS)",
                    gross as f32 / 1e9
                ));
            }
            if let Some(net) = mem_use.net {
                ui.label(format!("Net: {:.2} GB (actually used)", net as f32 / 1e9));
            }

            if false {
                ui.label(format!(
                    "{:.2} MB used by the string interner",
                    re_string_interner::bytes_used() as f32 / 1e6
                ));

                // TODO(emilk): show usage by different parts of the system
            }

            ui.separator();
        }

        if !self.history.is_empty() {
            self.plot(ui);
        }
    }

    fn plot(&self, ui: &mut egui::Ui) {
        use itertools::Itertools as _;

        egui::plot::Plot::new("mem_history_plot")
            .include_y(0.0)
            .label_formatter(|name, value| format!("{}: {:.2} GB", name, value.y))
            .x_axis_formatter(|time, _| format!("{} s", time))
            .y_axis_formatter(|gb, _| format!("{} GB", gb))
            .show_x(false)
            .legend(egui::plot::Legend::default().position(egui::plot::Corner::LeftTop))
            .show(ui, |plot_ui| {
                plot_ui.line(
                    egui::plot::Line::new(
                        self.history
                            .gross
                            .iter()
                            .map(|(time, bytes)| [time, bytes as f64 / 1e9])
                            .collect_vec(),
                    )
                    .name("gross (GB)"),
                );
                plot_ui.line(
                    egui::plot::Line::new(
                        self.history
                            .net
                            .iter()
                            .map(|(time, bytes)| [time, bytes as f64 / 1e9])
                            .collect_vec(),
                    )
                    .name("net (GB)"),
                );
            });
    }
}
