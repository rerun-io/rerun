use re_byte_size::NamedMemUsageTree;
use re_entity_db::LogSource;
use re_renderer::WgpuResourcePoolStatistics;
use re_ui::{HasDesignTokens as _, UiExt as _, WindowFrameConfig};
use re_viewer_context::{ActiveStoreContext, StorageContext, store_hub::StoreHubStats};

use crate::{
    app_blueprint::AppBlueprint, app_state::WelcomeScreenState, background_tasks::BackgroundTasks,
};

use super::App;

impl App {
    /// Top-level ui function.
    ///
    /// Shows the viewer ui.
    pub(super) fn ui_impl(
        &mut self,
        ui: &mut egui::Ui,
        frame: &eframe::Frame,
        app_blueprint: &AppBlueprint<'_>,
        gpu_resource_stats: &WgpuResourcePoolStatistics,
        store_context: Option<&ActiveStoreContext<'_>>,
        storage_context: &StorageContext<'_>,
        mem_usage_tree: Option<NamedMemUsageTree>,
        store_stats: Option<&StoreHubStats>,
    ) {
        let custom_window_decorations = self.custom_window_decorations();
        #[cfg(any(target_os = "windows", target_os = "linux"))]
        {
            let id = egui::Id::new("custom_window_decorations_applied");
            let was_applied = ui.ctx().data_mut(|data| {
                let was_applied = data.get_temp::<bool>(id);
                data.insert_temp(id, custom_window_decorations);
                was_applied
            });

            if let Some(was_applied) = was_applied
                && was_applied != custom_window_decorations
            {
                ui.send_viewport_cmd(egui::ViewportCommand::Decorations(
                    !custom_window_decorations,
                ));
                ui.send_viewport_cmd(egui::ViewportCommand::Transparent(
                    custom_window_decorations,
                ));
            }

            // Apply windows undecorated shadow both on change and the first frame.
            #[cfg(target_os = "windows")]
            if was_applied != Some(custom_window_decorations)
                && let Some(window) = frame.winit_window()
            {
                use winit::platform::windows::WindowExtWindows as _;
                window.set_undecorated_shadow(custom_window_decorations);
            }
        }

        let mut main_panel_frame = egui::Frame::default();
        if self.custom_window_frame() {
            // Add some margin so that we can later paint an outline around it all.
            main_panel_frame.inner_margin = 1.0.into();
        }

        egui::CentralPanel::default()
            .frame(main_panel_frame)
            .show_inside(ui, |ui| {
                paint_background_fill(ui);

                crate::ui::mobile_warning_ui(ui, custom_window_decorations);

                if self.custom_window_frame() {
                    // The outer frame owns the rounded window background. Inner panels should not
                    // repaint opaque square corners over it.
                    ui.visuals_mut().panel_fill = egui::Color32::TRANSPARENT;
                }

                crate::ui::top_panel(
                    frame,
                    self,
                    app_blueprint,
                    store_context,
                    storage_context.hub,
                    gpu_resource_stats,
                    ui,
                );

                self.dev_panel_ui(
                    ui,
                    gpu_resource_stats,
                    mem_usage_tree,
                    store_stats,
                    storage_context,
                );

                self.egui_debug_panel_ui(ui);

                let egui_renderer = &mut frame
                    .wgpu_render_state()
                    .expect("Failed to get frame render state")
                    .renderer
                    .write();

                if let Some(render_ctx) = egui_renderer
                    .callback_resources
                    .get_mut::<re_renderer::RenderContext>()
                {
                    render_ctx.begin_frame(); // This may actually be called multiple times per egui frame, if we have a multi-pass layout frame.

                    // In some (rare) circumstances we run two egui passes in a single frame.
                    // This happens on call to `egui::Context::request_discard`.
                    let is_start_of_new_frame = ui.current_pass_index() == 0;

                    if is_start_of_new_frame {
                        self.state.redap_servers.on_frame_start(
                            &self.connection_registry,
                            &self.async_runtime,
                            &self.egui_ctx,
                            self.startup_options.login_enabled(),
                            &self.command_sender,
                        );
                    }

                    self.texture_readback.poll_and_save_texture_readbacks(
                        render_ctx,
                        ui,
                        &self.command_sender,
                        &mut self.notifications,
                    );

                    // TODO(RR-3033): `AppState::show` still expects a non-optional `ActiveStoreContext`; fall back to a sentinel empty context for no-store routes.
                    let empty_store_context = ActiveStoreContext::empty();
                    let active_store_context = store_context.unwrap_or(&empty_store_context);

                    self.state.show(
                        &self.app_env,
                        &self.startup_options,
                        app_blueprint,
                        ui,
                        render_ctx,
                        active_store_context,
                        storage_context,
                        &self.reflection,
                        &self.component_ui_registry,
                        &self.component_fallback_registry,
                        &self.view_class_registry,
                        &self.rx_log,
                        &self.command_sender,
                        &WelcomeScreenState {
                            hide_examples: self.startup_options.hide_welcome_screen,
                            opacity: self.welcome_screen_opacity(ui),
                        },
                        self.event_dispatcher.as_ref(),
                        &self.connection_registry,
                        &self.async_runtime,
                        self.custom_window_frame(),
                    );
                    render_ctx.before_submit();

                    self.show_text_logs_as_notifications();
                }
            });

        if custom_window_decorations {
            custom_windows_decorations_resize_ui(ui);
        }

        if self.app_options().show_notification_toasts {
            self.notifications.show_toasts(ui);
        }
    }

