use web_time::Instant;

use re_data_store::store_db::StoreDb;
use re_log_types::{LogMsg, StoreKind};
use re_renderer::WgpuResourcePoolStatistics;
use re_smart_channel::Receiver;
use re_ui::{toasts, UICommand, UICommandSender};
use re_viewer_context::{
    command_channel, AppOptions, CommandReceiver, CommandSender, ComponentUiRegistry,
    DynSpaceViewClass, PlayState, SpaceViewClassRegistry, SpaceViewClassRegistryError,
    StoreContext, SystemCommand, SystemCommandSender,
};

use crate::{
    app_blueprint::AppBlueprint,
    background_tasks::BackgroundTasks,
    store_hub::{StoreHub, StoreHubStats},
    viewer_analytics::ViewerAnalytics,
    AppState, StoreBundle,
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

    /// What is serialized
    pub(crate) state: AppState,

    /// Pending background tasks, e.g. files being saved.
    pub(crate) background_tasks: BackgroundTasks,

    /// Interface for all recordings and blueprints
    pub(crate) store_hub: Option<StoreHub>,

    /// Toast notifications.
    toasts: toasts::Toasts,

    memory_panel: crate::memory_panel::MemoryPanel,
    memory_panel_open: bool,

    pub(crate) latest_queue_interest: web_time::Instant,

    /// Measures how long a frame takes to paint
    pub(crate) frame_time_history: egui::util::History<f32>,

    /// Commands to run at the end of the frame.
    pub command_sender: CommandSender,
    command_receiver: CommandReceiver,
    cmd_palette: re_ui::CommandPalette,

    analytics: ViewerAnalytics,

    /// All known space view types.
    space_view_class_registry: SpaceViewClassRegistry,
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

        let (command_sender, command_receiver) = command_channel();

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
            state,
            background_tasks: Default::default(),
            store_hub: Some(StoreHub::default()),
            toasts: toasts::Toasts::new(),
            memory_panel: Default::default(),
            memory_panel_open: false,

            latest_queue_interest: web_time::Instant::now(), // TODO(emilk): `Instant::MIN` when we have our own `Instant` that supports it.

            frame_time_history: egui::util::History::new(1..100, 0.5),

            command_sender,
            command_receiver,
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
    pub fn add_space_view_class<T: DynSpaceViewClass + Default + 'static>(
        &mut self,
    ) -> Result<(), SpaceViewClassRegistryError> {
        self.space_view_class_registry.add::<T>()
    }

    fn check_keyboard_shortcuts(&self, egui_ctx: &egui::Context) {
        if let Some(cmd) = UICommand::listen_for_kb_shortcut(egui_ctx) {
            self.command_sender.send_ui(cmd);
        }
    }

    fn run_pending_system_commands(&mut self, store_hub: &mut StoreHub, egui_ctx: &egui::Context) {
        while let Some(cmd) = self.command_receiver.recv_system() {
            self.run_system_command(cmd, store_hub, egui_ctx);
        }
    }

    fn run_pending_ui_commands(
        &mut self,
        app_blueprint: &AppBlueprint<'_>,
        store_context: Option<&StoreContext<'_>>,
        frame: &mut eframe::Frame,
    ) {
        while let Some(cmd) = self.command_receiver.recv_ui() {
            self.run_ui_command(cmd, app_blueprint, store_context, frame);
        }
    }

    #[allow(clippy::unused_self)]
    fn run_system_command(
        &mut self,
        cmd: SystemCommand,
        store_hub: &mut StoreHub,
        egui_ctx: &egui::Context,
    ) {
        match cmd {
            SystemCommand::SetRecordingId(recording_id) => {
                store_hub.set_recording_id(recording_id);
            }
            #[cfg(not(target_arch = "wasm32"))]
            SystemCommand::LoadRrd(path) => {
                let with_notification = true;
                if let Some(rrd) = crate::loading::load_file_path(&path, with_notification) {
                    store_hub.add_bundle(rrd);
                }
            }
            SystemCommand::ResetViewer => self.reset(store_hub, egui_ctx),
            SystemCommand::UpdateBlueprint(blueprint_id, updates) => {
                let blueprint_db = store_hub.store_db_mut(&blueprint_id);
                for row in updates {
                    match blueprint_db.entity_db.try_add_data_row(&row) {
                        Ok(()) => {}
                        Err(err) => {
                            re_log::warn_once!("Failed to store blueprint delta: {err}",);
                        }
                    }
                }
            }
        }
    }

    fn run_ui_command(
        &mut self,
        cmd: UICommand,
        app_blueprint: &AppBlueprint<'_>,
        store_context: Option<&StoreContext<'_>>,
        _frame: &mut eframe::Frame,
    ) {
        match cmd {
            #[cfg(not(target_arch = "wasm32"))]
            UICommand::Save => {
                save(self, store_context, None);
            }
            #[cfg(not(target_arch = "wasm32"))]
            UICommand::SaveSelection => {
                save(
                    self,
                    store_context,
                    self.state.loop_selection(store_context),
                );
            }
            #[cfg(not(target_arch = "wasm32"))]
            UICommand::Open => {
                if let Some(rrd_file) = open_rrd_dialog() {
                    self.command_sender
                        .send_system(SystemCommand::LoadRrd(rrd_file));
                }
            }
            #[cfg(not(target_arch = "wasm32"))]
            UICommand::Quit => {
                _frame.close();
            }

            UICommand::ResetViewer => self.command_sender.send_system(SystemCommand::ResetViewer),

            #[cfg(not(target_arch = "wasm32"))]
            UICommand::OpenProfiler => {
                self.profiler.start();
            }

            UICommand::ToggleMemoryPanel => {
                self.memory_panel_open ^= true;
            }
            UICommand::ToggleBlueprintPanel => {
                app_blueprint.toggle_blueprint_panel(&self.command_sender);
            }
            UICommand::ToggleSelectionPanel => {
                app_blueprint.toggle_selection_panel(&self.command_sender);
            }
            UICommand::ToggleTimePanel => app_blueprint.toggle_time_panel(&self.command_sender),

            #[cfg(not(target_arch = "wasm32"))]
            UICommand::ToggleFullscreen => {
                _frame.set_fullscreen(!_frame.info().window_info.fullscreen);
            }
            #[cfg(not(target_arch = "wasm32"))]
            UICommand::ZoomIn => {
                self.app_options_mut().zoom_factor += 0.1;
            }
            #[cfg(not(target_arch = "wasm32"))]
            UICommand::ZoomOut => {
                self.app_options_mut().zoom_factor -= 0.1;
            }
            #[cfg(not(target_arch = "wasm32"))]
            UICommand::ZoomReset => {
                self.app_options_mut().zoom_factor = 1.0;
            }

            UICommand::SelectionPrevious => {
                let state = &mut self.state;
                if let Some(rec_cfg) = store_context
                    .and_then(|ctx| ctx.recording)
                    .map(|rec| rec.store_id())
                    .and_then(|rec_id| state.recording_config_mut(rec_id))
                {
                    rec_cfg.selection_state.select_previous();
                }
            }
            UICommand::SelectionNext => {
                let state = &mut self.state;
                if let Some(rec_cfg) = store_context
                    .and_then(|ctx| ctx.recording)
                    .map(|rec| rec.store_id())
                    .and_then(|rec_id| state.recording_config_mut(rec_id))
                {
                    rec_cfg.selection_state.select_next();
                }
            }
            UICommand::ToggleCommandPalette => {
                self.cmd_palette.toggle();
            }

            UICommand::PlaybackTogglePlayPause => {
                self.run_time_control_command(store_context, TimeControlCommand::TogglePlayPause);
            }
            UICommand::PlaybackFollow => {
                self.run_time_control_command(store_context, TimeControlCommand::Follow);
            }
            UICommand::PlaybackStepBack => {
                self.run_time_control_command(store_context, TimeControlCommand::StepBack);
            }
            UICommand::PlaybackStepForward => {
                self.run_time_control_command(store_context, TimeControlCommand::StepForward);
            }
            UICommand::PlaybackRestart => {
                self.run_time_control_command(store_context, TimeControlCommand::Restart);
            }

            #[cfg(not(target_arch = "wasm32"))]
            UICommand::ScreenshotWholeApp => {
                self.screenshotter.request_screenshot();
            }
            #[cfg(not(target_arch = "wasm32"))]
            UICommand::PrintDatastore => {
                if let Some(ctx) = store_context {
                    if let Some(recording) = ctx.recording {
                        let table = recording.entity_db.data_store.to_data_table();
                        println!("{table}");
                    }
                }
            }
        }
    }

    fn run_time_control_command(
        &mut self,
        store_context: Option<&StoreContext<'_>>,
        command: TimeControlCommand,
    ) {
        let Some(store_db) = store_context.as_ref().and_then(|ctx| ctx.recording) else { return; };
        let rec_id = store_db.store_id();
        let Some(rec_cfg) = self.state.recording_config_mut(rec_id) else { return; };
        let time_ctrl = &mut rec_cfg.time_ctrl;

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

    fn memory_panel_ui(
        &mut self,
        ui: &mut egui::Ui,
        gpu_resource_stats: &WgpuResourcePoolStatistics,
        store_stats: &StoreHubStats,
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
                    store_stats,
                );
            });
    }

    /// Top-level ui function.
    ///
    /// Shows the viewer ui.
    #[allow(clippy::too_many_arguments)]
    fn ui(
        &mut self,
        egui_ctx: &egui::Context,
        frame: &mut eframe::Frame,
        app_blueprint: &AppBlueprint<'_>,
        gpu_resource_stats: &WgpuResourcePoolStatistics,
        store_context: Option<&StoreContext<'_>>,
        store_stats: &StoreHubStats,
    ) {
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

                crate::ui::top_panel(
                    app_blueprint,
                    store_context,
                    ui,
                    frame,
                    self,
                    gpu_resource_stats,
                );

                self.memory_panel_ui(ui, gpu_resource_stats, store_stats);

                if let Some(store_view) = store_context {
                    // TODO(jleibs): We don't necessarily want to show the wait
                    // screen just because we're missing a recording. If we've
                    // loaded a blueprint, we can still show the empty layouts or
                    // static data, but we need to jump through some hoops to
                    // handle a missing `RecordingConfig` in this case.
                    if let Some(store_db) = store_view.recording {
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

                            self.state.show(
                                app_blueprint,
                                ui,
                                render_ctx,
                                store_db,
                                store_view,
                                &self.re_ui,
                                &self.component_ui_registry,
                                &self.space_view_class_registry,
                                &self.rx,
                                &self.command_sender,
                            );

                            render_ctx.before_submit();
                        }
                    } else {
                        crate::ui::wait_screen_ui(ui, &self.rx);
                    }
                } else {
                    crate::ui::wait_screen_ui(ui, &self.rx);
                }
            });
    }

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

    fn receive_messages(&mut self, store_hub: &mut StoreHub, egui_ctx: &egui::Context) {
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

            let is_new_store = matches!(&msg, LogMsg::SetStoreInfo(_msg));

            let store_db = store_hub.store_db_mut(store_id);

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

            // Set the recording-id after potentially creating the store in the
            // hub. This ordering is important because the `StoreHub` internally
            // updates the app-id when changing the recording.
            if let LogMsg::SetStoreInfo(msg) = &msg {
                match msg.info.store_id.kind {
                    StoreKind::Recording => {
                        re_log::debug!("Opening a new recording: {:?}", msg.info);
                        store_hub.set_recording_id(store_id.clone());
                    }

                    StoreKind::Blueprint => {
                        re_log::debug!("Opening a new blueprint: {:?}", msg.info);
                        store_hub.set_blueprint_for_app_id(
                            store_id.clone(),
                            msg.info.application_id.clone(),
                        );
                    }
                }
            }

            if start.elapsed() > web_time::Duration::from_millis(10) {
                egui_ctx.request_repaint(); // make sure we keep receiving messages asap
                break; // don't block the main thread for too long
            }
        }
    }

    fn purge_memory_if_needed(&mut self, store_hub: &mut StoreHub) {
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

            re_log::trace!("RAM limit: {}", format_limit(limit.limit));
            if let Some(resident) = mem_use_before.resident {
                re_log::trace!("Resident: {}", format_bytes(resident as _),);
            }
            if let Some(counted) = mem_use_before.counted {
                re_log::trace!("Counted: {}", format_bytes(counted as _));
            }

            re_tracing::profile_scope!("pruning");
            if let Some(counted) = mem_use_before.counted {
                re_log::trace!(
                    "Attempting to purge {:.1}% of used RAM ({})…",
                    100.0 * fraction_to_purge,
                    format_bytes(counted as f64 * fraction_to_purge as f64)
                );
            }
            store_hub.purge_fraction_of_ram(fraction_to_purge);
            self.state.cache.purge_memory();

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
    fn reset(&mut self, store_hub: &mut StoreHub, egui_ctx: &egui::Context) {
        self.state = Default::default();
        store_hub.clear_blueprint();

        // Keep the style:
        let style = egui_ctx.style();
        egui_ctx.memory_mut(|mem| *mem = Default::default());
        egui_ctx.set_style((*style).clone());
    }

    pub fn recording_db(&self) -> Option<&StoreDb> {
        self.store_hub
            .as_ref()
            .and_then(|store_hub| store_hub.current_recording())
    }

    fn on_rrd_loaded(&mut self, store_hub: &mut StoreHub, loaded_store_bundle: StoreBundle) {
        let mut new_rec_id = None;
        if let Some(store_db) = loaded_store_bundle.recordings().next() {
            new_rec_id = Some(store_db.store_id().clone());
            self.analytics.on_open_recording(store_db);
        }

        for blueprint_db in loaded_store_bundle.blueprints() {
            if let Some(app_id) = blueprint_db.app_id() {
                store_hub.set_blueprint_for_app_id(blueprint_db.store_id().clone(), app_id.clone());
            }
        }

        store_hub.add_bundle(loaded_store_bundle);

        // Set recording-id after adding to the store so that app-id, etc.
        // is available internally.
        if let Some(rec_id) = new_rec_id {
            store_hub.set_recording_id(rec_id);
        }
    }

    fn handle_dropping_files(&mut self, store_hub: &mut StoreHub, egui_ctx: &egui::Context) {
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
                    self.on_rrd_loaded(store_hub, rrd);

                    #[allow(clippy::needless_return)] // false positive on wasm32
                    return;
                }
            }

            #[cfg(not(target_arch = "wasm32"))]
            if let Some(path) = &file.path {
                let with_notification = true;
                if let Some(rrd) = crate::loading::load_file_path(path, with_notification) {
                    self.on_rrd_loaded(store_hub, rrd);
                }
            }
        }
    }
}

