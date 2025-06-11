use egui::NumExt as _;
use itertools::Itertools as _;

use re_format::format_uint;
use re_renderer::WgpuResourcePoolStatistics;
use re_smart_channel::{ReceiveSet, SmartChannelSource};
use re_ui::{ContextExt as _, UICommand, UiExt as _};
use re_viewer_context::StoreContext;

use crate::{App, app_blueprint::AppBlueprint};

pub fn top_panel(
    frame: &eframe::Frame,
    app: &mut App,
    app_blueprint: &AppBlueprint<'_>,
    store_context: Option<&StoreContext<'_>>,
    gpu_resource_stats: &WgpuResourcePoolStatistics,
    ui: &mut egui::Ui,
) {
    re_tracing::profile_function!();

    let style_like_web = app.is_screenshotting();
    let top_bar_style = ui.ctx().top_bar_style(style_like_web);
    let top_panel_frame = ui.tokens().top_panel_frame();

    let mut content = |ui: &mut egui::Ui, show_content: bool| {
        // React to dragging and double-clicking the top bar:
        #[cfg(not(target_arch = "wasm32"))]
        if !re_ui::NATIVE_WINDOW_BAR {
            // Interact with background first, so that buttons in the top bar gets input priority
            // (last added widget has priority for input).
            let title_bar_response = ui.interact(
                ui.max_rect(),
                ui.id().with("background"),
                egui::Sense::click(),
            );
            if title_bar_response.double_clicked() {
                let maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
            } else if title_bar_response.is_pointer_button_down_on() {
                // TODO(emilk): This should probably only run on `title_bar_response.drag_started_by(PointerButton::Primary)`,
                // see https://github.com/emilk/egui/pull/4656
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
        }

        egui::containers::menu::Bar::new().ui(ui, |ui| {
            ui.set_height(top_bar_style.height);
            ui.add_space(top_bar_style.indent);

            if show_content {
                top_bar_ui(
                    frame,
                    app,
                    app_blueprint,
                    store_context,
                    ui,
                    gpu_resource_stats,
                );
            }
        });
    };

    let panel = egui::TopBottomPanel::top("top_bar")
        .frame(top_panel_frame)
        .exact_height(top_bar_style.height);
    let is_expanded = app_blueprint.top_panel_state().is_expanded();

    // On MacOS, we show the close/minimize/maximize buttons in the top panel.
    // We _always_ want to show the top panel in that case, and only hide its content.
    if !re_ui::NATIVE_WINDOW_BAR {
        panel.show_inside(ui, |ui| content(ui, is_expanded));
    } else {
        panel.show_animated_inside(ui, is_expanded, |ui| content(ui, is_expanded));
    }
}

fn top_bar_ui(
    frame: &eframe::Frame,
    app: &mut App,
    app_blueprint: &AppBlueprint<'_>,
    store_context: Option<&StoreContext<'_>>,
    ui: &mut egui::Ui,
    gpu_resource_stats: &WgpuResourcePoolStatistics,
) {
    app.rerun_menu_button_ui(frame.wgpu_render_state(), store_context, ui);

    ui.add_space(12.0);
    website_link_ui(ui);

    if !app.is_screenshotting() {
        if app.app_options().show_metrics {
            ui.separator();
            frame_time_label_ui(ui, app);
            memory_use_label_ui(ui, gpu_resource_stats);
        }

        let latency_snapshot = store_context
            .map(|store_context| store_context.recording.ingestion_stats().latency_snapshot());

        if let Some(latency_snapshot) = latency_snapshot {
            // Should we show the e2e latency?

            // High enough to be consering; low enough to be believable (and almost realtime).
            let is_latency_interesting = latency_snapshot
                .e2e
                .is_some_and(|e2e| app.app_options().warn_e2e_latency < e2e && e2e < 60.0);

            // Avoid flicker by showing the latency for 1 seconds ince it was last deemned interesting:

            if is_latency_interesting {
                app.latest_latency_interest = web_time::Instant::now();
            }

            if app.latest_latency_interest.elapsed().as_secs_f32() < 1.0 {
                ui.separator();
                latency_snapshot_button_ui(ui, latency_snapshot);
            }
        }
    }

    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if re_ui::CUSTOM_WINDOW_DECORATIONS && !cfg!(target_arch = "wasm32") {
            ui.add_space(8.0);
            #[cfg(not(target_arch = "wasm32"))]
            ui.native_window_buttons_ui();
            ui.separator();
        } else {
            // Make the first button the same distance form the side as from the top,
            // no matter how high the top bar is.
            let extra_margin = (ui.available_height() - 24.0) / 2.0;
            ui.add_space(extra_margin);
        }

        panel_buttons_r2l(app, app_blueprint, ui);

        if !app.is_screenshotting() {
            connection_status_ui(ui, app.msg_receive_set());
        }

        if let Some(wgpu) = frame.wgpu_render_state() {
            let info = wgpu.adapter.get_info();
            if info.device_type == wgpu::DeviceType::Cpu {
                // TODO(#4304): replace with a panel showing recent log messages
                ui.hyperlink_to(
                    egui::RichText::new("⚠ Software rasterizer ⚠")
                        .small()
                        .color(ui.visuals().warn_fg_color),
                    "https://www.rerun.io/docs/getting-started/troubleshooting#graphics-issues",
                )
                .on_hover_ui(|ui| {
                    ui.label("Software rasterizer detected - expect poor performance.");
                    ui.label(
                        "Rerun requires hardware accelerated graphics (i.e. a GPU) for good performance.",
                    );
                    ui.label("Click for troubleshooting.");
                    ui.add_space(8.0);
                    ui.label(format!(
                        "wgpu adapter {}",
                        re_renderer::adapter_info_summary(&info)
                    ));
                });
            }
        }

        // Warn if in debug build
        if cfg!(debug_assertions) && !app.is_screenshotting() {
            ui.vertical_centered(|ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                ui.add_space(6.0); // TODO(emilk): in egui, add a proper way of centering a single widget in a UI.
                egui::warn_if_debug_build(ui);
            });
        }
    });
}

