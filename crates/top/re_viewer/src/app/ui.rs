use re_agent_panel::ShownContent;
use re_byte_size::NamedMemUsageTree;
use re_log_channel::{LogSource, RecordingOpenBehavior};
use re_renderer::WgpuResourcePoolStatistics;
use re_ui::{HasDesignTokens as _, UiExt as _, WindowFrameConfig};
use re_viewer_context::{
    ActiveStoreContext, EntryKind, Route, StorageContext, store_hub::StoreHubStats,
};

use crate::ui::WelcomeScreenState;
use crate::{app_blueprint::AppBlueprint, background_tasks::BackgroundTasks};

use super::{AppCore, WindowDecorationsRequest};

impl AppCore {
    /// Top-level ui function.
    ///
    /// Shows the viewer ui.
    /// Returns where the central panel of the current route was shown, if it has one.
    pub(super) fn ui_impl(
        &mut self,
        ui: &mut egui::Ui,
        frame: &eframe::Frame,
        app_blueprint: &AppBlueprint<'_>,
        gpu_resource_stats: &WgpuResourcePoolStatistics,
        active_store_context: Option<&ActiveStoreContext<'_>>,
        storage_context: &StorageContext<'_>,
        mem_usage_tree: Option<NamedMemUsageTree>,
        store_stats: Option<&StoreHubStats>,
    ) -> Option<egui::Rect> {
        // Before the panels: the highlight is dismissed by any click, and the welcome screen
        // clears `InputState::pointer` while it paints, so a read afterwards would miss that
        // click. The outline is painted on its own layer, so it still ends up on top.
        if let Some(highlight) = &self.viewer_control.screen_highlight
            && !highlight.show(ui)
        {
            self.viewer_control.screen_highlight = None;
        }

        let custom_window_decorations = self.custom_window_decorations();

        let mut main_panel_frame = egui::Frame::default();
        if self.custom_window_frame() {
            // Add some margin so that we can later paint an outline around it all.
            main_panel_frame.inner_margin = 1.0.into();
        }

        let central_panel_rect = egui::CentralPanel::default()
            .frame(main_panel_frame)
            .show(ui, |ui| {
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
                    active_store_context,
                    storage_context.hub,
                    gpu_resource_stats,
                    ui,
                );

                self.agent_panel_ui(ui, active_store_context);

                self.dev_panel_ui(
                    ui,
                    gpu_resource_stats,
                    mem_usage_tree,
                    store_stats,
                    active_store_context,
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
                        if let Some(origin) = self.connections.registry.internal_origin() {
                            self.state.redap_servers.add_internal_server(
                                origin.clone(),
                                &self.connections.registry,
                                &self.async_runtime,
                                &self.egui_ctx,
                            );
                            if self.app_options().use_viewer_catalog {
                                self.state.redap_servers.reveal_internal_catalog();
                            }
                        }

                        self.state.redap_servers.on_frame_start(
                            &self.connections.registry,
                            &self.async_runtime,
                            &self.egui_ctx,
                            self.startup.options.login_enabled(),
                            &self.command_sender,
                        );

                        // After `on_frame_start`, so a catalog answer that arrived this frame is
                        // taken into account right away.
                        self.navigation.resolve_route(&self.state.redap_servers);

                        // Install our url decorator so links render nicely. This is done every frame
                        // so the data stays up-to-date.
                        let url_name_lookup =
                            std::sync::Arc::new(self.state.redap_servers.build_url_lookup());
                        re_ui::UrlDecorator::set(
                            &self.egui_ctx,
                            re_viewer_context::make_url_decorator(
                                url_name_lookup,
                                self.egui_ctx.theme(),
                            ),
                        );
                    }

                    self.capture
                        .texture_readback
                        .poll_and_save_texture_readbacks(
                            render_ctx,
                            ui,
                            &self.command_sender,
                            &mut self.notifications,
                        );

                    let welcome_screen_revealed = self.update_welcome_screen_revealed(ui);
                    let welcome_screen_state = WelcomeScreenState {
                        hide_examples: self.startup.options.hide_welcome_screen
                            || !self.app_options().enable_welcome_screen,
                        revealed: welcome_screen_revealed,
                    };
                    let central_panel_rect = self.route_ui(
                        ui,
                        app_blueprint,
                        render_ctx,
                        active_store_context,
                        storage_context,
                        &welcome_screen_state,
                    );
                    render_ctx.before_submit();

                    self.show_text_logs_as_notifications();

                    central_panel_rect
                } else {
                    None
                }
            })
            .inner;

        if custom_window_decorations {
            custom_windows_decorations_resize_ui(ui);
        }

        if self.app_options().show_notification_toasts {
            self.notifications.show_toasts(ui);
        }

        central_panel_rect
    }

    fn dev_panel_ui(
        &mut self,
        ui: &mut egui::Ui,
        gpu_resource_stats: &WgpuResourcePoolStatistics,
        mem_usage_tree: Option<NamedMemUsageTree>,
        store_stats: Option<&StoreHubStats>,
        store_context: Option<&ActiveStoreContext<'_>>,
        storage_context: &re_viewer_context::StorageContext<'_>,
    ) {
        let window_frame = self.window_frame_config(ui.ctx());
        let frame = egui::Frame {
            fill: ui.visuals().panel_fill,
            ..ui.tokens().bottom_panel_frame(window_frame)
        };

        let external_trees = if self.dev_panel.open {
            self.memory.external_memory_users.captured_trees()
        } else {
            &[]
        };

        let mut dev_panel_open = self.dev_panel.open;
        let mut close_requested = false;
        egui::Panel::bottom("dev_panel")
            .default_size(300.0)
            .resizable(true)
            .drag_to_open(false)
            .frame(frame)
            .show_collapsible(ui, &mut dev_panel_open, |ui| {
                let response = self.dev_panel.ui(
                    ui,
                    &self.state.app_options().memory_limit,
                    mem_usage_tree,
                    external_trees,
                    gpu_resource_stats,
                    store_stats,
                    store_context,
                    &self.stores.time_controls,
                    storage_context,
                );
                close_requested = response.close_requested;
                if response.repaint_requested {
                    ui.request_repaint();
                }
            });
        // `show_collapsible` flips `dev_panel_open` when the user drags the panel closed:
        self.dev_panel.open = dev_panel_open && !close_requested;
    }

    /// The rightmost panel: a chat with a coding agent that drives this viewer.
    fn agent_panel_ui(
        &mut self,
        ui: &mut egui::Ui,
        active_store_context: Option<&ActiveStoreContext<'_>>,
    ) {
        let viewer_endpoint = self
            .connections
            .registry
            .internal_origin()
            .map(|origin| origin.as_url());
        let cache_dir = cfg_select! {
            target_arch = "wasm32" => None::<std::path::PathBuf>,
            _ => self.app_options().cache_directory.clone(),
        };
        let is_recording_open = active_store_context
            .is_some_and(|context| !context.recording.store_id().is_empty_recording());
        let shown_content = match self.navigation.routes.current() {
            Route::RedapEntry {
                kind: Some(EntryKind::Dataset(_)),
                ..
            } => ShownContent::Dataset,
            Route::LocalTable(_)
            | Route::RedapEntry {
                kind: Some(EntryKind::Table),
                ..
            } => ShownContent::Table,
            _ if is_recording_open => ShownContent::Recording,
            _ => ShownContent::Other,
        };

        let Self {
            agent_panel,
            state,
            startup,
            async_runtime,
            ..
        } = self;

        agent_panel.show(
            ui,
            &mut state.agent_panel_open,
            &mut state.agent_settings,
            async_runtime,
            viewer_endpoint.as_deref(),
            cache_dir.as_deref(),
            startup.build_info.is_in_rerun_workspace,
            shown_content,
        );
        state.agent_panel_ever_opened |= state.agent_panel_open;
    }

    fn egui_debug_panel_ui(&mut self, ui: &mut egui::Ui) {
        let egui_ctx = ui.ctx().clone();

        let mut egui_debug_panel_open = self.egui_debug_panel_open;
        egui::Panel::left("style_panel")
            .default_size(300.0)
            .resizable(true)
            .drag_to_open(false)
            .frame(
                ui.tokens()
                    .top_panel_frame(self.window_frame_config(ui.ctx())),
            )
            .show_collapsible(ui, &mut egui_debug_panel_open, |ui| {
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
        // `show_collapsible` flips `egui_debug_panel_open` when the user drags the panel closed:
        self.egui_debug_panel_open = egui_debug_panel_open;
    }

    /// Whether the welcome screen may be drawn this frame.
    ///
    /// When data is expected right after startup, the welcome screen is held back briefly so it
    /// doesn't flash before the recording arrives. It is not skipped entirely, because the data
    /// may never come. Once revealed it stays revealed: sources that connect later must never
    /// blank a welcome screen the user can already see.
    fn update_welcome_screen_revealed(&mut self, egui_ctx: &egui::Context) -> bool {
        // Enough for `spawn()` to send its recording info, which it does right after connecting.
        const HOLD_BACK: std::time::Duration = std::time::Duration::from_millis(400);

        if !self.welcome_screen_revealed {
            let elapsed = self.startup.start_time.elapsed();
            if HOLD_BACK <= elapsed
                || !expects_data_soon(
                    self.startup.options.expect_data_soon,
                    &self.rx_log.sources(),
                )
            {
                self.welcome_screen_revealed = true;
            } else {
                egui_ctx.request_repaint_after(HOLD_BACK.saturating_sub(elapsed));
            }
        }

        self.welcome_screen_revealed
    }

    /// Show recent text log messages to the user as toast notifications.
    fn show_text_logs_as_notifications(&mut self) {
        re_tracing::profile_function!();

        while let Ok(message) = self.text_log_rx.try_recv() {
            self.viewer_control.log.push(&message);
            self.notifications.add_log(message);
        }
    }

    /// Whether we — rather than the OS — draw the window decorations this frame.
    ///
    /// Everything that paints must use this, and never
    /// [`re_viewer_context::AppOptions::custom_window_decorations`]: it is the state the
    /// window is actually in, so the whole frame agrees even when the user flips the
    /// setting mid-frame.
    pub(crate) fn custom_window_decorations(&self) -> bool {
        let custom = match self.window_decorations_request {
            WindowDecorationsRequest::NotSent => re_ui::custom_window_decorations_default(),
            WindowDecorationsRequest::Native => false,
            WindowDecorationsRequest::Custom => true,
        };

        custom
            // A screenshot should show the app, not the frame around it. We leave the
            // window itself alone: the capture is of our surface, not of the OS chrome.
            && !self.is_screenshotting()
            && !self.app_env().is_test()
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
        let response = ui.interact(
            rect,
            ui.make_persistent_id(id),
            egui::Sense::click_and_drag(),
        );
        if response.hovered() || response.dragged() {
            ui.ctx().set_cursor_icon(cursor_icon);
        }
        if response.drag_started_by(egui::PointerButton::Primary) {
            ui.send_viewport_cmd(egui::ViewportCommand::BeginResize(direction));
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn custom_windows_decorations_resize_ui(_ui: &egui::Ui) {}

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

pub(super) fn paint_custom_window_frame(egui_ctx: &egui::Context) {
    let tokens = egui_ctx.tokens();

    let painter = egui::Painter::new(
        egui_ctx.clone(),
        egui::LayerId::new(egui::Order::TOP, egui::Id::unique("native_window_frame")),
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

/// Whether a dropped file is an image that the agent prompt can take.
pub(super) fn is_agent_image_path(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            ["jpeg", "jpg", "png"]
                .iter()
                .any(|image_extension| extension.eq_ignore_ascii_case(image_extension))
        })
}

/// Whether a hovered file is an image that the agent prompt can take.
///
/// The web backend only gives the MIME type of a hovered file, not its path.
fn is_agent_image(file: &egui::HoveredFile) -> bool {
    match &file.path {
        Some(path) => is_agent_image_path(path),
        None => ["image/jpeg", "image/png"].contains(&file.mime.as_str()),
    }
}

/// Shows where hovered files will go when dropped.
///
/// Images go to the agent panel when there is an `agent_drop_target`, everything else is loaded
/// into the central panel, or the whole window if there is no `central_panel_rect`.
pub(super) fn preview_files_being_dropped(
    egui_ctx: &egui::Context,
    central_panel_rect: Option<egui::Rect>,
    agent_drop_target: Option<&re_agent_panel::ImageDropTarget>,
) {
    use egui::{Align2, Id, LayerId, Order, TextStyle};

    let hovered_files = egui_ctx.input(|i| i.raw.hovered_files.clone());
    if hovered_files.is_empty() {
        return;
    }

    let (images, files_to_load): (Vec<_>, Vec<_>) = hovered_files
        .iter()
        .partition(|file| agent_drop_target.is_some() && is_agent_image(file));

    let style = egui_ctx.global_style();
    let font = TextStyle::Body.resolve(&style);
    let text_color = style.visuals.strong_text_color();
    let fill = style.visuals.extreme_bg_color.gamma_multiply_u8(192);
    let stroke = egui_ctx.tokens().drop_target_container_stroke;

    let painter = egui_ctx.layer_painter(LayerId::new(
        Order::Foreground,
        Id::unique("file_drop_target"),
    ));

    let file_list = |title: &str, files: &[&egui::HoveredFile]| {
        use std::fmt::Write as _;

        let mut text = format!("{title}\n");
        for file in files {
            if let Some(path) = &file.path {
                write!(text, "\n{}", path.display()).ok();
            } else if !file.mime.is_empty() {
                write!(text, "\n{}", file.mime).ok();
            }
        }
        text
    };

    if let Some(agent_drop_target) = agent_drop_target
        && !images.is_empty()
    {
        let rect = agent_drop_target.rect;
        painter.rect_filled(rect, 0.0, fill);
        painter.rect_stroke(rect, 0.0, stroke, egui::StrokeKind::Inside);
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            if images.len() == 1 {
                "Add image to prompt"
            } else {
                "Add images to prompt"
            },
            font.clone(),
            text_color,
        );
    }

    if !files_to_load.is_empty() {
        let rect = central_panel_rect.unwrap_or_else(|| egui_ctx.content_rect());
        painter.rect_filled(rect, 0.0, fill);
        painter.rect_stroke(rect, 0.0, stroke, egui::StrokeKind::Inside);
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            file_list("Drop to load:", &files_to_load),
            font,
            text_color,
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

/// Is a recording likely to arrive within moments?
fn expects_data_soon(
    expect_data_soon: Option<bool>,
    sources: &[std::sync::Arc<LogSource>],
) -> bool {
    if let Some(expect_data_soon) = expect_data_soon {
        return expect_data_soon;
    }

    sources.iter().any(|source| match &**source {
        // Background streams (e.g. table blueprints) never open a recording.
        // Plain `rerun` starts a gRPC server by default, and nothing may ever be sent to it;
        // `spawn()` also uses that server, but passes `--expect-data-soon`, handled above.
        LogSource::RedapGrpcStream {
            open_behavior: RecordingOpenBehavior::Background,
            ..
        }
        | LogSource::MessageProxy { .. } => false,

        LogSource::File { .. }
        | LogSource::HttpStream { .. }
        | LogSource::JsChannel { .. }
        | LogSource::RedapGrpcStream { .. }
        | LogSource::RrdWebEvent
        | LogSource::Sdk
        | LogSource::Stdin => true,
    })
}