impl eframe::App for App {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0; 4] // transparent so we can get rounded corners when doing [`re_ui::CUSTOM_WINDOW_DECORATIONS`]
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        if self.startup_options.persist_state {
            // Save the app state
            eframe::set_value(storage, eframe::APP_KEY, &self.state);

            // Save the blueprints
            // TODO(2579): implement web-storage for blueprints as well
            #[cfg(not(target_arch = "wasm32"))]
            if let Some(hub) = &mut self.store_hub {
                match hub.persist_app_blueprints() {
                    Ok(f) => f,
                    Err(err) => {
                        re_log::error!("Saving blueprints failed: {err}");
                    }
                };
            }
        }
    }

    fn update(&mut self, egui_ctx: &egui::Context, frame: &mut eframe::Frame) {
        let frame_start = Instant::now();

        // Temporarily take the `StoreHub` out of the Viewer so it doesn't interfere with mutability
        let mut store_hub = self.store_hub.take().unwrap();

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
                self.state.app_options_mut().zoom_factor = zoom_factor;
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

        let store_stats = store_hub.stats();

        // do early, before doing too many allocations
        self.memory_panel.update(&gpu_resource_stats, &store_stats);

        self.check_keyboard_shortcuts(egui_ctx);

        self.purge_memory_if_needed(&mut store_hub);

        self.state.cache.begin_frame();

        self.show_text_logs_as_notifications();
        self.receive_messages(&mut store_hub, egui_ctx);

        store_hub.purge_empty();
        self.state.cleanup(&store_hub);

        file_saver_progress_ui(egui_ctx, &mut self.background_tasks); // toasts for background file saver

        let store_context = store_hub.read_context();

        let app_blueprint = AppBlueprint::new(store_context.as_ref(), egui_ctx);

        self.ui(
            egui_ctx,
            frame,
            &app_blueprint,
            &gpu_resource_stats,
            store_context.as_ref(),
            &store_stats,
        );

        if re_ui::CUSTOM_WINDOW_DECORATIONS {
            // Paint the main window frame on top of everything else
            paint_native_window_frame(egui_ctx);
        }

        if !self.screenshotter.is_screenshotting() {
            self.toasts.show(egui_ctx);
        }

        if let Some(cmd) = self.cmd_palette.show(egui_ctx) {
            self.command_sender.send_ui(cmd);
        }

        self.run_pending_ui_commands(&app_blueprint, store_context.as_ref(), frame);

        self.run_pending_system_commands(&mut store_hub, egui_ctx);

        self.handle_dropping_files(&mut store_hub, egui_ctx);

        // Return the `StoreHub` to the Viewer so we have it on the next frame
        self.store_hub = Some(store_hub);

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

/// Add built-in space views to the registry.
fn populate_space_view_class_registry_with_builtin(
    space_view_class_registry: &mut SpaceViewClassRegistry,
) -> Result<(), SpaceViewClassRegistryError> {
    space_view_class_registry.add::<re_space_view_bar_chart::BarChartSpaceView>()?;
    space_view_class_registry.add::<re_space_view_spatial::SpatialSpaceView>()?;
    space_view_class_registry.add::<re_space_view_tensor::TensorSpaceView>()?;
    space_view_class_registry.add::<re_space_view_text_box::TextBoxSpaceView>()?;
    space_view_class_registry.add::<re_space_view_text::TextSpaceView>()?;
    space_view_class_registry.add::<re_space_view_time_series::TimeSeriesSpaceView>()?;
    Ok(())
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
use std::path::PathBuf;
#[cfg(not(target_arch = "wasm32"))]
fn open_rrd_dialog() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("rerun data file", &["rrd"])
        .pick_file()
}

#[cfg(not(target_arch = "wasm32"))]
fn save(
    app: &mut App,
    store_context: Option<&StoreContext<'_>>,
    loop_selection: Option<(re_data_store::Timeline, re_log_types::TimeRangeF)>,
) {
    use crate::saving::save_database_to_file;

    let Some(store_db) = store_context.as_ref().and_then(|view| view.recording) else {
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
