use egui::{Atom, Button, Color32, Id, Image, NumExt as _, Popup, RichText, Sense, include_image};
use emath::{Rect, RectAlign, Vec2};
use re_format::format_uint;
use re_renderer::WgpuResourcePoolStatistics;
use re_ui::{ContextExt as _, UICommand, UiExt as _, icons};
use re_viewer_context::{StoreContext, StoreHub, SystemCommand, SystemCommandSender as _};

use crate::App;
use crate::app_blueprint::AppBlueprint;
use crate::latency_tracker::{LatencyResult, ServerLatencyTrackers};

pub fn top_panel(
    frame: &eframe::Frame,
    app: &mut App,
    app_blueprint: &AppBlueprint<'_>,
    store_context: Option<&StoreContext<'_>>,
    store_hub: &StoreHub,
    gpu_resource_stats: &WgpuResourcePoolStatistics,
    ui: &mut egui::Ui,
) {
    re_tracing::profile_function!();

    let style_like_web = app.is_screenshotting() || app.app_env().is_test();
    let top_bar_style = ui.ctx().top_bar_style(frame, style_like_web);
    let top_panel_frame = ui.tokens().top_panel_frame();

    let mut content = |ui: &mut egui::Ui, show_content: bool| {
        // React to dragging and double-clicking the top bar:
        #[cfg(not(target_arch = "wasm32"))]
        if !re_ui::native_window_bar(ui.ctx().os()) {
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

        egui::MenuBar::new().ui(ui, |ui| {
            ui.set_height(top_bar_style.height);
            ui.add_space(top_bar_style.indent);

            if show_content {
                top_bar_ui(
                    frame,
                    app,
                    app_blueprint,
                    store_context,
                    store_hub,
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
    if !re_ui::native_window_bar(ui.ctx().os()) {
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
    store_hub: &StoreHub,
    ui: &mut egui::Ui,
    gpu_resource_stats: &WgpuResourcePoolStatistics,
) {
    app.rerun_menu_button_ui(frame.wgpu_render_state(), store_context, ui);

    ui.add_space(12.0);
    website_link_ui(ui);

    if !app.startup_options().web_history_enabled() {
        ui.add_space(12.0);
        app.navigation_buttons(ui);
    }

    if !app.is_screenshotting() && !app.app_env().is_test() {
        show_warnings(frame, ui, app.app_env()); // Fixed width: put first

        let latency_snapshot = store_context
            .map(|store_context| store_context.recording.ingestion_stats().latency_snapshot());

        if app.app_options().show_metrics {
            ui.separator();

            ui.scope(|ui| {
                ui.spacing_mut().item_spacing.x = 12.0;

                // Varying widths:
                memory_use_label_ui(ui, gpu_resource_stats);
                frame_time_label_ui(ui, app);
                fps_ui(ui, app);

                if let Some(latency_snapshot) = latency_snapshot {
                    // Always show latency when metrics are enabled:
                    latency_snapshot_button_ui(ui, latency_snapshot);
                }
            });
        } else {
            // Show latency metrics only if high enough to be "interesting":
            if let Some(latency_snapshot) = latency_snapshot {
                // Should we show the e2e latency?

                // High enough to be concerning; low enough to be believable (and almost realtime).
                let is_latency_interesting = latency_snapshot
                    .e2e
                    .is_some_and(|e2e| app.app_options().warn_e2e_latency < e2e && e2e < 60.0);

                // Avoid flicker by showing the latency for 1 second since it was last deemed interesting:
                if is_latency_interesting {
                    app.latest_latency_interest = Some(web_time::Instant::now());
                }

                if app
                    .latest_latency_interest
                    .is_some_and(|instant| instant.elapsed().as_secs_f32() < 1.0)
                {
                    ui.separator();
                    latency_snapshot_button_ui(ui, latency_snapshot);
                }
            }
        }

        if cfg!(debug_assertions) && !app.app_env().is_test() {
            multi_pass_warning_dot_ui(ui);
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

        panel_buttons_r2l(app, app_blueprint, ui, store_hub);

        if !app.is_screenshotting() && !app.app_env().is_test() {
            connection_status_ui(
                ui,
                &mut app.server_latency_trackers,
                app.state.navigation.current(),
                store_hub,
            );
        }
    });
}

fn show_warnings(frame: &eframe::Frame, ui: &mut egui::Ui, app_env: &crate::AppEnvironment) {
    // We could log these as warning instead and relying on the notification panel to show it.
    // However, there are a few benefits of instead showing it like this:
    // * it's more visible
    // * it will be captured in screenshots in bug reports etc
    // * it let's us customize the message a bit more, with links etc.

    // We want to add a separator if there is any warning. This works.
    let mut has_shown_warning = false;

    fn show_warning(
        ui: &mut egui::Ui,
        has_shown_warning: &mut bool,
        callback: impl FnOnce(&mut egui::Ui),
    ) {
        if !*has_shown_warning {
            ui.separator();
            *has_shown_warning = true;
        }

        callback(ui);
    }

    if cfg!(debug_assertions) {
        show_warning(ui, &mut has_shown_warning, |ui| {
            // Warn if in debug build
            ui.label(
                egui::RichText::new("⚠ Debug build")
                    .small()
                    .color(ui.visuals().warn_fg_color),
            )
            .on_hover_text("Rerun was compiled with debug assertions enabled.");
        });
    }

    if !app_env.is_test()
        && let Some(wgpu) = frame.wgpu_render_state()
        && let info = wgpu.adapter.get_info()
        && info.device_type == wgpu::DeviceType::Cpu
    {
        show_warning(ui, &mut has_shown_warning, |ui| {
            software_rasterizer_warning_ui(ui, &info);
        });
    }

    if crate::docker_detection::is_docker() {
        show_warning(ui, &mut has_shown_warning, |ui| {
            let text = egui::RichText::new("⚠ Docker")
                .small()
                .color(ui.visuals().warn_fg_color);
            let url = "https://github.com/rerun-io/rerun/issues/6835";
            ui.hyperlink_to(text,url).on_hover_ui(|ui| {
                ui.label("It looks like the Rerun Viewer is running inside a Docker container. This is not officially supported, and may lead to subtle bugs. ");
                ui.label("Click for more info.");
            });
        });
    }
}

fn software_rasterizer_warning_ui(ui: &mut egui::Ui, info: &wgpu::AdapterInfo) {
    ui.hyperlink_to(
        egui::RichText::new("⚠ Software rasterizer")
            .small()
            .color(ui.visuals().warn_fg_color),
        "https://www.rerun.io/docs/getting-started/troubleshooting#graphics-issues",
    )
    .on_hover_ui(|ui| {
        ui.label("Software rasterizer detected - expect poor performance.");
        ui.label("Rerun requires hardware accelerated graphics (i.e. a GPU) for good performance.");
        ui.label("Click for troubleshooting.");
        ui.add_space(8.0);
        ui.label(format!(
            "wgpu adapter {}",
            re_renderer::adapter_info_summary(info)
        ));
    });
}

/// Show an orange dot to warn about multi-pass layout in egui.
///
/// If it is shown, it means something called `egui::Context::request_discard` the previous pass,
/// causing a multi-pass layout frame in egui.
/// This is used to cover up some visual glitches, but it is also
/// a bit costly and we shouldn't do it too often.
///
/// An infrequent blinking of the dot (e.g. when opening a new panel) is expected,
/// but it should not be sustained.
fn multi_pass_warning_dot_ui(ui: &mut egui::Ui) {
    let is_multi_pass = 0 < ui.ctx().current_pass_index();

    // Showing the dot just one frame is not enough (e.g. easily missed at 120Hz),
    // so we blink it up and then fade it out quickly.

    let now = ui.ctx().input(|i| i.time);
    let last_multipass_time = ui.data_mut(|data| {
        let last_multipass_time = data
            .get_temp_mut_or_insert_with(egui::Id::new("last_multipass_time"), || {
                f64::NEG_INFINITY
            });
        if is_multi_pass {
            *last_multipass_time = now;
        }
        *last_multipass_time
    });
    let time_since_last_multipass = (now - last_multipass_time) as f32;

    let intensity = egui::remap_clamp(time_since_last_multipass, 0.0..=0.5, 1.0..=0.0);

    let radius = 5.0 * egui::emath::ease_in_ease_out(intensity);

    let (response, painter) = ui.allocate_painter(egui::Vec2::splat(12.0), egui::Sense::hover());

    if intensity <= 0.0 {
        // Nothing to show, but we still allocate space so we can show a tooltip to developers
        // who wondered what the hell that blinking orange dot was
    } else {
        // Paint dot:
        painter.circle_filled(response.rect.center(), radius, egui::Color32::ORANGE);

        // Make sure we ask for a repaint so we can animate the dot fading out:
        ui.ctx().request_repaint();
    }

    response.on_hover_text(
        "A blinking orange dot appears here in debug builds whenever request_discard is called.\n\
        It is expect that the dot appears occasionally, e.g. when showing a new panel for the first time.\n\
        However, it should not be sustained, as that would indicate a performance bug.",
    );
}

fn connection_status_ui(
    ui: &mut egui::Ui,
    latency_trackers: &mut ServerLatencyTrackers,
    display_mode: &re_viewer_context::DisplayMode,
    store_hub: &StoreHub,
) {
    if let Some(origin) = display_mode.redap_origin(store_hub)
        && origin != *re_redap_browser::EXAMPLES_ORIGIN
    {
        let latency = latency_trackers.origin_latency(&origin);

        let url = origin.format_host();
        match latency {
            LatencyResult::ToBeAssigned => {}
            LatencyResult::NoConnection => {
                ui.label(format!("no connection to {url}"));
            }
            LatencyResult::MostRecent(duration) => {
                let mut layout_job = egui::text::LayoutJob::default();

                let ms = duration.as_millis();

                RichText::new(format!("{ms} ms")).strong().append_to(
                    &mut layout_job,
                    ui.style(),
                    egui::FontSelection::Default,
                    egui::Align::Center,
                );

                RichText::new(format!(" latency for {url}")).append_to(
                    &mut layout_job,
                    ui.style(),
                    egui::FontSelection::Default,
                    egui::Align::Center,
                );

                ui.label(layout_job);
            }
        }
    }
}

/// Lay out the panel button right-to-left
fn panel_buttons_r2l(
    app: &mut App,
    app_blueprint: &AppBlueprint<'_>,
    ui: &mut egui::Ui,
    store_hub: &StoreHub,
) {
    let display_mode = app.state.navigation.current();

    #[cfg(target_arch = "wasm32")]
    if app.is_fullscreen_allowed() {
        let (icon, label) = if app.is_fullscreen_mode() {
            (&re_ui::icons::MINIMIZE, "Minimize")
        } else {
            (&re_ui::icons::MAXIMIZE, "Maximize")
        };

        if ui
            .medium_icon_toggle_button(icon, label, &mut true)
            .on_hover_text("Toggle fullscreen")
            .clicked()
        {
            app.toggle_fullscreen();
        }
    }

    // selection panel
    ui.add_enabled_ui(
        display_mode.has_selection_panel() && !app_blueprint.selection_panel_overridden(),
        |ui| {
            if ui
                .medium_icon_toggle_button(
                    &re_ui::icons::RIGHT_PANEL_TOGGLE,
                    "Selection panel toggle",
                    &mut app_blueprint.selection_panel_state().is_expanded(),
                )
                .on_hover_ui(|ui| UICommand::ToggleSelectionPanel.tooltip_ui(ui))
                .clicked()
            {
                app_blueprint.toggle_selection_panel(&app.command_sender);
            }
        },
    );

    // time panel
    ui.add_enabled_ui(
        display_mode.has_time_panel() && !app_blueprint.time_panel_overridden(),
        |ui| {
            if ui
                .medium_icon_toggle_button(
                    &re_ui::icons::BOTTOM_PANEL_TOGGLE,
                    "Time panel toggle",
                    &mut app_blueprint.time_panel_state().is_expanded(),
                )
                .on_hover_ui(|ui| UICommand::ToggleTimePanel.tooltip_ui(ui))
                .clicked()
            {
                app_blueprint.toggle_time_panel(&app.command_sender);
            }
        },
    );

    // blueprint panel
    ui.add_enabled_ui(
        display_mode.has_blueprint_panel() && !app_blueprint.blueprint_panel_overridden(),
        |ui| {
            if ui
                .medium_icon_toggle_button(
                    &re_ui::icons::LEFT_PANEL_TOGGLE,
                    "Blueprint panel toggle",
                    &mut app_blueprint.blueprint_panel_state().is_expanded(),
                )
                .on_hover_ui(|ui| UICommand::ToggleBlueprintPanel.tooltip_ui(ui))
                .clicked()
            {
                app_blueprint.toggle_blueprint_panel(&app.command_sender);
            }
        },
    );

    app.notifications.notification_toggle_button(ui);

    let selection = app.state.selection_state.selected_items();
    let rec_cfg = store_hub
        .active_store_id()
        .and_then(|id| app.state.time_controls.get(id));
    app.state.share_modal.button_ui(
        ui,
        store_hub,
        app.state.navigation.current(),
        rec_cfg,
        selection,
    );

    if let Some(auth) = &app.state.auth_state
        && !app.is_screenshotting()
        && !app.app_env().is_test()
    {
        let rect_id = Id::new("user_icon_rect");
        let user_icon_size = 16.0;
        let response = Button::new((
            Atom::custom(rect_id, Vec2::splat(user_icon_size)),
            icons::DROPDOWN_ARROW
                .as_image()
                .tint(ui.visuals().text_color()),
        ))
        .atom_ui(ui);
        if let Some(rect) = response.rect(rect_id) {
            user_icon(&auth.email, rect, ui, user_icon_size / 2.0, 220);
        }

        Popup::menu(&response.response)
            .align(RectAlign::BOTTOM_END)
            .gap(3.0)
            .show(|ui| {
                ui.horizontal(|ui| {
                    let user_icon_size = 32.0;
                    let (rect, _) =
                        ui.allocate_exact_size(Vec2::splat(user_icon_size), Sense::hover());
                    user_icon(&auth.email, rect, ui, 8.0, 255);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        ui.label(RichText::new(&auth.email).color(ui.tokens().text_default));
                        if ui
                            .link(RichText::new("Log out").color(ui.tokens().text_subdued))
                            .clicked()
                        {
                            app.command_sender.send_system(SystemCommand::Logout);
                        }
                    })
                });
            });
    }
}

fn user_icon(email: &str, rect: Rect, ui: &egui::Ui, corner_radius: f32, tint: u8) {
    // The color should not change based on theme, so it's fine to hard-code here
    #[expect(clippy::disallowed_methods)]
    let text_color = Color32::from_gray(tint);
    Image::new(include_image!("../../data/user_image.jpg"))
        .corner_radius(corner_radius)
        .tint(text_color)
        .paint_at(ui, rect);
    let initial = email.chars().next().unwrap_or('?').to_ascii_uppercase();
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        initial,
        egui::FontId::proportional(rect.height() * 0.6),
        Color32::WHITE,
    );
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
        .add(egui::Button::image(image))
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

fn fps_ui(ui: &mut egui::Ui, app: &App) {
    if let Some(fps) = app.frame_time_history.rate() {
        let visuals = ui.visuals();

        // We only warn if we _suspect_ that we're in "continuous repaint mode".
        let low_fps_right_now = fps < 20.0 && ui.ctx().has_requested_repaint();

        let now = ui.ctx().input(|i| i.time);
        let warn_start_id = ui.id().with("fps_warning");
        let warn_start_time = ui.data_mut(|d| {
            if low_fps_right_now {
                *d.get_persisted_mut_or::<f64>(warn_start_id, now)
            } else {
                d.remove::<f64>(warn_start_id);
                now
            }
        });

        // Avoid blinking warning
        let low_fps_for_some_time = 0.5 < (now - warn_start_time);

        let color = if low_fps_for_some_time {
            visuals.warn_fg_color
        } else {
            visuals.weak_text_color()
        };

        // we use monospace so the width doesn't fluctuate as the numbers change.
        let text = format!("{fps:.0} FPS");
        ui.label(egui::RichText::new(text).monospace().color(color))
            .on_hover_text("Frames per second. Higher is better.");
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

    let text = format!("Latency: {}", latency_text(ui.visuals(), e2e).text());
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

    if let (Some(log2chunk), Some(chunk2encode), Some(transmission), Some(decode2ingest)) =
        (log2chunk, chunk2encode, transmission, decode2ingest)
    {
        // We have a full picture - use a nice vertical layout:

        if let Some(e2e) = e2e {
            ui.horizontal(|ui| {
                ui.label("end-to-end:").on_hover_text(e2e_hover_text);
                latency_label(ui, e2e);
            });
            ui.separator();
        }

        ui.vertical_centered(|ui| {
            fn small_and_weak(text: &str) -> egui::RichText {
                egui::RichText::new(text).small().weak()
            }

            ui.spacing_mut().item_spacing.y = 0.0;
            ui.label("log call");
            ui.label(small_and_weak("|"));
            latency_label(ui, log2chunk);
            ui.label(small_and_weak("↓"));
            ui.label("batch creation");
            ui.label(small_and_weak("|"));
            latency_label(ui, chunk2encode);
            ui.label(small_and_weak("↓"));
            ui.label("encode and transmit");
            ui.label(small_and_weak("|"));
            latency_label(ui, transmission);
            ui.label(small_and_weak("↓"));
            ui.label("receive and decode");
            ui.label(small_and_weak("|"));
            latency_label(ui, decode2ingest);
            ui.label(small_and_weak("↓"));
            ui.label("ingest into viewer");
        });
    } else {
        // We have a partial picture - show only what we got:
        egui::Grid::new("latency_snapshot")
            .num_columns(2)
            .striped(false)
            .show(ui, |ui| {
                if let Some(e2e) = e2e {
                    ui.strong("log -> ingest (total end-to-end)")
                        .on_hover_text(e2e_hover_text);
                    latency_label(ui, e2e);
                    ui.end_row();

                    ui.end_row(); // Intentional extra blank line
                }

                if let Some(log2chunk) = log2chunk {
                    ui.label("log -> chunk");
                    latency_label(ui, log2chunk);
                    ui.end_row();
                }
                if let Some(chunk2encode) = chunk2encode {
                    ui.label("chunk -> encode");
                    latency_label(ui, chunk2encode);
                    ui.end_row();
                }
                if let Some(transmission) = transmission {
                    ui.label("encode -> decode (transmission)");
                    latency_label(ui, transmission);
                    ui.end_row();
                }
                if let Some(decode2ingest) = decode2ingest {
                    ui.label("decode -> ingest");
                    latency_label(ui, decode2ingest);
                    ui.end_row();
                }
            });
    }
}

fn latency_label(ui: &mut egui::Ui, latency_sec: f32) -> egui::Response {
    ui.label(latency_text(ui.visuals(), latency_sec))
}

fn latency_text(visuals: &egui::Visuals, latency_sec: f32) -> egui::RichText {
    if latency_sec < 0.001 {
        egui::RichText::new(format!("{:.0} µs", 1e6 * latency_sec))
    } else if latency_sec < 1.0 {
        egui::RichText::new(format!("{:.0} ms", 1e3 * latency_sec))
    } else {
        egui::RichText::new(format!("{latency_sec:.1} s")).color(visuals.warn_fg_color)
    }
}
