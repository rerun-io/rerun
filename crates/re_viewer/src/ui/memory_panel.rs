use re_memory::{MemoryHistory, MemoryUse};

// ----------------------------------------------------------------------------

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct MemoryLimit {
    /// Limit in bytes compared to what is reported by OS.
    ///
    /// Resident Set Size (RSS) on Linux, Android, Mac, iOS.
    /// Working Set on Windows.
    pub gross: Option<i64>,

    /// Limit in bytes based compared to what is reported by [`re_memory::TrackingAllocator`].
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

            // TODO(emilk): show usage by different parts of the system. `if false` until then
            if false {
                ui.label(format!(
                    "{:.2} MB used by the string interner",
                    re_string_interner::bytes_used() as f32 / 1e6 // usually zero MB
                ));
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
