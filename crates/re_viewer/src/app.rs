use itertools::Itertools as _;
use web_time::Instant;

use re_arrow_store::{DataStoreConfig, DataStoreStats};
use re_data_store::store_db::StoreDb;
use re_log_types::{ApplicationId, LogMsg, StoreId, StoreKind};
use re_renderer::WgpuResourcePoolStatistics;
use re_smart_channel::Receiver;
use re_ui::{toasts, Command};
use re_viewer_context::{
    AppOptions, ComponentUiRegistry, PlayState, SpaceViewClass, SpaceViewClassRegistry,
    SpaceViewClassRegistryError,
};

use crate::{
    background_tasks::BackgroundTasks, ui::Blueprint, viewer_analytics::ViewerAnalytics, AppState,
    StoreHub,
};

// ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum TimeControlCommand {
    TogglePlayPause,
    StepBack,
    StepForward,
    Restart,
    Follow,
}

// ----------------------------------------------------------------------------

/// Settings set once at startup (e.g. via command-line options) and not serialized.
#[derive(Clone)]
pub struct StartupOptions {
    pub memory_limit: re_memory::MemoryLimit,

    pub persist_state: bool,

    /// Take a screenshot of the app and quit.
    /// We use this to generate screenshots of our exmples.
    #[cfg(not(target_arch = "wasm32"))]
    pub screenshot_to_path_then_quit: Option<std::path::PathBuf>,

    /// Set the screen resolution in logical points.
    #[cfg(not(target_arch = "wasm32"))]
    pub resolution_in_points: Option<[f32; 2]>,
}

impl Default for StartupOptions {
    fn default() -> Self {
        Self {
            memory_limit: re_memory::MemoryLimit::default(),
            persist_state: true,

            #[cfg(not(target_arch = "wasm32"))]
            screenshot_to_path_then_quit: None,

            #[cfg(not(target_arch = "wasm32"))]
            resolution_in_points: None,
        }
    }
}

// ----------------------------------------------------------------------------

#[cfg(not(target_arch = "wasm32"))]
const MIN_ZOOM_FACTOR: f32 = 0.2;
#[cfg(not(target_arch = "wasm32"))]
const MAX_ZOOM_FACTOR: f32 = 4.0;

/// The Rerun viewer as an [`eframe`] application.
pub struct App {
    build_info: re_build_info::BuildInfo,
    startup_options: StartupOptions,
    ram_limit_warner: re_memory::RamLimitWarner,
    re_ui: re_ui::ReUi,
    screenshotter: crate::screenshotter::Screenshotter,

    #[cfg(not(target_arch = "wasm32"))]
    profiler: crate::Profiler,

    /// Listens to the local text log stream
    text_log_rx: std::sync::mpsc::Receiver<re_log::LogMsg>,

    component_ui_registry: ComponentUiRegistry,

    rx: Receiver<LogMsg>,

    /// Where the recordings and blueprints are stored.
    pub(crate) store_hub: crate::StoreHub,

    /// What is serialized
    pub(crate) state: AppState,

    /// Pending background tasks, e.g. files being saved.
    pub(crate) background_tasks: BackgroundTasks,

    /// Toast notifications.
    toasts: toasts::Toasts,

    memory_panel: crate::memory_panel::MemoryPanel,
    memory_panel_open: bool,

    pub(crate) latest_queue_interest: web_time::Instant,

    /// Measures how long a frame takes to paint
    pub(crate) frame_time_history: egui::util::History<f32>,

    /// Commands to run at the end of the frame.
    pub(crate) pending_commands: Vec<Command>,
    cmd_palette: re_ui::CommandPalette,

    analytics: ViewerAnalytics,

    /// All known space view types.
    space_view_class_registry: SpaceViewClassRegistry,
}

/// Add built-in space views to the registry.
fn populate_space_view_class_registry_with_builtin(
    space_view_class_registry: &mut SpaceViewClassRegistry,
) -> Result<(), SpaceViewClassRegistryError> {
    space_view_class_registry.add(re_space_view_text::TextSpaceView::default())?;
    space_view_class_registry.add(re_space_view_text_box::TextBoxSpaceView::default())?;
    Ok(())
}

