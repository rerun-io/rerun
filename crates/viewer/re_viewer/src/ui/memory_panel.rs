use re_chunk_store::{ChunkStoreChunkStats, ChunkStoreConfig, ChunkStoreStats};
use re_format::{format_bytes, format_uint};
use re_memory::{MemoryHistory, MemoryLimit, MemoryUse, util::sec_since_start};
use re_query::{CacheStats, CachesStats};
use re_renderer::WgpuResourcePoolStatistics;
use re_ui::UiExt as _;
use re_viewer_context::store_hub::StoreHubStats;

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
        store_stats: Option<&StoreHubStats>,
    ) {
        re_tracing::profile_function!();
        self.history.capture(
            Some(
                (gpu_resource_stats.total_buffer_size_in_bytes
                    + gpu_resource_stats.total_texture_size_in_bytes) as _,
            ),
            store_stats.map(|stats| {
                (stats.recording_stats2.static_chunks.total_size_bytes
                    + stats.recording_stats2.temporal_chunks.total_size_bytes) as _
            }),
            store_stats.map(|stats| stats.recording_cached_stats.total_size_bytes() as _),
            store_stats.map(|stats| {
                (stats.blueprint_stats.static_chunks.total_size_bytes
                    + stats.blueprint_stats.temporal_chunks.total_size_bytes) as _
            }),
        );
    }

    /// Note that we purged memory at this time, to show in stats.
    #[inline]
    pub fn note_memory_purge(&mut self) {
        self.memory_purge_times.push(sec_since_start());
    }

    #[allow(clippy::too_many_arguments)]
    pub fn ui(
        &self,
        ui: &mut egui::Ui,
        limit: &MemoryLimit,
        gpu_resource_stats: &WgpuResourcePoolStatistics,
        store_stats: Option<&StoreHubStats>,
    ) {
        re_tracing::profile_function!();

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
        store_stats: Option<&StoreHubStats>,
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

        if let Some(store_stats) = store_stats {
            ui.separator();
            ui.collapsing("Datastore Resources", |ui| {
                Self::store_stats2(
                    ui,
                    &store_stats.recording_config2,
                    &store_stats.recording_stats2,
                );
            });

            ui.separator();
            ui.collapsing("Primary Cache Resources", |ui| {
                Self::caches_stats(ui, &store_stats.recording_cached_stats);
            });

            ui.separator();
            ui.collapsing("Blueprint Resources", |ui| {
                Self::store_stats2(
                    ui,
                    &store_stats.blueprint_config,
                    &store_stats.blueprint_stats,
                );
            });
        }
    }

    fn cpu_stats(ui: &mut egui::Ui, limit: &MemoryLimit) {
        if let Some(max_bytes) = limit.max_bytes {
            ui.label(format!("Memory limit: {}", format_bytes(max_bytes as _)));
        } else {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.label("You can set an upper limit of RAM use with the command-line option ");
                ui.code("--memory-limit");
            });
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
                    .on_hover_text("Live bytes, counted by our own allocator");
            } else if cfg!(debug_assertions) {
                ui.label("Memory-tracking allocator not installed.");
            }
        }

        let mut is_tracking_callstacks = re_memory::accounting_allocator::is_tracking_callstacks();
        ui.re_checkbox(&mut is_tracking_callstacks, "Detailed allocation tracking")
            .on_hover_text("This will slow down the program");
        re_memory::accounting_allocator::set_tracking_callstacks(is_tracking_callstacks);

        if let Some(tracking_stats) = re_memory::accounting_allocator::tracking_stats() {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            Self::tracking_stats(ui, tracking_stats);
        } else if !cfg!(target_arch = "wasm32") {
            ui.label(format!(
                "Set {RERUN_TRACK_ALLOCATIONS}=1 for detailed allocation tracking from startup."
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

    fn store_stats2(
        ui: &mut egui::Ui,
        store_config: &ChunkStoreConfig,
        store_stats: &ChunkStoreStats,
    ) {
        // TODO(cmc): this will become useful again once we introduce compaction settings.
        _ = store_config;

        egui::Grid::new("store stats grid 2")
            .num_columns(3)
            .show(ui, |ui| {
                let ChunkStoreStats {
                    static_chunks,
                    temporal_chunks,
                } = *store_stats;

                ui.label(egui::RichText::new("Stats").italics());
                ui.label("Chunks");
                ui.label("Rows (total)");
                ui.label("Events (total)")
                    .on_hover_text("Number of non-null component batches (cells)");
                ui.label("Size (total)");
                ui.end_row();

                fn label_chunk_stats(ui: &mut egui::Ui, stats: ChunkStoreChunkStats) {
                    let ChunkStoreChunkStats {
                        num_chunks,
                        total_size_bytes,
                        num_rows,
                        num_events,
                    } = stats;

                    ui.label(re_format::format_uint(num_chunks));
                    ui.label(re_format::format_uint(num_rows));
                    ui.label(re_format::format_uint(num_events));
                    ui.label(re_format::format_bytes(total_size_bytes as _));
                }

                ui.label("Static:");
                label_chunk_stats(ui, static_chunks);
                ui.end_row();

                ui.label("Temporal:");
                label_chunk_stats(ui, temporal_chunks);
                ui.end_row();

                ui.label("Total:");
                label_chunk_stats(ui, static_chunks + temporal_chunks);
                ui.end_row();
            });
    }

    fn caches_stats(ui: &mut egui::Ui, caches_stats: &CachesStats) {
        let CachesStats { latest_at, range } = caches_stats;

        if !latest_at.is_empty() {
            ui.separator();
            ui.strong("LatestAt");
            egui::ScrollArea::vertical()
                .max_height(200.0)
                .id_salt("latest_at")
                .show(ui, |ui| {
                    egui::Grid::new("latest_at cache stats grid")
                        .num_columns(3)
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new("Entity").underline());
                            ui.label(egui::RichText::new("Component").underline());
                            ui.label(egui::RichText::new("Chunks").underline())
                                .on_hover_text("How many chunks in the cache?");
                            ui.label(egui::RichText::new("Effective size").underline())
                                .on_hover_text("What would be the size of this cache in the worst case, i.e. if all chunks had been fully copied?");
                            ui.label(egui::RichText::new("Actual size").underline())
                                .on_hover_text("What is the actual size of this cache after deduplication?");
                            ui.end_row();

                            for (cache_key, stats) in latest_at {
                                let &CacheStats {
                                    total_chunks,
                                    total_effective_size_bytes,
                                    total_actual_size_bytes,
                                } = stats;

                                ui.label(cache_key.entity_path.to_string());
                                ui.label(cache_key.component_descr.to_string());
                                ui.label(re_format::format_uint(total_chunks));
                                ui.label(re_format::format_bytes(total_effective_size_bytes as _));
                                ui.label(re_format::format_bytes(total_actual_size_bytes as _));
                                ui.end_row();
                            }
                        });
                });
        }

        if !range.is_empty() {
            ui.separator();
            ui.strong("Range");
            egui::ScrollArea::vertical()
                .max_height(200.0)
                .id_salt("range")
                .show(ui, |ui| {
                    egui::Grid::new("range cache stats grid")
                        .num_columns(4)
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new("Entity").underline());
                            ui.label(egui::RichText::new("Component").underline());
                            ui.label(egui::RichText::new("Chunks").underline())
                                .on_hover_text("How many chunks in the cache?");
                            ui.label(egui::RichText::new("Effective size").underline())
                                .on_hover_text("What would be the size of this cache in the worst case, i.e. if all chunks had been fully copied?");
                            ui.label(egui::RichText::new("Actual size").underline())
                                .on_hover_text("What is the actual size of this cache after deduplication?");
                            ui.end_row();

                            for (cache_key, stats) in range {
                                let &CacheStats {
                                    total_chunks,
                                    total_effective_size_bytes,
                                    total_actual_size_bytes,
                                } = stats;

                                ui.label(cache_key.entity_path.to_string());
                                ui.label(cache_key.component_descr.to_string());
                                ui.label(re_format::format_uint(total_chunks));
                                ui.label(re_format::format_bytes(total_effective_size_bytes as _));
                                ui.label(re_format::format_bytes(total_actual_size_bytes as _));
                                ui.end_row();
                            }
                        });
                });
        }
    }

    fn tracking_stats(
        ui: &mut egui::Ui,
        tracking_stats: re_memory::accounting_allocator::TrackingStatistics,
    ) {
        ui.label("counted = fully_tracked + stochastically_tracked + untracked + overhead");
        ui.label(format!(
            "fully_tracked: {} in {} allocs",
            format_bytes(tracking_stats.fully_tracked.size as _),
            format_uint(tracking_stats.fully_tracked.count),
        ));
        ui.label(format!(
            "stochastically_tracked: {} in {} allocs",
            format_bytes(tracking_stats.stochastically_tracked.size as _),
            format_uint(tracking_stats.stochastically_tracked.count),
        ));
        ui.label(format!(
            "untracked: {} in {} allocs (all smaller than {})",
            format_bytes(tracking_stats.untracked.size as _),
            format_uint(tracking_stats.untracked.count),
            format_bytes(tracking_stats.track_size_threshold as _),
        ));
        ui.label(format!(
            "overhead: {} in {} allocs",
            format_bytes(tracking_stats.overhead.size as _),
            format_uint(tracking_stats.overhead.count),
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

                            let text = format!(
                                "{}{} in {} allocs (≈{} / alloc){} - {}",
                                if is_stochastic { "≈" } else { "" },
                                format_bytes((callstack.extant.size * stochastic_rate) as _),
                                format_uint(callstack.extant.count * stochastic_rate),
                                format_bytes(
                                    callstack.extant.size as f64 / callstack.extant.count as f64
                                ),
                                if stochastic_rate <= 1 {
                                    String::new()
                                } else {
                                    format!(" ({} stochastic samples)", callstack.extant.count)
                                },
                                summarize_callstack(&callstack.readable_backtrace.to_string())
                            );

                            if ui
                                .button(text)
                                .on_hover_text("Click to copy callstack to clipboard")
                                .clicked()
                            {
                                let mut text = callstack.readable_backtrace.to_string();
                                if text.is_empty() {
                                    // This is weird
                                    text = "No callstack available".to_owned();
                                }
                                ui.ctx().copy_text(text);
                            }
                        }
                    });
            });
    }

    fn plot(&self, ui: &mut egui::Ui, limit: &MemoryLimit) {
        re_tracing::profile_function!();

        use itertools::Itertools as _;

        fn to_line<'a>(name: &str, history: &egui::util::History<i64>) -> egui_plot::Line<'a> {
            egui_plot::Line::new(
                name,
                history
                    .iter()
                    .map(|(time, bytes)| [time, bytes as f64])
                    .collect_vec(),
            )
        }

        egui_plot::Plot::new("mem_history_plot")
            .min_size(egui::Vec2::splat(200.0))
            .label_formatter(|name, value| format!("{name}: {}", format_bytes(value.y)))
            .x_axis_formatter(|time, _| format!("{} s", time.value))
            .y_axis_formatter(|bytes, _| format_bytes(bytes.value))
            .show_x(false)
            .legend(egui_plot::Legend::default().position(egui_plot::Corner::LeftTop))
            .include_y(0.0)
            // TODO(emilk): turn off plot interaction, and always do auto-sizing
            .show(ui, |plot_ui| {
                if let Some(max_bytes) = limit.max_bytes {
                    plot_ui.hline(
                        egui_plot::HLine::new("Limit (counted)", max_bytes as f64).width(2.0),
                    );
                }

                for &time in &self.memory_purge_times {
                    plot_ui.vline(
                        egui_plot::VLine::new("RAM purge", time)
                            .color(egui::Color32::from_rgb(252, 161, 3))
                            .width(2.0),
                    );
                }

                let MemoryHistory {
                    resident,
                    counted,
                    counted_gpu,
                    counted_store,
                    counted_primary_caches,
                    counted_blueprint,
                } = &self.history;

                plot_ui.line(to_line("Resident", resident).width(1.5));
                plot_ui.line(to_line("Counted", counted).width(1.5));
                plot_ui.line(to_line("Counted GPU", counted_gpu).width(1.5));
                plot_ui.line(to_line("Counted store 2", counted_store).width(1.5));
                plot_ui.line(to_line("Counted primary caches", counted_primary_caches).width(1.5));
                plot_ui.line(to_line("Counted blueprint", counted_blueprint).width(1.5));
            });
    }
}

fn summarize_callstack(callstack: &str) -> String {
    let patterns = [
        ("App::receive_messages", "App::receive_messages"),
        ("w_store::store::ComponentBucket>::archive", "archive"),
        ("ChunkStore>::insert", "ChunkStore"),
        ("EntityDb", "EntityDb"),
        ("EntityDb", "EntityDb"),
        ("EntityTree", "EntityTree"),
        ("::LogMsg>::deserialize", "LogMsg"),
        ("::TimePoint>::deserialize", "TimePoint"),
        ("ImageCache", "ImageCache"),
        ("gltf", "gltf"),
        ("image::image", "image"),
        ("epaint::text::text_layout", "text_layout"),
        ("egui_wgpu", "egui_wgpu"),
        ("wgpu_hal", "wgpu_hal"),
        ("prepare_staging_buffer", "prepare_staging_buffer"),
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