    fn dev_panel_ui(
        &mut self,
        ui: &mut egui::Ui,
        gpu_resource_stats: &WgpuResourcePoolStatistics,
        mem_usage_tree: Option<NamedMemUsageTree>,
        store_stats: Option<&StoreHubStats>,
        storage_context: &re_viewer_context::StorageContext<'_>,
    ) {
        let window_frame = self.window_frame_config(ui.ctx());
        let frame = egui::Frame {
            fill: ui.visuals().panel_fill,
            ..ui.tokens().bottom_panel_frame(window_frame)
        };

        let external_trees = if self.dev_panel_open {
            self.external_memory_users.captured_trees()
        } else {
            &[]
        };

        egui::Panel::bottom("dev_panel")
            .default_size(300.0)
            .resizable(true)
            .frame(frame)
            .show_animated_inside(ui, self.dev_panel_open, |ui| {
                self.dev_panel.ui(
                    ui,
                    &self.state.app_options().memory_limit,
                    mem_usage_tree,
                    external_trees,
                    gpu_resource_stats,
                    store_stats,
                    storage_context,
                );
            });
    }

    fn egui_debug_panel_ui(&self, ui: &mut egui::Ui) {
        let egui_ctx = ui.ctx().clone();

        egui::Panel::left("style_panel")
            .default_size(300.0)
            .resizable(true)
            .frame(
                ui.tokens()
                    .top_panel_frame(self.window_frame_config(ui.ctx())),
            )
            .show_animated_inside(ui, self.egui_debug_panel_open, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if ui
                        .button("request_discard")
                        .on_hover_text("Request a second layout pass. Just for testing.")
                        .clicked()
                    {
                        ui.request_discard("testing");
                    }

                    egui::CollapsingHeader::new("egui settings")
                        .default_open(false)
                        .show(ui, |ui| {
                            egui_ctx.settings_ui(ui);
                        });

                    egui::CollapsingHeader::new("egui inspection")
                        .default_open(false)
                        .show(ui, |ui| {
                            egui_ctx.inspection_ui(ui);
                        });
                });
            });
    }

    fn should_fade_in_welcome_screen(&self) -> bool {
        if let Some(expect_data_soon) = self.startup_options.expect_data_soon {
            return expect_data_soon;
        }

        // The reason for the fade-in is to avoid the welcome screen
        // flickering quickly before receiving some data.
        // So: if we expect data very soon, we do a fade-in.

        for source in self.rx_log.sources() {
            match &*source {
                LogSource::File { .. }
                | LogSource::HttpStream { .. }
                | LogSource::RedapGrpcStream { .. }
                | LogSource::Stdin
                | LogSource::RrdWebEvent
                | LogSource::Sdk
                | LogSource::JsChannel { .. } => {
                    return true; // We expect data soon, so fade-in
                }

                // We start a gRPC server by default in native rerun, i.e. when just running `rerun`,
                // and in that case fading in the welcome screen would be slightly annoying.
                // However, we also use the gRPC server for sending data from the logging SDKs
                // when they call `spawn()`, and in that case we really want to fade in the welcome screen.
                // Therefore `spawn()` uses the special `--expect-data-soon` flag
                // (handled earlier in this function), so here we know we are in the other case:
                // a user calling `rerun` in their terminal (don't fade in).
                LogSource::MessageProxy { .. } => {}
            }
        }

        false // No special sources (or no sources at all), so don't fade in
    }

    /// Handle fading in the welcome screen, if we should.
    fn welcome_screen_opacity(&self, egui_ctx: &egui::Context) -> f32 {
        if self.should_fade_in_welcome_screen() {
            // The reason for this delay is to avoid the welcome screen
            // flickering quickly before receiving some data.
            // The only time it has for that is between the call to `spawn` and sending the recording info,
            // which should happen _right away_, so we only need a small delay.
            // Why not skip the wlecome screen completely when we expect the data?
            // Because maybe the data never comes.
            let sec_since_first_shown = self.start_time.elapsed().as_secs_f32();
            let opacity = egui::remap_clamp(sec_since_first_shown, 0.4..=0.6, 0.0..=1.0);
            if opacity < 1.0 {
                egui_ctx.request_repaint();
            }
            opacity
        } else {
            1.0
        }
    }

    /// Show recent text log messages to the user as toast notifications.
    fn show_text_logs_as_notifications(&mut self) {
        re_tracing::profile_function!();

        while let Ok(message) = self.text_log_rx.try_recv() {
            self.notifications.add_log(message);
        }
    }

    #[cfg(any(target_os = "windows", target_os = "linux"))]
    pub(crate) fn custom_window_decorations(&self) -> bool {
        self.app_options().custom_window_decorations
            && !self.is_screenshotting()
            && !self.app_env().is_test()
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    pub(crate) fn custom_window_decorations(&self) -> bool {
        let _ = self;
        false
    }

    pub(crate) fn window_frame_config(&self, ctx: &egui::Context) -> WindowFrameConfig {
        if self.custom_window_frame() {
            WindowFrameConfig::custom(ctx)
        } else {
            WindowFrameConfig::Native
        }
    }
}