impl App {
    /// Create a viewer that receives new log messages over time
    pub fn from_receiver(
        build_info: re_build_info::BuildInfo,
        app_env: &crate::AppEnvironment,
        startup_options: StartupOptions,
        re_ui: re_ui::ReUi,
        storage: Option<&dyn eframe::Storage>,
        rx: Receiver<LogMsg>,
    ) -> Self {
        let (logger, text_log_rx) = re_log::ChannelLogger::new(re_log::LevelFilter::Info);
        if re_log::add_boxed_logger(Box::new(logger)).is_err() {
            // This can happen when `rerun` crate users call `spawn`. TODO(emilk): make `spawn` spawn a new process.
            re_log::debug!(
                "re_log not initialized - we won't see any log messages as GUI notifications"
            );
        }

        let state: AppState = if startup_options.persist_state {
            storage
                .and_then(|storage| eframe::get_value(storage, eframe::APP_KEY))
                .unwrap_or_default()
        } else {
            AppState::default()
        };

        let mut analytics = ViewerAnalytics::new();
        analytics.on_viewer_started(&build_info, app_env);

        let mut space_view_class_registry = SpaceViewClassRegistry::default();
        if let Err(err) =
            populate_space_view_class_registry_with_builtin(&mut space_view_class_registry)
        {
            re_log::error!(
                "Failed to populate space view type registry with builtin space views: {}",
                err
            );
        }

        #[allow(unused_mut, clippy::needless_update)] // false positive on web
        let mut screenshotter = crate::screenshotter::Screenshotter::default();

        #[cfg(not(target_arch = "wasm32"))]
        if let Some(screenshot_path) = startup_options.screenshot_to_path_then_quit.clone() {
            screenshotter.screenshot_to_path_then_quit(screenshot_path);
        }

        Self {
            build_info,
            startup_options,
            ram_limit_warner: re_memory::RamLimitWarner::warn_at_fraction_of_max(0.75),
            re_ui,
            screenshotter,

            #[cfg(not(target_arch = "wasm32"))]
            profiler: Default::default(),

            text_log_rx,
            component_ui_registry: re_data_ui::create_component_ui_registry(),
            rx,
            store_hub: Default::default(),
            state,
            background_tasks: Default::default(),
            toasts: toasts::Toasts::new(),
            memory_panel: Default::default(),
            memory_panel_open: false,

            latest_queue_interest: web_time::Instant::now(), // TODO(emilk): `Instant::MIN` when we have our own `Instant` that supports it.

            frame_time_history: egui::util::History::new(1..100, 0.5),

            pending_commands: Default::default(),
            cmd_palette: Default::default(),

            space_view_class_registry,

            analytics,
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn set_profiler(&mut self, profiler: crate::Profiler) {
        self.profiler = profiler;
    }

    pub fn build_info(&self) -> &re_build_info::BuildInfo {
        &self.build_info
    }

    pub fn re_ui(&self) -> &re_ui::ReUi {
        &self.re_ui
    }

    pub fn app_options(&self) -> &AppOptions {
        self.state.app_options()
    }

    pub fn app_options_mut(&mut self) -> &mut AppOptions {
        self.state.app_options_mut()
    }

    pub fn is_screenshotting(&self) -> bool {
        self.screenshotter.is_screenshotting()
    }

    pub fn msg_receiver(&self) -> &Receiver<LogMsg> {
        &self.rx
    }

    /// Adds a new space view class to the viewer.
    pub fn add_space_view_class(
        &mut self,
        space_view_class: impl SpaceViewClass + 'static,
    ) -> Result<(), SpaceViewClassRegistryError> {
        self.space_view_class_registry.add(space_view_class)
    }

    fn check_keyboard_shortcuts(&mut self, egui_ctx: &egui::Context) {
        if let Some(cmd) = Command::listen_for_kb_shortcut(egui_ctx) {
            self.pending_commands.push(cmd);
        }
    }

    fn run_pending_commands(
        &mut self,
        blueprint: &mut Blueprint,
        egui_ctx: &egui::Context,
        frame: &mut eframe::Frame,
    ) {
        let commands = self.pending_commands.drain(..).collect_vec();
        for cmd in commands {
            self.run_command(cmd, blueprint, frame, egui_ctx);
        }
    }

    fn run_command(
        &mut self,
        cmd: Command,
        blueprint: &mut Blueprint,
        _frame: &mut eframe::Frame,
        egui_ctx: &egui::Context,
    ) {
        let is_narrow_screen = egui_ctx.screen_rect().width() < 600.0; // responsive ui for mobiles etc

        match cmd {
            #[cfg(not(target_arch = "wasm32"))]
            Command::Save => {
                save(self, None);
            }
            #[cfg(not(target_arch = "wasm32"))]
            Command::SaveSelection => {
                save(self, self.state.loop_selection());
            }
            #[cfg(not(target_arch = "wasm32"))]
            Command::Open => {
                if let Some(rrd) = open_rrd_dialog() {
                    self.on_rrd_loaded(rrd);
                }
            }
            #[cfg(not(target_arch = "wasm32"))]
            Command::Quit => {
                _frame.close();
            }

            Command::ResetViewer => {
                self.reset(egui_ctx);
            }

            #[cfg(not(target_arch = "wasm32"))]
            Command::OpenProfiler => {
                self.profiler.start();
            }

            Command::ToggleMemoryPanel => {
                self.memory_panel_open ^= true;
            }
            Command::ToggleBlueprintPanel => {
                blueprint.blueprint_panel_expanded ^= true;

                // Only one of blueprint or selection panel can be open at a time on mobile:
                if is_narrow_screen && blueprint.blueprint_panel_expanded {
                    blueprint.selection_panel_expanded = false;
                }
            }
            Command::ToggleSelectionPanel => {
                blueprint.selection_panel_expanded ^= true;

                // Only one of blueprint or selection panel can be open at a time on mobile:
                if is_narrow_screen && blueprint.selection_panel_expanded {
                    blueprint.blueprint_panel_expanded = false;
                }
            }
            Command::ToggleTimePanel => {
                blueprint.time_panel_expanded ^= true;
            }

            #[cfg(not(target_arch = "wasm32"))]
            Command::ToggleFullscreen => {
                _frame.set_fullscreen(!_frame.info().window_info.fullscreen);
            }
            #[cfg(not(target_arch = "wasm32"))]
            Command::ZoomIn => {
                self.app_options_mut().zoom_factor += 0.1;
            }
            #[cfg(not(target_arch = "wasm32"))]
            Command::ZoomOut => {
                self.app_options_mut().zoom_factor -= 0.1;
            }
            #[cfg(not(target_arch = "wasm32"))]
            Command::ZoomReset => {
                self.app_options_mut().zoom_factor = 1.0;
            }

            Command::SelectionPrevious => {
                let state = &mut self.state;
                if let Some(rec_cfg) = state
                    .recording_id()
                    .and_then(|rec_id| state.recording_config_mut(&rec_id))
                {
                    rec_cfg.selection_state.select_previous();
                }
            }
            Command::SelectionNext => {
                let state = &mut self.state;
                if let Some(rec_cfg) = state
                    .recording_id()
                    .and_then(|rec_id| state.recording_config_mut(&rec_id))
                {
                    rec_cfg.selection_state.select_next();
                }
            }
            Command::ToggleCommandPalette => {
                self.cmd_palette.toggle();
            }

            Command::PlaybackTogglePlayPause => {
                self.run_time_control_command(TimeControlCommand::TogglePlayPause);
            }
            Command::PlaybackFollow => {
                self.run_time_control_command(TimeControlCommand::Follow);
            }
            Command::PlaybackStepBack => {
                self.run_time_control_command(TimeControlCommand::StepBack);
            }
            Command::PlaybackStepForward => {
                self.run_time_control_command(TimeControlCommand::StepForward);
            }
            Command::PlaybackRestart => {
                self.run_time_control_command(TimeControlCommand::Restart);
            }

            #[cfg(not(target_arch = "wasm32"))]
            Command::ScreenshotWholeApp => {
                self.screenshotter.request_screenshot();
            }
        }
    }

    fn run_time_control_command(&mut self, command: TimeControlCommand) {
        let Some(rec_id) = self.state.recording_id() else { return; };
        let Some(rec_cfg) = self.state.recording_config_mut(&rec_id) else { return; };
        let time_ctrl = &mut rec_cfg.time_ctrl;

        let Some(store_db) = self.store_hub.recording(&rec_id) else { return; };
        let times_per_timeline = store_db.times_per_timeline();

        match command {
            TimeControlCommand::TogglePlayPause => {
                time_ctrl.toggle_play_pause(times_per_timeline);
            }
            TimeControlCommand::Follow => {
                time_ctrl.set_play_state(times_per_timeline, PlayState::Following);
            }
            TimeControlCommand::StepBack => {
                time_ctrl.step_time_back(times_per_timeline);
            }
            TimeControlCommand::StepForward => {
                time_ctrl.step_time_fwd(times_per_timeline);
            }
            TimeControlCommand::Restart => {
                time_ctrl.restart(times_per_timeline);
            }
        }
    }

    /// The app ID of active blueprint.
    pub fn selected_app_id(&self) -> ApplicationId {
        self.recording_db()
            .and_then(|store_db| {
                store_db
                    .store_info()
                    .map(|store_info| store_info.application_id.clone())
            })
            .unwrap_or(ApplicationId::unknown())
    }

    fn memory_panel_ui(
        &mut self,
        ui: &mut egui::Ui,
        gpu_resource_stats: &WgpuResourcePoolStatistics,
        store_config: &DataStoreConfig,
        blueprint_config: &DataStoreConfig,
        store_stats: &DataStoreStats,
        blueprint_stats: &DataStoreStats,
    ) {
        let frame = egui::Frame {
            fill: ui.visuals().panel_fill,
            ..self.re_ui.bottom_panel_frame()
        };

        egui::TopBottomPanel::bottom("memory_panel")
            .default_height(300.0)
            .resizable(true)
            .frame(frame)
            .show_animated_inside(ui, self.memory_panel_open, |ui| {
                self.memory_panel.ui(
                    ui,
                    &self.startup_options.memory_limit,
                    gpu_resource_stats,
                    store_config,
                    blueprint_config,
                    store_stats,
                    blueprint_stats,
                );
            });
    }

    // Materialize the blueprint from the DB if the selected blueprint id isn't the default one.
    fn load_or_create_blueprint(
        &mut self,
        this_frame_blueprint_id: &StoreId,
        egui_ctx: &egui::Context,
    ) -> Blueprint {
        let blueprint_db = self.store_hub.blueprint_entry(this_frame_blueprint_id);
        Blueprint::from_db(egui_ctx, blueprint_db)
    }

    /// Top-level ui function.
    ///
    /// Shows the viewer ui.
    #[allow(clippy::too_many_arguments)]
    fn ui(
        &mut self,
        egui_ctx: &egui::Context,
        frame: &mut eframe::Frame,
        blueprint: &mut Blueprint,
        gpu_resource_stats: &WgpuResourcePoolStatistics,
        blueprint_config: &DataStoreConfig,
        store_stats: &DataStoreStats,
        blueprint_stats: &DataStoreStats,
    ) {
        let store_config = self
            .recording_db()
            .map(|store_db| store_db.entity_db.data_store.config().clone())
            .unwrap_or_default();

        let mut main_panel_frame = egui::Frame::default();
        if re_ui::CUSTOM_WINDOW_DECORATIONS {
            // Add some margin so that we can later paint an outline around it all.
            main_panel_frame.inner_margin = 1.0.into();
        }

        egui::CentralPanel::default()
            .frame(main_panel_frame)
            .show(egui_ctx, |ui| {
                paint_background_fill(ui);

                crate::ui::mobile_warning_ui(&self.re_ui, ui);

                crate::ui::top_panel(&*blueprint, ui, frame, self, gpu_resource_stats);

                self.memory_panel_ui(
                    ui,
                    gpu_resource_stats,
                    &store_config,
                    blueprint_config,
                    store_stats,
                    blueprint_stats,
                );

                // NOTE: cannot call `.store_db()` due to borrowck shenanigans
                if let Some(store_db) = self
                    .state
                    .recording_id()
                    .and_then(|rec_id| self.store_hub.recording(&rec_id))
                {
                    self.state
                        .recording_config_entry(
                            store_db.store_id().clone(),
                            self.rx.source(),
                            store_db,
                        )
                        .selection_state
                        .on_frame_start(|item| blueprint.is_item_valid(item));

                    // TODO(andreas): store the re_renderer somewhere else.
                    let egui_renderer = {
                        let render_state = frame.wgpu_render_state().unwrap();
                        &mut render_state.renderer.write()
                    };
                    if let Some(render_ctx) = egui_renderer
                        .paint_callback_resources
                        .get_mut::<re_renderer::RenderContext>()
                    {
                        render_ctx.begin_frame();

                        if store_db.is_empty() {
                            crate::ui::wait_screen_ui(ui, &self.rx);
                        } else {
                            self.state.show(
                                blueprint,
                                ui,
                                render_ctx,
                                store_db,
                                &self.re_ui,
                                &self.component_ui_registry,
                                &self.space_view_class_registry,
                                &self.rx,
                            );

                            render_ctx.before_submit();
                        }
                    }
                } else {
                    crate::ui::wait_screen_ui(ui, &self.rx);
                }
            });
    }
}

impl eframe::App for App {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0; 4] // transparent so we can get rounded corners when doing [`re_ui::CUSTOM_WINDOW_DECORATIONS`]
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        if self.startup_options.persist_state {
            eframe::set_value(storage, eframe::APP_KEY, &self.state);
        }
    }

