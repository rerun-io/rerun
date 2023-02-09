use re_arrow_store::DataStoreStats;
use re_format::{format_bytes, format_number};
use re_memory::{util::sec_since_start, MemoryHistory, MemoryLimit, MemoryUse};
use re_renderer::WgpuResourcePoolStatistics;

use crate::env_vars::RERUN_TRACK_ALLOCATIONS;

// ----------------------------------------------------------------------------

#[derive(Default)]
pub struct MemoryPanel {
    history: MemoryHistory,
    memory_purge_times: Vec<f64>,
}

impl MemoryPanel {
    /// Call once per frame
    pub fn update(
        &mut self,
        gpu_resource_stats: &WgpuResourcePoolStatistics,
        store_stats: &DataStoreStats,
    ) {
        crate::profile_function!();
        self.history.capture(
            Some(
                (gpu_resource_stats.total_buffer_size_in_bytes
                    + gpu_resource_stats.total_texture_size_in_bytes) as _,
            ),
            Some(
                (store_stats.total_index_size_bytes + store_stats.total_component_size_bytes) as _,
            ),
        );
    }

    /// Note that we purged memory at this time, to show in stats.
    pub fn note_memory_purge(&mut self) {
        self.memory_purge_times.push(sec_since_start());
    }

    pub fn ui(
        &self,
        ui: &mut egui::Ui,
        limit: &MemoryLimit,
        gpu_resource_stats: &WgpuResourcePoolStatistics,
        store_stats: &DataStoreStats,
    ) {
        crate::profile_function!();

        // We show realtime stats, so keep showing the latest!
        ui.ctx().request_repaint();

        egui::SidePanel::left("not_the_plot")
            .resizable(false)
            .min_width(250.0)
            .default_width(300.0)
            .show_inside(ui, |ui| {
                Self::left_side(ui, limit, gpu_resource_stats, store_stats);
            });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.label("🗠 Rerun Viewer memory use over time");
            self.plot(ui, limit);
        });
    }

    fn left_side(
        ui: &mut egui::Ui,
        limit: &MemoryLimit,
        gpu_resource_stats: &WgpuResourcePoolStatistics,
        store_stats: &DataStoreStats,
    ) {
        ui.strong("Rerun Viewer resource usage");

        ui.separator();
        ui.collapsing("CPU Resources", |ui| {
            Self::cpu_stats(ui, limit);
        });

        ui.separator();
        ui.collapsing("GPU Resources", |ui| {
            Self::gpu_stats(ui, gpu_resource_stats);
        });

        ui.separator();
        ui.collapsing("Datastore Resources", |ui| {
            Self::store_stats(ui, store_stats);
        });
    }

    fn cpu_stats(ui: &mut egui::Ui, limit: &MemoryLimit) {
        if let Some(limit) = limit.limit {
            ui.label(format!("Memory limit: {}", format_bytes(limit as _)));
        } else {
            ui.label(
                "You can set an upper limit of RAM use with the command-lime option --memory-limit",
            );
            ui.separator();
        }

        let mem_use = MemoryUse::capture();

        if mem_use.resident.is_some() || mem_use.counted.is_some() {
            if let Some(resident) = mem_use.resident {
                ui.label(format!("resident: {}", format_bytes(resident as _)))
                    .on_hover_text("Resident Set Size (or Working Set on Windows). Memory in RAM and not in swap.");
            }

            if let Some(counted) = mem_use.counted {
                ui.label(format!("counted: {}", format_bytes(counted as _)))
                    .on_hover_text("Live bytes, counted by our own allocator.");
            } else if cfg!(debug_assertions) {
                ui.label("Memory-tracking allocator not installed.");
            }
        }

        if let Some(tracking_stats) = re_memory::accounting_allocator::tracking_stats() {
            ui.style_mut().wrap = Some(false);
            Self::tracking_stats(ui, tracking_stats);
        } else {
            ui.separator();
            ui.label(format!(
                "Set {RERUN_TRACK_ALLOCATIONS}=1 to turn on detailed allocation tracking."
            ));
        }
    }

    fn gpu_stats(ui: &mut egui::Ui, gpu_resource_stats: &WgpuResourcePoolStatistics) {
        egui::Grid::new("gpu resource grid")
            .num_columns(2)
            .show(ui, |ui| {
                let WgpuResourcePoolStatistics {
                    num_bind_group_layouts,
                    num_pipeline_layouts,
                    num_render_pipelines,
                    num_samplers,
                    num_shader_modules,
                    num_bind_groups,
                    num_buffers,
                    num_textures,
                    total_buffer_size_in_bytes,
                    total_texture_size_in_bytes,
                } = gpu_resource_stats;

                ui.label("# Bind Group Layouts:");
                ui.label(num_bind_group_layouts.to_string());
                ui.end_row();
                ui.label("# Pipeline Layouts:");
                ui.label(num_pipeline_layouts.to_string());
                ui.end_row();
                ui.label("# Render Pipelines:");
                ui.label(num_render_pipelines.to_string());
                ui.end_row();
                ui.label("# Samplers:");
                ui.label(num_samplers.to_string());
                ui.end_row();
                ui.label("# Shader Modules:");
                ui.label(num_shader_modules.to_string());
                ui.end_row();
                ui.label("# Bind Groups:");
                ui.label(num_bind_groups.to_string());
                ui.end_row();
                ui.label("# Buffers:");
                ui.label(num_buffers.to_string());
                ui.end_row();
                ui.label("# Textures:");
                ui.label(num_textures.to_string());
                ui.end_row();
                ui.label("Buffer Memory:");
                ui.label(re_format::format_bytes(*total_buffer_size_in_bytes as _));
                ui.end_row();
                ui.label("Texture Memory:");
                ui.label(re_format::format_bytes(*total_texture_size_in_bytes as _));
                ui.end_row();
            });
    }

    fn store_stats(ui: &mut egui::Ui, store_stats: &DataStoreStats) {
        egui::Grid::new("gpu resource grid")
            .num_columns(3)
            .show(ui, |ui| {
                let DataStoreStats {
                    total_timeless_index_rows,
                    total_timeless_index_size_bytes,
                    total_timeless_component_rows,
                    total_timeless_component_size_bytes,
                    total_temporal_index_rows,
                    total_temporal_index_size_bytes,
                    total_temporal_component_rows,
                    total_temporal_component_size_bytes,
                    total_index_rows,
                    total_index_size_bytes,
                    total_component_rows,
                    total_component_size_bytes,
                } = *store_stats;

                let label_rows = |ui: &mut egui::Ui, num_rows| {
                    ui.label(format!("rows: {}", re_format::format_number(num_rows as _)))
                };
                let label_size =
                    |ui: &mut egui::Ui, size| ui.label(re_format::format_bytes(size as _));

                ui.label("Indices (timeless):");
                label_rows(ui, total_timeless_index_rows);
                label_size(ui, total_timeless_index_size_bytes);
                ui.end_row();

                ui.label("Indices (temporal):");
                label_rows(ui, total_temporal_index_rows);
                label_size(ui, total_temporal_index_size_bytes);
                ui.end_row();

                ui.label("Indices (total):");
                label_rows(ui, total_index_rows);
                label_size(ui, total_index_size_bytes);
                ui.end_row();

                ui.label("Components (timeless):");
                label_rows(ui, total_timeless_component_rows);
                label_size(ui, total_timeless_component_size_bytes);
                ui.end_row();

                ui.label("Components (temporal):");
                label_rows(ui, total_temporal_component_rows);
                label_size(ui, total_temporal_component_size_bytes);
                ui.end_row();

                ui.label("Components (total):");
                label_rows(ui, total_component_rows);
                label_size(ui, total_component_size_bytes);
                ui.end_row();
            });
    }

    fn tracking_stats(
        ui: &mut egui::Ui,
        tracking_stats: re_memory::accounting_allocator::TrackingStatistics,
    ) {
        ui.label("counted = fully_tracked + stochastically_tracked + untracked + overhead");
        ui.label(format!(
            "fully_tracked: {} in {} allocs",
            format_bytes(tracking_stats.fully_tracked.size as _),
            format_number(tracking_stats.fully_tracked.count),
        ));
        ui.label(format!(
            "stochastically_tracked: {} in {} allocs",
            format_bytes(tracking_stats.stochastically_tracked.size as _),
            format_number(tracking_stats.stochastically_tracked.count),
        ));
        ui.label(format!(
            "untracked: {} in {} allocs (all smaller than {})",
            format_bytes(tracking_stats.untracked.size as _),
            format_number(tracking_stats.untracked.count),
            format_bytes(tracking_stats.track_size_threshold as _),
        ));
        ui.label(format!(
            "overhead: {} in {} allocs",
            format_bytes(tracking_stats.overhead.size as _),
            format_number(tracking_stats.overhead.count),
        ))
        .on_hover_text("Used for the book-keeping of the allocation tracker");

        egui::CollapsingHeader::new("Top memory consumers")
            .default_open(true)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(300.0)
                    .show(ui, |ui| {
                        ui.set_min_width(750.0);
                        for callstack in tracking_stats.top_callstacks {
                            let stochastic_rate = callstack.stochastic_rate;
                            let is_stochastic = stochastic_rate > 1;

                            if ui
                                .button(format!(
                                    "{}{} in {} allocs (≈{} / alloc){} - {}",
                                    if is_stochastic { "≈" } else { "" },
                                    format_bytes((callstack.extant.size * stochastic_rate) as _),
                                    format_number(callstack.extant.count * stochastic_rate),
                                    format_bytes(
                                        callstack.extant.size as f64
                                            / callstack.extant.count as f64
                                    ),
                                    if stochastic_rate <= 1 {
                                        String::new()
                                    } else {
                                        format!(" ({} stochastic samples)", callstack.extant.count)
                                    },
                                    summarize_callstack(&callstack.readable_backtrace.to_string())
                                ))
                                .on_hover_text("Click to copy callstack to clipboard")
                                .clicked()
                            {
                                ui.output_mut(|o| {
                                    o.copied_text = callstack.readable_backtrace.to_string();
                                });
                            }
                        }
                    });
            });
    }

    fn plot(&self, ui: &mut egui::Ui, limit: &MemoryLimit) {
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
            .label_formatter(|name, value| format!("{name}: {}", format_bytes(value.y)))
            .x_axis_formatter(|time, _| format!("{time} s"))
            .y_axis_formatter(|bytes, _| format_bytes(bytes))
            .show_x(false)
            .legend(egui::plot::Legend::default().position(egui::plot::Corner::LeftTop))
            .include_y(0.0)
            // TODO(emilk): turn off plot interaction, and always do auto-sizing
            .show(ui, |plot_ui| {
                if let Some(counted_limit) = limit.limit {
                    plot_ui.hline(
                        egui::plot::HLine::new(counted_limit as f64)
                            .name("Limit (counted)")
                            .width(2.0),
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

                let MemoryHistory {
                    resident,
                    counted,
                    counted_gpu,
                    counted_store,
                } = &self.history;

                plot_ui.line(to_line(resident).name("Resident").width(1.5));
                plot_ui.line(to_line(counted).name("Counted").width(1.5));
                plot_ui.line(to_line(counted_gpu).name("Counted GPU").width(1.5));
                plot_ui.line(to_line(counted_store).name("Counted Store").width(1.5));
            });
    }
}

fn summarize_callstack(callstack: &str) -> String {
    let patterns = [
        ("LogDb", "LogDb"),
        ("EntityDb", "EntityDb"),
        ("EntityTree", "EntityTree"),
        ("::LogMsg>::deserialize", "LogMsg"),
        ("::TimePoint>::deserialize", "TimePoint"),
        ("gltf", "gltf"),
        ("image::image", "image"),
        // -----
        // Very general:
        ("crossbeam::channel::Sender", "crossbeam::channel::Sender"),
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