fn connection_status_ui(ui: &mut egui::Ui, rx: &ReceiveSet<re_log_types::LogMsg>) {
    let sources = rx
        .sources()
        .into_iter()
        .filter(|source| {
            match source.as_ref() {
                SmartChannelSource::File(_)
                | SmartChannelSource::RrdHttpStream { .. }
                | SmartChannelSource::RedapGrpcStream { .. }
                | SmartChannelSource::Stdin => {
                    false // These show up in the recordings panel as a "Loading…" in `recordings_panel.rs`
                }

                SmartChannelSource::RrdWebEventListener
                | SmartChannelSource::Sdk
                | SmartChannelSource::MessageProxy { .. }
                | SmartChannelSource::JsChannel { .. } => true,
            }
        })
        .collect_vec();

    match sources.len() {
        0 => return,
        1 => {
            source_label(ui, sources[0].as_ref());
        }
        n => {
            // In practice we never get here
            ui.label(format!("{n} sources connected"))
                .on_hover_ui(|ui| {
                    ui.vertical(|ui| {
                        for source in &sources {
                            source_label(ui, source.as_ref());
                        }
                    });
                });
        }
    }

    fn source_label(ui: &mut egui::Ui, source: &SmartChannelSource) -> egui::Response {
        let response = ui.label(status_string(source));

        let tooltip = match source {
            SmartChannelSource::File(_)
            | SmartChannelSource::Stdin
            | SmartChannelSource::RrdHttpStream { .. }
            | SmartChannelSource::RedapGrpcStream { .. }
            | SmartChannelSource::RrdWebEventListener
            | SmartChannelSource::JsChannel { .. }
            | SmartChannelSource::Sdk => None,

            SmartChannelSource::MessageProxy { .. } => {
                Some("Waiting for an SDK to connect".to_owned())
            }
        };

        if let Some(tooltip) = tooltip {
            response.on_hover_text(tooltip)
        } else {
            response
        }
    }

    fn status_string(source: &SmartChannelSource) -> String {
        match source {
            re_smart_channel::SmartChannelSource::File(path) => {
                format!("Loading {}…", path.display())
            }
            re_smart_channel::SmartChannelSource::Stdin => "Loading stdin…".to_owned(),
            re_smart_channel::SmartChannelSource::RrdHttpStream { url, .. } => {
                format!("Waiting for data on {url}…")
            }
            re_smart_channel::SmartChannelSource::MessageProxy(uri) => {
                format!("Waiting for data on {uri}…")
            }
            re_smart_channel::SmartChannelSource::RedapGrpcStream { uri, .. } => {
                format!(
                    "Waiting for data on {}…",
                    uri.clone().without_query_and_fragment()
                )
            }
            re_smart_channel::SmartChannelSource::RrdWebEventListener
            | re_smart_channel::SmartChannelSource::JsChannel { .. } => {
                "Waiting for logging data…".to_owned()
            }
            re_smart_channel::SmartChannelSource::Sdk => {
                "Waiting for logging data from SDK".to_owned()
            }
        }
    }
}