    fn update(&mut self, egui_ctx: &egui::Context, frame: &mut eframe::Frame) {
        let frame_start = Instant::now();

        #[cfg(not(target_arch = "wasm32"))]
        if let Some(resolution_in_points) = self.startup_options.resolution_in_points.take() {
            frame.set_window_size(resolution_in_points.into());
        }

        #[cfg(not(target_arch = "wasm32"))]
        if self.screenshotter.update(egui_ctx, frame).quit {
            frame.close();
            return;
        }

        if self.startup_options.memory_limit.limit.is_none() {
            // we only warn about high memory usage if the user hasn't specified a limit
            self.ram_limit_warner.update();
        }

        #[cfg(not(target_arch = "wasm32"))]
        if self.screenshotter.is_screenshotting() {
            // Make screenshots high-quality by pretending we have a high-dpi display, whether we do or not:
            egui_ctx.set_pixels_per_point(2.0);
        } else {
            // Ensure zoom factor is sane and in 10% steps at all times before applying it.
            {
                let mut zoom_factor = self.app_options().zoom_factor;
                zoom_factor = zoom_factor.clamp(MIN_ZOOM_FACTOR, MAX_ZOOM_FACTOR);
                zoom_factor = (zoom_factor * 10.).round() / 10.;
                self.app_options_mut().zoom_factor = zoom_factor;
            }

            // Apply zoom factor on top of natively reported pixel per point.
            let pixels_per_point = frame.info().native_pixels_per_point.unwrap_or(1.0)
                * self.app_options().zoom_factor;
            egui_ctx.set_pixels_per_point(pixels_per_point);
        }

        // TODO(andreas): store the re_renderer somewhere else.
        let gpu_resource_stats = {
            let egui_renderer = {
                let render_state = frame.wgpu_render_state().unwrap();
                &mut render_state.renderer.read()
            };
            let render_ctx = egui_renderer
                .paint_callback_resources
                .get::<re_renderer::RenderContext>()
                .unwrap();

            // Query statistics before begin_frame as this might be more accurate if there's resources that we recreate every frame.
            render_ctx.gpu_resources.statistics()
        };

        // Look up the blueprint in use for this frame.
        // Note that it's important that we save this because it's possible that it will be changed
        // by the end up the frame (e.g. if the user selects a different recording), but we need it
        // to save changes back to the correct blueprint at the end.
        let active_blueprint_id = self
            .state
            .selected_blueprint_by_app
            .get(&self.selected_app_id())
            .cloned()
            .unwrap_or_else(|| {
                StoreId::from_string(StoreKind::Blueprint, self.selected_app_id().0)
            });

        let store_stats = self
            .recording_db()
            .map(|store_db| DataStoreStats::from_store(&store_db.entity_db.data_store))
            .unwrap_or_default();

        let blueprint_stats = self
            .store_hub
            .blueprint_mut(&active_blueprint_id)
            .map(|bp_db| DataStoreStats::from_store(&bp_db.entity_db.data_store))
            .unwrap_or_default();

        // do early, before doing too many allocations
        self.memory_panel
            .update(&gpu_resource_stats, &store_stats, &blueprint_stats);

        self.check_keyboard_shortcuts(egui_ctx);

        self.purge_memory_if_needed();

        self.state.cache.begin_frame();

        self.show_text_logs_as_notifications();
        self.receive_messages(egui_ctx);

        self.store_hub.purge_empty();
        self.state.cleanup(&self.store_hub);

        file_saver_progress_ui(egui_ctx, &mut self.background_tasks); // toasts for background file saver

        let blueprint_snapshot = self.load_or_create_blueprint(&active_blueprint_id, egui_ctx);

        // Make a mutable copy we can edit.
        let mut blueprint = blueprint_snapshot.clone();

        let blueprint_config = self
            .store_hub
            .blueprint_mut(&active_blueprint_id)
            .map(|bp_db| bp_db.entity_db.data_store.config().clone())
            .unwrap_or_default();

        self.ui(
            egui_ctx,
            frame,
            &mut blueprint,
            &gpu_resource_stats,
            &blueprint_config,
            &store_stats,
            &blueprint_stats,
        );

        if re_ui::CUSTOM_WINDOW_DECORATIONS {
            // Paint the main window frame on top of everything else
            paint_native_window_frame(egui_ctx);
        }

        self.handle_dropping_files(egui_ctx);

        if !self.screenshotter.is_screenshotting() {
            self.toasts.show(egui_ctx);
        }

        if let Some(cmd) = self.cmd_palette.show(egui_ctx) {
            self.pending_commands.push(cmd);
        }

        self.run_pending_commands(&mut blueprint, egui_ctx, frame);

        // If there was a real active blueprint that came from the store, save the changes back.
        if let Some(blueprint_db) = self.store_hub.blueprint_mut(&active_blueprint_id) {
            blueprint.sync_changes_to_store(&blueprint_snapshot, blueprint_db);
        } else {
            // This shouldn't happen because we should have used `active_blueprint_id` to
            // create this same blueprint in `load_or_create_blueprint`, but we couldn't
            // keep it around for borrow-checker reasons.
            re_log::warn_once!("Blueprint unexpectedly missing from store.");
        }

        // Frame time measurer - must be last
        self.frame_time_history.add(
            egui_ctx.input(|i| i.time),
            frame_start.elapsed().as_secs_f32(),
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn post_rendering(&mut self, _window_size: [u32; 2], frame: &eframe::Frame) {
        if let Some(screenshot) = frame.screenshot() {
            self.screenshotter.save(&screenshot);
        }
    }
}

fn paint_background_fill(ui: &mut egui::Ui) {
    // This is required because the streams view (time panel)
    // has rounded top corners, which leaves a gap.
    // So we fill in that gap (and other) here.
    // Of course this does some over-draw, but we have to live with that.

    ui.painter().rect_filled(
        ui.max_rect().shrink(0.5),
        re_ui::ReUi::native_window_rounding(),
        ui.visuals().panel_fill,
    );
}

fn paint_native_window_frame(egui_ctx: &egui::Context) {
    let painter = egui::Painter::new(
        egui_ctx.clone(),
        egui::LayerId::new(egui::Order::TOP, egui::Id::new("native_window_frame")),
        egui::Rect::EVERYTHING,
    );

    let stroke = egui::Stroke::new(1.0, egui::Color32::from_gray(42)); // from figma 2022-02-06

    painter.rect_stroke(
        egui_ctx.screen_rect().shrink(0.5),
        re_ui::ReUi::native_window_rounding(),
        stroke,
    );
}

impl App {
    /// Show recent text log messages to the user as toast notifications.
    fn show_text_logs_as_notifications(&mut self) {
        re_tracing::profile_function!();

        while let Ok(re_log::LogMsg { level, target, msg }) = self.text_log_rx.try_recv() {
            let is_rerun_crate = target.starts_with("rerun") || target.starts_with("re_");
            if !is_rerun_crate {
                continue;
            }

            let kind = match level {
                re_log::Level::Error => toasts::ToastKind::Error,
                re_log::Level::Warn => toasts::ToastKind::Warning,
                re_log::Level::Info => toasts::ToastKind::Info,
                re_log::Level::Debug | re_log::Level::Trace => {
                    continue; // too spammy
                }
            };

            self.toasts.add(toasts::Toast {
                kind,
                text: msg,
                options: toasts::ToastOptions::with_ttl_in_seconds(4.0),
            });
        }
    }

    fn receive_messages(&mut self, egui_ctx: &egui::Context) {
        re_tracing::profile_function!();

        let start = web_time::Instant::now();

        while let Ok(msg) = self.rx.try_recv() {
            let msg = match msg.payload {
                re_smart_channel::SmartMessagePayload::Msg(msg) => msg,
                re_smart_channel::SmartMessagePayload::Quit(err) => {
                    if let Some(err) = err {
                        re_log::warn!(%msg.source, err, "data source has left unexpectedly");
                    } else {
                        re_log::debug!(%msg.source, "data source has left");
                    }
                    continue;
                }
            };

            let store_id = msg.store_id();

            let is_new_store = if let LogMsg::SetStoreInfo(msg) = &msg {
                match msg.info.store_id.kind {
                    StoreKind::Recording => {
                        re_log::debug!("Opening a new recording: {:?}", msg.info);
                        self.state.set_recording_id(store_id.clone());
                    }

                    StoreKind::Blueprint => {
                        re_log::debug!("Opening a new blueprint: {:?}", msg.info);
                        self.state
                            .selected_blueprint_by_app
                            .insert(msg.info.application_id.clone(), msg.info.store_id.clone());
                    }
                }
                true
            } else {
                false
            };

            let store_db = self.store_hub.store_db_entry(store_id);

            if store_db.data_source.is_none() {
                store_db.data_source = Some(self.rx.source().clone());
            }

            if let Err(err) = store_db.add(&msg) {
                re_log::error!("Failed to add incoming msg: {err}");
            };

            if is_new_store && store_db.store_kind() == StoreKind::Recording {
                // Do analytics after ingesting the new message,
                // because thats when the `store_db.store_info` is set,
                // which we use in the analytics call.
                self.analytics.on_open_recording(store_db);
            }

            if start.elapsed() > web_time::Duration::from_millis(10) {
                egui_ctx.request_repaint(); // make sure we keep receiving messages asap
                break; // don't block the main thread for too long
            }
        }
    }

    fn purge_memory_if_needed(&mut self) {
        re_tracing::profile_function!();

        fn format_limit(limit: Option<i64>) -> String {
            if let Some(bytes) = limit {
                format_bytes(bytes as _)
            } else {
                "∞".to_owned()
            }
        }

        use re_format::format_bytes;
        use re_memory::MemoryUse;

        let limit = self.startup_options.memory_limit;
        let mem_use_before = MemoryUse::capture();

        if let Some(minimum_fraction_to_purge) = limit.is_exceeded_by(&mem_use_before) {
            let fraction_to_purge = (minimum_fraction_to_purge + 0.2).clamp(0.25, 1.0);

            re_log::debug!("RAM limit: {}", format_limit(limit.limit));
            if let Some(resident) = mem_use_before.resident {
                re_log::debug!("Resident: {}", format_bytes(resident as _),);
            }
            if let Some(counted) = mem_use_before.counted {
                re_log::debug!("Counted: {}", format_bytes(counted as _));
            }

            {
                re_tracing::profile_scope!("pruning");
                if let Some(counted) = mem_use_before.counted {
                    re_log::debug!(
                        "Attempting to purge {:.1}% of used RAM ({})…",
                        100.0 * fraction_to_purge,
                        format_bytes(counted as f64 * fraction_to_purge as f64)
                    );
                }
                self.store_hub.purge_fraction_of_ram(fraction_to_purge);
                self.state.cache.purge_memory();
            }

            let mem_use_after = MemoryUse::capture();

            let freed_memory = mem_use_before - mem_use_after;

            if let (Some(counted_before), Some(counted_diff)) =
                (mem_use_before.counted, freed_memory.counted)
            {
                re_log::debug!(
                    "Freed up {} ({:.1}%)",
                    format_bytes(counted_diff as _),
                    100.0 * counted_diff as f32 / counted_before as f32
                );
            }

            self.memory_panel.note_memory_purge();
        }
    }

    /// Reset the viewer to how it looked the first time you ran it.
    fn reset(&mut self, egui_ctx: &egui::Context) {
        let selected_rec_id = self.state.recording_id();
        self.state = Default::default();
        if let Some(selected_rec_id) = selected_rec_id {
            self.state.set_recording_id(selected_rec_id);
        }

        // Keep the style:
        let style = egui_ctx.style();
        egui_ctx.memory_mut(|mem| *mem = Default::default());
        egui_ctx.set_style((*style).clone());
    }

    /// Get access to the [`StoreDb`] of the currently shown recording, if any.
    pub fn recording_db(&self) -> Option<&StoreDb> {
        self.state
            .recording_id()
            .and_then(|rec_id| self.store_hub.recording(&rec_id))
    }

    fn on_rrd_loaded(&mut self, store_hub: StoreHub) {
        if let Some(store_db) = store_hub.recordings().next() {
            self.state.set_recording_id(store_db.store_id().clone());
            self.analytics.on_open_recording(store_db);
        }

        for blueprint_db in store_hub.blueprints() {
            if let Some(app_id) = blueprint_db.app_id() {
                self.state
                    .selected_blueprint_by_app
                    .insert(app_id.clone(), blueprint_db.store_id().clone());
            }
        }

        self.store_hub.append(store_hub);
    }

    fn handle_dropping_files(&mut self, egui_ctx: &egui::Context) {
        preview_files_being_dropped(egui_ctx);

        // Collect dropped files:
        if egui_ctx.input(|i| i.raw.dropped_files.len()) > 2 {
            rfd::MessageDialog::new()
                .set_level(rfd::MessageLevel::Error)
                .set_description("Can only load one file at a time")
                .show();
        }
        if let Some(file) = egui_ctx.input(|i| i.raw.dropped_files.first().cloned()) {
            if let Some(bytes) = &file.bytes {
                let mut bytes: &[u8] = &(*bytes)[..];
                if let Some(rrd) = crate::loading::load_file_contents(&file.name, &mut bytes) {
                    self.on_rrd_loaded(rrd);

                    #[allow(clippy::needless_return)] // false positive on wasm32
                    return;
                }
            }

            #[cfg(not(target_arch = "wasm32"))]
            if let Some(path) = &file.path {
                if let Some(rrd) = crate::loading::load_file_path(path) {
                    self.on_rrd_loaded(rrd);
                }
            }
        }
    }
}

fn preview_files_being_dropped(egui_ctx: &egui::Context) {
    use egui::{Align2, Color32, Id, LayerId, Order, TextStyle};

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

        let screen_rect = egui_ctx.screen_rect();
        painter.rect_filled(screen_rect, 0.0, Color32::from_black_alpha(192));
        painter.text(
            screen_rect.center(),
            Align2::CENTER_CENTER,
            text,
            TextStyle::Body.resolve(&egui_ctx.style()),
            Color32::WHITE,
        );
    }
}

// ----------------------------------------------------------------------------

fn file_saver_progress_ui(egui_ctx: &egui::Context, background_tasks: &mut BackgroundTasks) {
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
                        ui.spinner();
                        ui.label("Writing file to disk…");
                    })
                });
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn open_rrd_dialog() -> Option<StoreHub> {
    if let Some(path) = rfd::FileDialog::new()
        .add_filter("rerun data file", &["rrd"])
        .pick_file()
    {
        crate::loading::load_file_path(&path)
    } else {
        None
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn save(
    app: &mut App,
    loop_selection: Option<(re_data_store::Timeline, re_log_types::TimeRangeF)>,
) {
    let Some(store_db) = app.recording_db() else {
        // NOTE: Can only happen if saving through the command palette.
        re_log::error!("No data to save!");
        return;
    };

    let title = if loop_selection.is_some() {
        "Save loop selection"
    } else {
        "Save"
    };

    if let Some(path) = rfd::FileDialog::new()
        .set_file_name("data.rrd")
        .set_title(title)
        .save_file()
    {
        let f = match save_database_to_file(store_db, path, loop_selection) {
            Ok(f) => f,
            Err(err) => {
                re_log::error!("File saving failed: {err}");
                return;
            }
        };
        if let Err(err) = app.background_tasks.spawn_file_saver(f) {
            // NOTE: Can only happen if saving through the command palette.
            re_log::error!("File saving failed: {err}");
        }
    }
}

/// Returns a closure that, when run, will save the contents of the current database
/// to disk, at the specified `path`.
///
/// If `time_selection` is specified, then only data for that specific timeline over that
/// specific time range will be accounted for.
#[cfg(not(target_arch = "wasm32"))]
fn save_database_to_file(
    store_db: &StoreDb,
    path: std::path::PathBuf,
    time_selection: Option<(re_data_store::Timeline, re_log_types::TimeRangeF)>,
) -> anyhow::Result<impl FnOnce() -> anyhow::Result<std::path::PathBuf>> {
    use re_arrow_store::TimeRange;

    re_tracing::profile_scope!("dump_messages");

    let begin_rec_msg = store_db
        .recording_msg()
        .map(|msg| LogMsg::SetStoreInfo(msg.clone()));

    let ent_op_msgs = store_db
        .iter_entity_op_msgs()
        .map(|msg| LogMsg::EntityPathOpMsg(store_db.store_id().clone(), msg.clone()))
        .collect_vec();

    let time_filter = time_selection.map(|(timeline, range)| {
        (
            timeline,
            TimeRange::new(range.min.floor(), range.max.ceil()),
        )
    });
    let data_msgs: Result<Vec<_>, _> = store_db
        .entity_db
        .data_store
        .to_data_tables(time_filter)
        .map(|table| {
            table
                .to_arrow_msg()
                .map(|msg| LogMsg::ArrowMsg(store_db.store_id().clone(), msg))
        })
        .collect();

    use anyhow::Context as _;
    let data_msgs = data_msgs.with_context(|| "Failed to export to data tables")?;

    let msgs = std::iter::once(begin_rec_msg)
        .flatten() // option
        .chain(ent_op_msgs)
        .chain(data_msgs);

    Ok(move || {
        re_tracing::profile_scope!("save_to_file");

        use anyhow::Context as _;
        let file = std::fs::File::create(path.as_path())
            .with_context(|| format!("Failed to create file at {path:?}"))?;

        let encoding_options = re_log_encoding::EncodingOptions::COMPRESSED;
        re_log_encoding::encoder::encode_owned(encoding_options, msgs, file)
            .map(|_| path)
            .context("Message encode")
    })
}