/// Add invisible resize handles for the compact title bar.
///
/// Disabling native decorations removes the OS-provided resize borders together
/// with the native title bar. This restores that interaction by placing thin
/// egui hit zones along the edges/corners and forwarding drag starts to winit
/// via [`egui::ViewportCommand::BeginResize`]. The actual resizing is still
/// performed by the windowing system.
#[cfg(not(target_arch = "wasm32"))]
fn custom_windows_decorations_resize_ui(ui: &egui::Ui) {
    let fullscreen = ui.ctx().input(|i| i.viewport().fullscreen).unwrap_or(false);
    let maximized = ui.ctx().input(|i| i.viewport().maximized).unwrap_or(false);

    if fullscreen || maximized {
        return;
    }

    let rect = ui.max_rect();
    let resize_margin = 6.0;
    let corner_size = 16.0;

    // Corners get larger square hit zones so diagonal resizing is easy to grab.
    // Edges get thin strips between the corners.
    let resize_regions = [
        (
            egui::Rect::from_min_max(
                rect.left_top(),
                rect.left_top() + egui::vec2(corner_size, corner_size),
            ),
            egui::ResizeDirection::NorthWest,
            egui::CursorIcon::ResizeNorthWest,
            "compact_title_bar_resize_nw",
        ),
        (
            egui::Rect::from_min_max(
                rect.right_top() - egui::vec2(corner_size, 0.0),
                rect.right_top() + egui::vec2(0.0, corner_size),
            ),
            egui::ResizeDirection::NorthEast,
            egui::CursorIcon::ResizeNorthEast,
            "compact_title_bar_resize_ne",
        ),
        (
            egui::Rect::from_min_max(
                rect.left_bottom() - egui::vec2(0.0, corner_size),
                rect.left_bottom() + egui::vec2(corner_size, 0.0),
            ),
            egui::ResizeDirection::SouthWest,
            egui::CursorIcon::ResizeSouthWest,
            "compact_title_bar_resize_sw",
        ),
        (
            egui::Rect::from_min_max(
                rect.right_bottom() - egui::vec2(corner_size, corner_size),
                rect.right_bottom(),
            ),
            egui::ResizeDirection::SouthEast,
            egui::CursorIcon::ResizeSouthEast,
            "compact_title_bar_resize_se",
        ),
        (
            egui::Rect::from_min_max(
                rect.left_top() + egui::vec2(corner_size, 0.0),
                rect.right_top() + egui::vec2(-corner_size, resize_margin),
            ),
            egui::ResizeDirection::North,
            egui::CursorIcon::ResizeNorth,
            "compact_title_bar_resize_n",
        ),
        (
            egui::Rect::from_min_max(
                rect.left_bottom() + egui::vec2(corner_size, -resize_margin),
                rect.right_bottom() + egui::vec2(-corner_size, 0.0),
            ),
            egui::ResizeDirection::South,
            egui::CursorIcon::ResizeSouth,
            "compact_title_bar_resize_s",
        ),
        (
            egui::Rect::from_min_max(
                rect.left_top() + egui::vec2(0.0, corner_size),
                rect.left_bottom() + egui::vec2(resize_margin, -corner_size),
            ),
            egui::ResizeDirection::West,
            egui::CursorIcon::ResizeWest,
            "compact_title_bar_resize_w",
        ),
        (
            egui::Rect::from_min_max(
                rect.right_top() + egui::vec2(-resize_margin, corner_size),
                rect.right_bottom() + egui::vec2(0.0, -corner_size),
            ),
            egui::ResizeDirection::East,
            egui::CursorIcon::ResizeEast,
            "compact_title_bar_resize_e",
        ),
    ];

    for (rect, direction, cursor_icon, id) in resize_regions {
        let response = ui.interact(rect, ui.id().with(id), egui::Sense::click_and_drag());
        if response.hovered() || response.dragged() {
            ui.ctx().set_cursor_icon(cursor_icon);
        }
        if response.drag_started_by(egui::PointerButton::Primary) {
            ui.send_viewport_cmd(egui::ViewportCommand::BeginResize(direction));
        }
    }
}

