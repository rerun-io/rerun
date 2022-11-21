use re_memory::{
    util::{format_bytes, sec_since_start},
    MemoryHistory, MemoryLimit, MemoryUse,
};

// ----------------------------------------------------------------------------

#[derive(Default)]
pub struct MemoryPanel {
    history: MemoryHistory,
    memory_purge_times: Vec<f64>,
}

impl MemoryPanel {
    /// Call once per frame
    pub fn update(&mut self) {
        crate::profile_function!();
        self.history.capture();
    }

    /// Note that we purged memory at this time, to show in stats.
    pub fn note_memory_purge(&mut self) {
        self.memory_purge_times.push(sec_since_start());
    }

    pub fn ui(&self, ui: &mut egui::Ui) {
        crate::profile_function!();

        // We show realtime stats, so keep showing the latest!
        ui.ctx().request_repaint();

        egui::SidePanel::left("not_the_plot")
            .resizable(false)
            .min_width(250.0)
            .default_width(300.0)
            .show_inside(ui, |ui| {
                Self::left_side(ui);
            });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.label("🗠 Memory use over time");
            self.plot(ui);
        });
    }

    fn left_side(ui: &mut egui::Ui) {
        let limit = MemoryLimit::from_env_var("RERUN_MEMORY_LIMIT");
        if let Some(gross_limit) = limit.gross {
            ui.label(format!(
                "Gross limit: {} (set by RERUN_MEMORY_LIMIT)",
                format_bytes(gross_limit as _)
            ));
        } else {
            ui.label("You can use the environment variable RERUN_MEMORY_LIMIT to set an upper limit of memory use. For instance: 'RERUN_MEMORY_LIMIT=16GB'.");
            ui.separator();
        }

        let mem_use = MemoryUse::capture();

        if mem_use.gross.is_some() || mem_use.net.is_some() {
            if let Some(gross) = mem_use.gross {
                ui.label(format!(
                    "Gross: {} (allocated from OS)",
                    format_bytes(gross as _)
                ));
            }

            if let Some(net) = mem_use.net {
                ui.label(format!("Net: {} (actually used)", format_bytes(net as _)));
            } else if cfg!(debug_assertions) {
                ui.label("Memory tracking allocator not installed.");
            }
        }

        let max_callstacks = 100;
        if let Some(tracking_stats) = re_memory::tracking_allocator::tracking_stats(max_callstacks)
        {
            ui.style_mut().wrap = Some(false);
            Self::tracking_stats(ui, tracking_stats);
        } else {
            ui.separator();
            ui.label("Set RERUN_TRACK_ALLOCATIONS=1 to turn on detailed allocation tracking.");
        }
    }

    fn tracking_stats(
        ui: &mut egui::Ui,
        tracking_stats: re_memory::tracking_allocator::TrackingStatistics,
    ) {
        ui.label(format!(
            "{} tracked in {} allocs",
            format_bytes(tracking_stats.tracked_bytes as _),
            format_count(tracking_stats.tracked_allocs),
        ));
        ui.label(format!(
            "{} untracked in {} allocs (all smaller than {})",
            format_bytes(tracking_stats.untracked_bytes as _),
            format_count(tracking_stats.untracked_allocs),
            format_bytes(tracking_stats.track_size_threshold as _),
        ));
        ui.label(format!(
            "{} in {} allocs used for the book-keeping of the allocation tracker",
            format_bytes(tracking_stats.tracker_bytes as _),
            format_count(tracking_stats.tracker_allocs),
        ));

        egui::CollapsingHeader::new("Top memory consumers")
            .default_open(true)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(300.0)
                    .show(ui, |ui| {
                        ui.set_min_width(750.0);
                        for callstack in tracking_stats.top_callstacks {
                            if ui
                                .button(format!(
                                    "{} in {} allocs (≈{} / alloc) - {}",
                                    format_bytes(callstack.extant_bytes as _),
                                    format_count(callstack.extant_allocs),
                                    format_bytes(
                                        callstack.extant_bytes as f64
                                            / callstack.extant_allocs as f64
                                    ),
                                    summarize_callstack(&callstack.readable_backtrace.to_string())
                                ))
                                .on_hover_text("Click to copy callstack to clipboard")
                                .clicked()
                            {
                                ui.output().copied_text = callstack.readable_backtrace.to_string();
                            }
                        }
                    });
            });
    }

    fn plot(&self, ui: &mut egui::Ui) {
        crate::profile_function!();

        use itertools::Itertools as _;

        fn to_line(history: &egui::util::History<i64>) -> egui::plot::Line {
            egui::plot::Line::new(
                history
                    .iter()
                    .map(|(time, bytes)| [time, bytes as f64])
                    .collect_vec(),
            )
        }

        egui::plot::Plot::new("mem_history_plot")
            .min_size(egui::Vec2::splat(200.0))
            .label_formatter(|name, value| format!("{}: {}", name, format_bytes(value.y)))
            .x_axis_formatter(|time, _| format!("{} s", time))
            .y_axis_formatter(|bytes, _| format_bytes(bytes))
            .show_x(false)
            .legend(egui::plot::Legend::default().position(egui::plot::Corner::LeftTop))
            .include_y(0.0)
            // TODO(emilk): turn off plot interaction, and always do auto-sizing
            .show(ui, |plot_ui| {
                let limit = MemoryLimit::from_env_var("RERUN_MEMORY_LIMIT");
                if let Some(gross_limit) = limit.gross {
                    plot_ui.hline(
                        egui::plot::HLine::new(gross_limit as f64)
                            .name("Gross limit")
                            .width(1.0),
                    );
                }
                if let Some(net_limit) = limit.net {
                    plot_ui.hline(
                        egui::plot::HLine::new(net_limit as f64)
                            .name("Net limit")
                            .width(1.0),
                    );
                }

                for &time in &self.memory_purge_times {
                    plot_ui.vline(
                        egui::plot::VLine::new(time)
                            .name("RAM purge")
                            .color(egui::Color32::from_rgb(252, 161, 3))
                            .width(2.0),
                    );
                }

                plot_ui.line(to_line(&self.history.gross).name("Gross use").width(1.5));
                plot_ui.line(to_line(&self.history.net).name("Net use").width(1.5));
            });
    }
}