/// Lay out the panel button right-to-left
fn panel_buttons_r2l(app: &mut App, app_blueprint: &AppBlueprint<'_>, ui: &mut egui::Ui) {
    #[cfg(target_arch = "wasm32")]
    if app.is_fullscreen_allowed() {
        let icon = if app.is_fullscreen_mode() {
            &re_ui::icons::MINIMIZE
        } else {
            &re_ui::icons::MAXIMIZE
        };

        if ui
            .medium_icon_toggle_button(icon, &mut true)
            .on_hover_text("Toggle fullscreen")
            .clicked()
        {
            app.toggle_fullscreen();
        }
    }

    // selection panel
    if !app_blueprint.selection_panel_overridden()
        && ui
            .medium_icon_toggle_button(
                &re_ui::icons::RIGHT_PANEL_TOGGLE,
                &mut app_blueprint.selection_panel_state().is_expanded(),
            )
            .on_hover_ui(|ui| UICommand::ToggleSelectionPanel.tooltip_ui(ui))
            .clicked()
    {
        app_blueprint.toggle_selection_panel(&app.command_sender);
    }

    // time panel
    if !app_blueprint.time_panel_overridden()
        && ui
            .medium_icon_toggle_button(
                &re_ui::icons::BOTTOM_PANEL_TOGGLE,
                &mut app_blueprint.time_panel_state().is_expanded(),
            )
            .on_hover_ui(|ui| UICommand::ToggleTimePanel.tooltip_ui(ui))
            .clicked()
    {
        app_blueprint.toggle_time_panel(&app.command_sender);
    }

    // blueprint panel
    if !app_blueprint.blueprint_panel_overridden()
        && ui
            .medium_icon_toggle_button(
                &re_ui::icons::LEFT_PANEL_TOGGLE,
                &mut app_blueprint.blueprint_panel_state().is_expanded(),
            )
            .on_hover_ui(|ui| UICommand::ToggleBlueprintPanel.tooltip_ui(ui))
            .clicked()
    {
        app_blueprint.toggle_blueprint_panel(&app.command_sender);
    }

    re_ui::notifications::notification_toggle_button(ui, &mut app.notifications);
}

/// Shows clickable website link as an image (text doesn't look as nice)
fn website_link_ui(ui: &mut egui::Ui) {
    let desired_height = ui.max_rect().height();
    let desired_height = desired_height.at_most(20.0);

    let image = re_ui::icons::RERUN_IO_TEXT
        .as_image()
        .fit_to_original_size(2.0) // hack, because the original SVG is very small
        .max_height(desired_height)
        .tint(ui.tokens().strong_fg_color);

    let url = "https://rerun.io/";
    let response = ui
        .add(egui::ImageButton::new(image))
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    if response.clicked() {
        ui.ctx().open_url(egui::output::OpenUrl {
            url: url.to_owned(),
            new_tab: true,
        });
    }
}

fn frame_time_label_ui(ui: &mut egui::Ui, app: &App) {
    if let Some(frame_time) = app.frame_time_history.average() {
        let ms = frame_time * 1e3;

        let visuals = ui.visuals();
        let color = if ms < 15.0 {
            visuals.weak_text_color()
        } else {
            visuals.warn_fg_color
        };

        // we use monospace so the width doesn't fluctuate as the numbers change.
        let text = format!("{ms:.1} ms");
        ui.label(egui::RichText::new(text).monospace().color(color))
            .on_hover_text("CPU time used by Rerun Viewer each frame. Lower is better.");
    }
}