fn paint_background_fill(ui: &egui::Ui) {
    // This is required because the streams view (time panel)
    // has rounded top corners, which leaves a gap.
    // So we fill in that gap (and other) here.
    // Of course this does some over-draw, but we have to live with that.

    let tokens = ui.tokens();
    let is_maximized = ui.ctx().input(|i| i.viewport().maximized == Some(true));

    ui.painter().rect_filled(
        ui.max_rect().expand(0.5),
        tokens.native_window_corner_radius(is_maximized),
        tokens.panel_bg_color,
    );
}

#[cfg(target_arch = "wasm32")]
fn custom_windows_decorations_resize_ui(_ui: &egui::Ui) {}

pub(super) fn paint_custom_window_frame(egui_ctx: &egui::Context) {
    let tokens = egui_ctx.tokens();

    let painter = egui::Painter::new(
        egui_ctx.clone(),
        egui::LayerId::new(egui::Order::TOP, egui::Id::new("native_window_frame")),
        egui::Rect::EVERYTHING,
    );

    let is_maximized = egui_ctx.input(|i| i.viewport().maximized == Some(true));
    let corner_radius = tokens.native_window_corner_radius(is_maximized);

    painter.rect_stroke(
        egui_ctx.content_rect(),
        corner_radius,
        egui_ctx.tokens().native_frame_stroke,
        egui::StrokeKind::Inside,
    );
}

pub(super) fn preview_files_being_dropped(egui_ctx: &egui::Context) {
    use egui::{Align2, Id, LayerId, Order, TextStyle};

    // Preview hovering files:
    if !egui_ctx.input(|i| i.raw.hovered_files.is_empty()) {
        use std::fmt::Write as _;

        let mut text = "Drop to load:\n".to_owned();
        egui_ctx.input(|input| {
            for file in &input.raw.hovered_files {
                if let Some(path) = &file.path {
                    write!(text, "\n{}", path.display()).ok();
                } else if !file.mime.is_empty() {
                    write!(text, "\n{}", file.mime).ok();
                }
            }
        });

        let painter =
            egui_ctx.layer_painter(LayerId::new(Order::Foreground, Id::new("file_drop_target")));

        let screen_rect = egui_ctx.content_rect();
        painter.rect_filled(
            screen_rect,
            0.0,
            egui_ctx
                .global_style()
                .visuals
                .extreme_bg_color
                .gamma_multiply_u8(192),
        );
        painter.text(
            screen_rect.center(),
            Align2::CENTER_CENTER,
            text,
            TextStyle::Body.resolve(&egui_ctx.global_style()),
            egui_ctx.global_style().visuals.strong_text_color(),
        );
    }
}

// ----------------------------------------------------------------------------

pub(super) fn file_saver_progress_ui(
    egui_ctx: &egui::Context,
    background_tasks: &mut BackgroundTasks,
) {
    if background_tasks.is_file_save_in_progress() {
        // There's already a file save running in the background.

        if let Some(res) = background_tasks.poll_file_saver_promise() {
            // File save promise has returned.
            match res {
                Ok(path) => {
                    re_log::info!("File saved to {path:?}."); // this will also show a notification the user
                }
                Err(err) => {
                    re_log::error!("{err}"); // this will also show a notification the user
                }
            }
        } else {
            // File save promise is still running in the background.

            // NOTE: not a toast, want something a bit more discreet here.
            egui::Window::new("file_saver_spin")
                .anchor(egui::Align2::RIGHT_BOTTOM, egui::Vec2::ZERO)
                .title_bar(false)
                .enabled(false)
                .auto_sized()
                .show(egui_ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.loading_indicator("Writing file to disk");
                        ui.label("Writing file to disk…");
                    })
                });
        }
    }
}