/// Using thousands separators readability.
fn format_count(number: usize) -> String {
    let number = number.to_string();
    let mut chars = number.chars().rev().peekable();

    let mut result = vec![];
    while chars.peek().is_some() {
        if !result.is_empty() {
            // thousands-deliminator:
            let thin_space = '\u{2009}'; // https://en.wikipedia.org/wiki/Thin_space
            result.push(thin_space);
        }
        for _ in 0..3 {
            if let Some(c) = chars.next() {
                result.push(c);
            }
        }
    }

    result.reverse();
    result.into_iter().collect()
}

#[test]
fn test_format_large_number() {
    assert_eq!(format_count(42), "42");
    assert_eq!(format_count(999), "999");
    assert_eq!(format_count(1_000), "1 000");
    assert_eq!(format_count(123_456), "123 456");
    assert_eq!(format_count(1_234_567), "1 234 567");
}

fn summarize_callstack(callstack: &str) -> String {
    let patterns = [
        ("LogDb", "LogDb"),
        ("ObjDb", "ObjDb"),
        ("ObjectTree", "ObjectTree"),
        ("TimelineStore", "TimelineStore"),
        ("ObjStore", "ObjStore"),
        ("::LogMsg>::deserialize", "LogMsg"),
        ("::TimePoint>::deserialize", "TimePoint"),
        ("gltf", "gltf"),
        ("image::image", "image"),
        // -----
        // Very general:
        ("std::sync::mpsc::Sender", "std::sync::mpsc::Sender"),
        ("epaint::texture_atlas", "egui font texture"),
        (
            "alloc::collections::btree::map::BTreeSet<K,V,A>",
            "BTreeSet",
        ),
        (
            "alloc::collections::btree::map::BTreeMap<K,V,A>",
            "BTreeMap",
        ),
        ("std::collections::hash::map::HashMap<K,V,S>", "HashMap"),
    ];

    let mut all_summaries = vec![];

    for (pattern, summary) in patterns {
        if callstack.contains(pattern) {
            all_summaries.push(summary);
        }
    }

    all_summaries.join(", ")
}