fn memory_use_label_ui(ui: &mut egui::Ui, gpu_resource_stats: &WgpuResourcePoolStatistics) {
    const CODE: &str = "use re_memory::AccountingAllocator;\n\
                        #[global_allocator]\n\
                        static GLOBAL: AccountingAllocator<std::alloc::System> =\n    \
                            AccountingAllocator::new(std::alloc::System);";

    fn click_to_copy(
        ui: &mut egui::Ui,
        text: impl Into<String>,
        add_contents_on_hover: impl FnOnce(&mut egui::Ui),
    ) {
        #[allow(clippy::blocks_in_conditions)]
        let text = text.into();
        if ui
            .add(
                egui::Label::new(
                    egui::RichText::new(text)
                        .monospace()
                        .color(ui.visuals().weak_text_color()),
                )
                .sense(egui::Sense::click()),
            )
            .on_hover_ui(|ui| add_contents_on_hover(ui))
            .clicked()
        {
            ui.ctx().copy_text(CODE.to_owned());
        }
    }

    let mem = re_memory::MemoryUse::capture();

    if let Some(count) = re_memory::accounting_allocator::global_allocs() {
        // we use monospace so the width doesn't fluctuate as the numbers change.

        let bytes_used_text = re_format::format_bytes(count.size as _);
        ui.label(
            egui::RichText::new(&bytes_used_text)
                .monospace()
                .color(ui.visuals().weak_text_color()),
        )
        .on_hover_text(format!(
            "Rerun Viewer is using {} of RAM in {} separate allocations,\n\
            plus {} of GPU memory in {} textures and {} buffers.",
            bytes_used_text,
            format_uint(count.count),
            re_format::format_bytes(gpu_resource_stats.total_bytes() as _),
            format_uint(gpu_resource_stats.num_textures),
            format_uint(gpu_resource_stats.num_buffers),
        ));
    } else if let Some(rss) = mem.resident {
        let bytes_used_text = re_format::format_bytes(rss as _);
        click_to_copy(ui, &bytes_used_text, |ui| {
            ui.label(format!(
                "Rerun Viewer is using {} of Resident memory (RSS),\n\
                plus {} of GPU memory in {} textures and {} buffers.",
                bytes_used_text,
                re_format::format_bytes(gpu_resource_stats.total_bytes() as _),
                format_uint(gpu_resource_stats.num_textures),
                format_uint(gpu_resource_stats.num_buffers),
            ));
            ui.label(
                "To get more accurate memory reportings, consider configuring your Rerun \n\
                 viewer to use an AccountingAllocator by adding the following to your \n\
                 code's main entrypoint:",
            );
            ui.code(CODE);
            ui.label("(click to copy to clipboard)");
        });
    } else {
        click_to_copy(ui, "N/A MiB", |ui| {
            ui.label(
                "The Rerun viewer was not configured to run with an AccountingAllocator,\n\
                consider adding the following to your code's main entrypoint:",
            );
            ui.code(CODE);
            ui.label("(click to copy to clipboard)");
        });
    }
}

/// Shows the e2e latency.
fn latency_snapshot_button_ui(
    ui: &mut egui::Ui,
    latency: re_entity_db::LatencySnapshot,
) -> Option<egui::Response> {
    let Some(e2e) = latency.e2e else {
        return None; // No e2e latency, nothing to show as a summary
    };

    // Unit: seconds
    if 60.0 < e2e {
        return None; // Probably an old recording and not live data.
    }

    let text = format!("Latency: {}", latency_text(e2e));
    let response = ui.weak(text);

    let response = response.on_hover_ui(|ui| {
        latency_details_ui(ui, latency);
    });

    Some(response)
}

fn latency_details_ui(ui: &mut egui::Ui, latency: re_entity_db::LatencySnapshot) {
    // The user is interested in the latency, so keep it updated.
    ui.ctx().request_repaint();

    let e2e_hover_text = "End-to-end latency from when the data was logged by the SDK to when it is shown in the viewer.\n\
    This includes time for encoding, network latency, and decoding.\n\
    It is also affected by the framerate of the viewer.\n\
    This latency is inaccurate if the logging was done on a different machine, since it is clock-based.";

    // Note: all times are in seconds.
    let re_entity_db::LatencySnapshot {
        e2e,
        log2chunk,
        chunk2encode,
        transmission,
        decode2ingest,
    } = latency;

    egui::Grid::new("latency_snapshot")
        .num_columns(2)
        .striped(false)
        .show(ui, |ui| {
            if let Some(e2e) = e2e {
                ui.strong("log -> ingest (total end-to-end)")
                    .on_hover_text(e2e_hover_text);
                ui.monospace(latency_text(e2e));
                ui.end_row();

                ui.end_row(); // Intentional extra blank line
            }

            if let Some(log2chunk) = log2chunk {
                ui.label("log -> chunk");
                ui.monospace(latency_text(log2chunk));
                ui.end_row();
            }

            if let Some(chunk2encode) = chunk2encode {
                ui.label("chunk -> encode");
                ui.monospace(latency_text(chunk2encode));
                ui.end_row();
            }
            if let Some(transmission) = transmission {
                ui.label("encode -> decode (transmission)");
                ui.monospace(latency_text(transmission));
                ui.end_row();
            }
            if let Some(decode2ingest) = decode2ingest {
                ui.label("decode -> ingest");
                ui.monospace(latency_text(decode2ingest));
                ui.end_row();
            }
        });
}

fn latency_text(latency_sec: f32) -> String {
    if latency_sec < 0.001 {
        format!("{:.0} µs", 1e6 * latency_sec)
    } else if latency_sec < 1.0 {
        format!("{:.0} ms", 1e3 * latency_sec)
    } else {
        format!("{latency_sec:.1} s")
    }
}
