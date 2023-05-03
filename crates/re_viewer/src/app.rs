use std::{any::Any, hash::Hash};

use ahash::HashMap;
use egui::NumExt as _;
use instant::Instant;
use itertools::Itertools as _;
use nohash_hasher::IntMap;
use poll_promise::Promise;

use re_arrow_store::{DataStoreConfig, DataStoreStats};
use re_data_store::log_db::LogDb;
use re_format::format_number;
use re_log_types::{ApplicationId, LogMsg, RecordingId};
use re_renderer::WgpuResourcePoolStatistics;
use re_smart_channel::Receiver;
use re_ui::{toasts, Command};

use crate::{
    app_icon::setup_app_icon,
    depthai::depthai,
    misc::{AppOptions, Caches, RecordingConfig, ViewerContext},
    ui::{data_ui::ComponentUiRegistry, Blueprint},
    viewer_analytics::ViewerAnalytics,
};

#[cfg(not(target_arch = "wasm32"))]
use re_log_types::TimeRangeF;

use super::app_icon::AppIconStatus;

const WATERMARK: bool = false; // Nice for recording media material

// ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum TimeControlCommand {
    TogglePlayPause,
    StepBack,
    StepForward,
    Restart,
}

// ----------------------------------------------------------------------------

/// Settings set once at startup (e.g. via command-line options) and not serialized.
#[derive(Clone, Copy, Default)]
pub struct StartupOptions {
    pub memory_limit: re_memory::MemoryLimit,
    pub persist_state: bool,
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

    /// Listens to the local text log stream
    text_log_rx: std::sync::mpsc::Receiver<re_log::LogMsg>,

    component_ui_registry: ComponentUiRegistry,

    rx: Receiver<LogMsg>,

    /// Where the logs are stored.
    log_dbs: IntMap<RecordingId, LogDb>,

    /// What is serialized
    state: AppState,

    /// Set to `true` on Ctrl-C.
    shutdown: std::sync::Arc<std::sync::atomic::AtomicBool>,

    /// Pending background tasks, using `poll_promise`.
    pending_promises: HashMap<String, Promise<Box<dyn Any + Send>>>,

    /// Toast notifications.
    toasts: toasts::Toasts,

    memory_panel: crate::memory_panel::MemoryPanel,
    memory_panel_open: bool,

    latest_queue_interest: instant::Instant,

    /// Measures how long a frame takes to paint
    frame_time_history: egui::util::History<f32>,

    /// Commands to run at the end of the frame.
    pending_commands: Vec<Command>,
    cmd_palette: re_ui::CommandPalette,

    analytics: ViewerAnalytics,

    icon_status: AppIconStatus,

    #[cfg(not(target_arch = "wasm32"))]
    backend_handle: Option<std::process::Child>,
}

impl App {
    #[cfg(not(target_arch = "wasm32"))]
    fn spawn_backend() -> Option<std::process::Child> {
        // TODO(filip): Is there some way I can know for sure where depthai_viewer_backend is?
        let backend_handle = match std::process::Command::new("python")
            .args(["-m", "depthai_viewer_backend"])
            .spawn()
        {
            Ok(child) => {
                println!("Backend started successfully.");
                Some(child)
            }
            Err(err) => {
                eprintln!("Failed to start depthai viewer: {err}");
                match std::process::Command::new("python3")
                    .args(["-m", "depthai_viewer_backend"])
                    .spawn()
                {
                    Ok(child) => {
                        println!("Backend started successfully.");
                        Some(child)
                    }
                    Err(err) => {
                        eprintln!("Failed to start depthai_viewer {err}");
                        None
                    }
                }
            }
        };
        // assert!(
        //     backend_handle.is_some(),
        //     "Couldn't start backend, exiting..."
        // );
        backend_handle
    }

    /// Create a viewer that receives new log messages over time
    pub fn from_receiver(
        build_info: re_build_info::BuildInfo,
        app_env: &crate::AppEnvironment,
        startup_options: StartupOptions,
        re_ui: re_ui::ReUi,
        storage: Option<&dyn eframe::Storage>,
        rx: Receiver<LogMsg>,
        shutdown: std::sync::Arc<std::sync::atomic::AtomicBool>,
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

        Self {
            build_info,
            startup_options,
            ram_limit_warner: re_memory::RamLimitWarner::warn_at_fraction_of_max(0.75),
            re_ui,
            text_log_rx,
            component_ui_registry: Default::default(),
            rx,
            log_dbs: Default::default(),
            state,
            shutdown,
            pending_promises: Default::default(),
            toasts: toasts::Toasts::new(),
            memory_panel: Default::default(),
            memory_panel_open: false,

            latest_queue_interest: instant::Instant::now(), // TODO(emilk): `Instant::MIN` when we have our own `Instant` that supports it.

            frame_time_history: egui::util::History::new(1..100, 0.5),

            pending_commands: Default::default(),
            cmd_palette: Default::default(),

            analytics,

            icon_status: AppIconStatus::NotSetTryAgain,
            #[cfg(not(target_arch = "wasm32"))]
            backend_handle: App::spawn_backend(),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn set_profiler(&mut self, profiler: crate::Profiler) {
        self.state.profiler = profiler;
    }

    /// Creates a promise with the specified name that will run `f` on a background
    /// thread using the `poll_promise` crate.
    ///
    /// Names can only be re-used once the promise with that name has finished running,
    /// otherwise an other is returned.
    // TODO(cmc): offer `spawn_async_promise` once we open save_file to the web
    #[cfg(not(target_arch = "wasm32"))]
    pub fn spawn_threaded_promise<F, T>(
        &mut self,
        name: impl Into<String>,
        f: F,
    ) -> anyhow::Result<()>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        let name = name.into();

        if self.pending_promises.contains_key(&name) {
            anyhow::bail!("there's already a promise {name:?} running!");
        }

        let f = move || Box::new(f()) as Box<dyn Any + Send>; // erase it
        let promise = Promise::spawn_thread(&name, f);

        self.pending_promises.insert(name, promise);

        Ok(())
    }

    /// Polls the promise with the given name.
    ///
    /// Returns `Some<T>` it it's ready, or `None` otherwise.
    ///
    /// Panics if `T` does not match the actual return value of the promise.
    pub fn poll_promise<T: Any>(&mut self, name: impl AsRef<str>) -> Option<T> {
        self.pending_promises
            .remove(name.as_ref())
            .and_then(|promise| match promise.try_take() {
                Ok(any) => Some(*any.downcast::<T>().unwrap()),
                Err(promise) => {
                    self.pending_promises
                        .insert(name.as_ref().to_owned(), promise);
                    None
                }
            })
    }

    /// Returns whether a promise with the given name is currently running.
    pub fn promise_exists(&mut self, name: impl AsRef<str>) -> bool {
        self.pending_promises.contains_key(name.as_ref())
    }

    fn check_keyboard_shortcuts(&mut self, egui_ctx: &egui::Context) {
        if let Some(cmd) = Command::listen_for_kb_shortcut(egui_ctx) {
            self.pending_commands.push(cmd);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn loop_selection(&self) -> Option<(re_data_store::Timeline, TimeRangeF)> {
        self.state
            .recording_configs
            .get(&self.state.selected_rec_id)
            // is there an active loop selection?
            .and_then(|rec_cfg| {
                rec_cfg
                    .time_ctrl
                    .loop_selection()
                    .map(|q| (*rec_cfg.time_ctrl.timeline(), q))
            })
    }

    fn run_pending_commands(&mut self, egui_ctx: &egui::Context, frame: &mut eframe::Frame) {
        let commands = self.pending_commands.drain(..).collect_vec();
        for cmd in commands {
            self.run_command(cmd, frame, egui_ctx);
        }
    }

    fn run_command(&mut self, cmd: Command, _frame: &mut eframe::Frame, egui_ctx: &egui::Context) {
        let is_narrow_screen = egui_ctx.screen_rect().width() < 600.0; // responsive ui for mobiles etc

        match cmd {
            #[cfg(not(target_arch = "wasm32"))]
            Command::Save => {
                save(self, None);
            }
            #[cfg(not(target_arch = "wasm32"))]
            Command::SaveSelection => {
                save(self, self.loop_selection());
            }
            #[cfg(not(target_arch = "wasm32"))]
            Command::Open => {
                open(self);
            }
            #[cfg(not(target_arch = "wasm32"))]
            Command::Quit => {
                self.state.depthai_state.shutdown();
                if let Some(backend_handle) = &mut self.backend_handle {
                    backend_handle.kill();
                }
                _frame.close();
            }

            Command::ResetViewer => {
                self.reset(egui_ctx);
            }

            #[cfg(not(target_arch = "wasm32"))]
            Command::OpenProfiler => {
                self.state.profiler.start();
            }

            Command::ToggleMemoryPanel => {
                self.memory_panel_open ^= true;
            }
            Command::ToggleBlueprintPanel => {
                let blueprint = self.blueprint_mut(egui_ctx);
                blueprint.blueprint_panel_expanded ^= true;

                // Only one of blueprint or selection panel can be open at a time on mobile:
                if is_narrow_screen && blueprint.blueprint_panel_expanded {
                    blueprint.selection_panel_expanded = false;
                }
            }
            Command::ToggleSelectionPanel => {
                let blueprint = self.blueprint_mut(egui_ctx);
                blueprint.selection_panel_expanded ^= true;

                // Only one of blueprint or selection panel can be open at a time on mobile:
                if is_narrow_screen && blueprint.selection_panel_expanded {
                    blueprint.blueprint_panel_expanded = false;
                }
            }
            Command::ToggleTimePanel => {
                self.blueprint_mut(egui_ctx).time_panel_expanded ^= true;
            }

            #[cfg(not(target_arch = "wasm32"))]
            Command::ToggleFullscreen => {
                _frame.set_fullscreen(!_frame.info().window_info.fullscreen);
            }
            #[cfg(not(target_arch = "wasm32"))]
            Command::ZoomIn => {
                self.state.app_options.zoom_factor += 0.1;
            }
            #[cfg(not(target_arch = "wasm32"))]
            Command::ZoomOut => {
                self.state.app_options.zoom_factor -= 0.1;
            }
            #[cfg(not(target_arch = "wasm32"))]
            Command::ZoomReset => {
                self.state.app_options.zoom_factor = 1.0;
            }

            Command::SelectionPrevious => {
                let state = &mut self.state;
                if let Some(rec_cfg) = state.recording_configs.get_mut(&state.selected_rec_id) {
                    rec_cfg.selection_state.select_previous();
                }
            }
            Command::SelectionNext => {
                let state = &mut self.state;
                if let Some(rec_cfg) = state.recording_configs.get_mut(&state.selected_rec_id) {
                    rec_cfg.selection_state.select_next();
                }
            }
            Command::ToggleCommandPalette => {
                self.cmd_palette.toggle();
            }

            Command::PlaybackTogglePlayPause => {
                self.run_time_control_command(TimeControlCommand::TogglePlayPause);
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
        }
    }

    fn run_time_control_command(&mut self, command: TimeControlCommand) {
        let rec_id = self.state.selected_rec_id;
        let Some(rec_cfg) = self.state.recording_configs.get_mut(&rec_id) else {return;};
        let time_ctrl = &mut rec_cfg.time_ctrl;

        let Some(log_db) = self.log_dbs.get(&rec_id) else { return };
        let times_per_timeline = log_db.times_per_timeline();

        match command {
            TimeControlCommand::TogglePlayPause => {
                time_ctrl.toggle_play_pause(times_per_timeline);
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

    fn selected_app_id(&self) -> ApplicationId {
        if let Some(log_db) = self.log_dbs.get(&self.state.selected_rec_id) {
            log_db
                .recording_info()
                .map_or_else(ApplicationId::unknown, |rec_info| {
                    rec_info.application_id.clone()
                })
        } else {
            ApplicationId::unknown()
        }
    }

    fn blueprint_mut(&mut self, egui_ctx: &egui::Context) -> &mut Blueprint {
        let selected_app_id = self.selected_app_id();
        self.state
            .blueprints
            .entry(selected_app_id)
            .or_insert_with(|| Blueprint::new(egui_ctx))
    }

    fn memory_panel_ui(
        &mut self,
        ui: &mut egui::Ui,
        gpu_resource_stats: &WgpuResourcePoolStatistics,
        store_config: &DataStoreConfig,
        store_stats: &DataStoreStats,
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
                    store_stats,
                );
            });
    }
}

impl eframe::App for App {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0; 4] // transparent so we can get rounded corners when doing [`re_ui::CUSTOM_WINDOW_DECORATIONS`]
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn on_close_event(&mut self) -> bool {
        self.state.depthai_state.shutdown();
        if let Some(backend_handle) = &mut self.backend_handle {
            backend_handle.kill();
        }
        true
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        if self.startup_options.persist_state {
            eframe::set_value(storage, eframe::APP_KEY, &self.state);
        }
    }

    fn update(&mut self, egui_ctx: &egui::Context, frame: &mut eframe::Frame) {
        let frame_start = Instant::now();
        self.state.depthai_state.update(); // Always update depthai state
        #[cfg(not(target_arch = "wasm32"))]
        {
            match &mut self.backend_handle {
                Some(handle) => match handle.try_wait() {
                    Ok(status) => {
                        if status.is_some() {
                            handle.kill();
                            re_log::debug!("Backend process has exited, restarting!");
                            self.backend_handle = App::spawn_backend();
                        }
                    }
                    Err(_) => {}
                },
                None => self.backend_handle = App::spawn_backend(),
            };
        }

        if self.backend_handle.is_none() {
            self.backend_handle = App::spawn_backend();
        };

        if self.startup_options.memory_limit.limit.is_none() {
            // we only warn about high memory usage if the user hasn't specified a limit
            self.ram_limit_warner.update();
        }

        if self.icon_status == AppIconStatus::NotSetTryAgain {
            self.icon_status = setup_app_icon();
        }

        if self.shutdown.load(std::sync::atomic::Ordering::Relaxed) {
            self.state.depthai_state.shutdown();
            #[cfg(not(target_arch = "wasm32"))]
            {
                if let Some(backend_handle) = &mut self.backend_handle {
                    backend_handle.kill();
                }
                frame.close();
            }
            return;
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            // Ensure zoom factor is sane and in 10% steps at all times before applying it.
            {
                let mut zoom_factor = self.state.app_options.zoom_factor;
                zoom_factor = zoom_factor.clamp(MIN_ZOOM_FACTOR, MAX_ZOOM_FACTOR);
                zoom_factor = (zoom_factor * 10.).round() / 10.;
                self.state.app_options.zoom_factor = zoom_factor;
            }

            // Apply zoom factor on top of natively reported pixel per point.
            let pixels_per_point = frame.info().native_pixels_per_point.unwrap_or(1.0)
                * self.state.app_options.zoom_factor;
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

        let store_config = self.log_db().entity_db.data_store.config().clone();
        let store_stats = DataStoreStats::from_store(&self.log_db().entity_db.data_store);

        // do first, before doing too many allocations
        self.memory_panel.update(&gpu_resource_stats, &store_stats);

        self.check_keyboard_shortcuts(egui_ctx);

        self.purge_memory_if_needed();

        self.state.cache.begin_frame();

        self.show_text_logs_as_notifications();
        self.receive_messages(egui_ctx);

        self.cleanup();

        file_saver_progress_ui(egui_ctx, self); // toasts for background file saver

        let mut main_panel_frame = egui::Frame::default();
        if re_ui::CUSTOM_WINDOW_DECORATIONS {
            // Add some margin so that we can later paint an outline around it all.
            main_panel_frame.inner_margin = 1.0.into();
        }
        egui::CentralPanel::default()
            .frame(main_panel_frame)
            .show(egui_ctx, |ui| {
                paint_background_fill(ui);

                warning_panel(&self.re_ui, ui, frame);

                top_panel(ui, frame, self, &gpu_resource_stats);

                self.memory_panel_ui(ui, &gpu_resource_stats, &store_config, &store_stats);

                let log_db = self.log_dbs.entry(self.state.selected_rec_id).or_default();
                let selected_app_id = log_db
                    .recording_info()
                    .map_or_else(ApplicationId::unknown, |rec_info| {
                        rec_info.application_id.clone()
                    });
                let blueprint = self
                    .state
                    .blueprints
                    .entry(selected_app_id)
                    .or_insert_with(|| Blueprint::new(egui_ctx));

                recording_config_entry(
                    &mut self.state.recording_configs,
                    self.state.selected_rec_id,
                    self.rx.source(),
                    log_db,
                )
                .selection_state
                .on_frame_start(blueprint);

                {
                    // TODO(andreas): store the re_renderer somewhere else.
                    let egui_renderer = {
                        let render_state = frame.wgpu_render_state().unwrap();
                        &mut render_state.renderer.write()
                    };
                    let render_ctx = egui_renderer
                        .paint_callback_resources
                        .get_mut::<re_renderer::RenderContext>()
                        .unwrap();
                    render_ctx.begin_frame();

                    self.state.show(
                        ui,
                        render_ctx,
                        log_db,
                        &self.re_ui,
                        &self.component_ui_registry,
                        self.rx.source(),
                    );

                    render_ctx.before_submit();
                }
            });

        if re_ui::CUSTOM_WINDOW_DECORATIONS {
            // Paint the main window frame on top of everything else
            paint_native_window_frame(egui_ctx);
        }

        self.handle_dropping_files(egui_ctx);
        self.toasts.show(egui_ctx);

        if let Some(cmd) = self.cmd_palette.show(egui_ctx) {
            self.pending_commands.push(cmd);
        }

        self.run_pending_commands(egui_ctx, frame);

        self.frame_time_history.add(
            egui_ctx.input(|i| i.time),
            frame_start.elapsed().as_secs_f32(),
        );
        egui_ctx.request_repaint(); // Force repaint even when out of focus
    }
}

fn paint_background_fill(ui: &mut egui::Ui) {
    // This is required because the streams view (time panel)
    // has rounded top corners, which leaves a gap.
    // So we fill in that gap (and other) here.
    // Of course this does some over-draw, but we have to live with that.

    ui.painter().rect_filled(
        ui.ctx().screen_rect().shrink(0.5),
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

fn wait_screen_ui(ui: &mut egui::Ui, rx: &Receiver<LogMsg>) {
    ui.centered_and_justified(|ui| {
        fn ready_and_waiting(ui: &mut egui::Ui, txt: &str) {
            let style = ui.style();
            let mut layout_job = egui::text::LayoutJob::default();
            layout_job.append(
                "Ready",
                0.0,
                egui::TextFormat::simple(
                    egui::TextStyle::Heading.resolve(style),
                    style.visuals.strong_text_color(),
                ),
            );
            layout_job.append(
                &format!("\n\n{txt}"),
                0.0,
                egui::TextFormat::simple(
                    egui::TextStyle::Body.resolve(style),
                    style.visuals.text_color(),
                ),
            );
            layout_job.halign = egui::Align::Center;
            ui.label(layout_job);
        }

        match rx.source() {
            re_smart_channel::Source::File { path } => {
                ui.strong(format!("Loading {}…", path.display()));
            }
            re_smart_channel::Source::RrdHttpStream { url } => {
                ui.strong(format!("Loading {url}…"));
            }
            re_smart_channel::Source::RrdWebEventListener => {
                ready_and_waiting(ui, "Waiting for logging data…");
            }
            re_smart_channel::Source::Sdk => {
                ready_and_waiting(ui, "Waiting for logging data from SDK");
            }
            re_smart_channel::Source::WsClient { ws_server_url } => {
                // TODO(emilk): it would be even better to know whether or not we are connected, or are attempting to connect
                ready_and_waiting(ui, &format!("Waiting for data from {ws_server_url}"));
            }
            re_smart_channel::Source::TcpServer { port } => {
                ready_and_waiting(ui, &format!("Listening on port {port}"));
            }
        };
    });
}

impl App {
    /// Show recent text log messages to the user as toast notifications.
    fn show_text_logs_as_notifications(&mut self) {
        crate::profile_function!();

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
        crate::profile_function!();

        let start = instant::Instant::now();

        while let Ok(msg) = self.rx.try_recv() {
            // All messages except [`LogMsg::GoodBye`] should have an associated recording id
            if let Some(recording_id) = msg.recording_id() {
                let is_new_recording = if let LogMsg::BeginRecordingMsg(msg) = &msg {
                    re_log::debug!("Opening a new recording: {:?}", msg.info);
                    self.state.selected_rec_id = msg.info.recording_id;
                    true
                } else {
                    false
                };

                let log_db = self.log_dbs.entry(*recording_id).or_default();

                if log_db.data_source.is_none() {
                    log_db.data_source = Some(self.rx.source().clone());
                }

                if let Err(err) = log_db.add(&msg) {
                    re_log::error!("Failed to add incoming msg: {err}");
                };

                if is_new_recording {
                    // Do analytics after ingesting the new message,
                    // because thats when the `log_db.recording_info` is set,
                    // which we use in the analytics call.
                    self.analytics.on_open_recording(log_db);
                }

                if start.elapsed() > instant::Duration::from_millis(10) {
                    egui_ctx.request_repaint(); // make sure we keep receiving messages asap
                    break; // don't block the main thread for too long
                }
            }
        }
    }

    fn cleanup(&mut self) {
        crate::profile_function!();

        self.log_dbs.retain(|_, log_db| !log_db.is_default());

        if !self.log_dbs.contains_key(&self.state.selected_rec_id) {
            self.state.selected_rec_id = self.log_dbs.keys().next().cloned().unwrap_or_default();
        }

        self.state
            .recording_configs
            .retain(|recording_id, _| self.log_dbs.contains_key(recording_id));

        if self.state.blueprints.len() > 100 {
            re_log::debug!("Pruning blueprints…");

            let used_app_ids: std::collections::HashSet<ApplicationId> = self
                .log_dbs
                .values()
                .filter_map(|log_db| {
                    log_db
                        .recording_info()
                        .map(|recording_info| recording_info.application_id.clone())
                })
                .collect();

            self.state
                .blueprints
                .retain(|application_id, _| used_app_ids.contains(application_id));
        }
    }

    fn purge_memory_if_needed(&mut self) {
        crate::profile_function!();

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
                crate::profile_scope!("pruning");
                if let Some(counted) = mem_use_before.counted {
                    re_log::debug!(
                        "Attempting to purge {:.1}% of used RAM ({})…",
                        100.0 * fraction_to_purge,
                        format_bytes(counted as f64 * fraction_to_purge as f64)
                    );
                }
                for log_db in self.log_dbs.values_mut() {
                    log_db.purge_fraction_of_ram(fraction_to_purge);
                }
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
        let selected_rec_id = self.state.selected_rec_id;

        self.state = Default::default();
        self.state.selected_rec_id = selected_rec_id;

        // Keep the style:
        let style = egui_ctx.style();
        egui_ctx.memory_mut(|mem| *mem = Default::default());
        egui_ctx.set_style((*style).clone());
    }

    /// Do we have an open `LogDb` that is non-empty?
    fn log_db_is_nonempty(&self) -> bool {
        self.log_dbs
            .get(&self.state.selected_rec_id)
            .map_or(false, |log_db| !log_db.is_default())
    }

    fn log_db(&mut self) -> &mut LogDb {
        self.log_dbs.entry(self.state.selected_rec_id).or_default()
    }

    fn show_log_db(&mut self, log_db: LogDb) {
        self.analytics.on_open_recording(&log_db);
        self.state.selected_rec_id = log_db.recording_id();
        self.log_dbs.insert(log_db.recording_id(), log_db);
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
                if let Some(log_db) = load_file_contents(&file.name, &mut bytes) {
                    self.show_log_db(log_db);

                    #[allow(clippy::needless_return)] // false positive on wasm32
                    return;
                }
            }

            #[cfg(not(target_arch = "wasm32"))]
            if let Some(path) = &file.path {
                if let Some(log_db) = load_file_path(path) {
                    self.show_log_db(log_db);
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

// ------------------------------------------------------------------------------------

#[derive(Copy, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
enum PanelSelection {
    #[default]
    Viewport,
}

#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
struct AppState {
    /// Global options for the whole viewer.
    app_options: AppOptions,

    /// Things that need caching.
    #[serde(skip)]
    cache: Caches,

    selected_rec_id: RecordingId,

    /// Configuration for the current recording (found in [`LogDb`]).
    recording_configs: IntMap<RecordingId, RecordingConfig>,

    #[serde(skip)] // Quick fix for subscriptions setting, just don't remembet space views
    blueprints: HashMap<ApplicationId, crate::ui::Blueprint>,

    /// Which view panel is currently being shown
    panel_selection: PanelSelection,

    selection_panel: crate::selection_panel::SelectionPanel,
    time_panel: crate::time_panel::TimePanel,

    selected_device: depthai::DeviceId,
    depthai_state: depthai::State,

    #[cfg(not(target_arch = "wasm32"))]
    #[serde(skip)]
    profiler: crate::Profiler,
}

impl AppState {
    #[allow(clippy::too_many_arguments)]
    fn show(
        &mut self,
        ui: &mut egui::Ui,
        render_ctx: &mut re_renderer::RenderContext,
        log_db: &LogDb,
        re_ui: &re_ui::ReUi,
        component_ui_registry: &ComponentUiRegistry,
        data_source: &re_smart_channel::Source,
    ) {
        crate::profile_function!();

        let Self {
            app_options: options,
            cache,
            selected_rec_id,
            recording_configs,
            panel_selection,
            blueprints,
            selection_panel,
            time_panel,
            selected_device,
            depthai_state,
            #[cfg(not(target_arch = "wasm32"))]
                profiler: _,
        } = self;

        let rec_cfg =
            recording_config_entry(recording_configs, *selected_rec_id, data_source, log_db);
        let selected_app_id = log_db
            .recording_info()
            .map_or_else(ApplicationId::unknown, |rec_info| {
                rec_info.application_id.clone()
            });

        let mut ctx = ViewerContext {
            app_options: options,
            cache,
            component_ui_registry,
            log_db,
            rec_cfg,
            re_ui,
            render_ctx,
            depthai_state,
        };

        let blueprint = blueprints
            .entry(selected_app_id.clone())
            .or_insert_with(|| Blueprint::new(ui.ctx()));
        // Hide time panel for now, reuse for recordings in the future
        // time_panel.show_panel(&mut ctx, blueprint, ui);
        selection_panel.show_panel(&mut ctx, ui, blueprint);

        let central_panel_frame = egui::Frame {
            fill: ui.style().visuals.panel_fill,
            inner_margin: egui::Margin::same(0.0),
            ..Default::default()
        };

        egui::CentralPanel::default()
            .frame(central_panel_frame)
            .show_inside(ui, |ui| match *panel_selection {
                PanelSelection::Viewport => blueprints
                    .entry(selected_app_id)
                    .or_insert_with(|| Blueprint::new(ui.ctx()))
                    .blueprint_panel_and_viewport(&mut ctx, ui),
            });

        // move time last, so we get to see the first data first!
        ctx.rec_cfg
            .time_ctrl
            .move_time(ui.ctx(), log_db.times_per_timeline());

        if WATERMARK {
            re_ui.paint_watermark();
        }
    }
}

fn warning_panel(re_ui: &re_ui::ReUi, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
    // We have not yet optimized the UI experience for mobile. Show a warning banner
    // with a link to the tracking issue.

    // Although this banner is applicable to IOS / Android generically without limit to web
    // There is a small issue in egui where Windows native currently reports as android.
    // TODO(jleibs): Remove the is_web gate once https://github.com/emilk/egui/pull/2832 has landed.
    if frame.is_web()
        && (ui.ctx().os() == egui::os::OperatingSystem::IOS
            || ui.ctx().os() == egui::os::OperatingSystem::Android)
    {
        let frame = egui::Frame {
            fill: ui.visuals().panel_fill,
            ..re_ui.bottom_panel_frame()
        };

        egui::TopBottomPanel::bottom("warning_panel")
            .resizable(false)
            .frame(frame)
            .show_inside(ui, |ui| {
                ui.centered_and_justified(|ui| {
                    let text =
                        re_ui.warning_text("Mobile OSes are not yet supported. Click for details.");
                    ui.hyperlink_to(text, "https://github.com/rerun-io/rerun/issues/1672");
                });
            });
    }
}

fn top_panel(
    ui: &mut egui::Ui,
    frame: &mut eframe::Frame,
    app: &mut App,
    gpu_resource_stats: &WgpuResourcePoolStatistics,
) {
    crate::profile_function!();

    let native_pixels_per_point = frame.info().native_pixels_per_point;
    let fullscreen = {
        #[cfg(target_arch = "wasm32")]
        {
            false
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            frame.info().window_info.fullscreen
        }
    };
    let top_bar_style = app.re_ui.top_bar_style(native_pixels_per_point, fullscreen);

    egui::TopBottomPanel::top("top_bar")
        .frame(app.re_ui.top_panel_frame())
        .exact_height(top_bar_style.height)
        .show_inside(ui, |ui| {
            let _response = egui::menu::bar(ui, |ui| {
                ui.set_height(top_bar_style.height);
                ui.add_space(top_bar_style.indent);

                top_bar_ui(ui, frame, app, gpu_resource_stats);
            })
            .response;

            #[cfg(not(target_arch = "wasm32"))]
            if !re_ui::NATIVE_WINDOW_BAR {
                let title_bar_response = _response.interact(egui::Sense::click());
                if title_bar_response.double_clicked() {
                    frame.set_maximized(!frame.info().window_info.maximized);
                } else if title_bar_response.is_pointer_button_down_on() {
                    frame.drag_window();
                }
            }
        });
}

fn rerun_menu_button_ui(ui: &mut egui::Ui, frame: &mut eframe::Frame, app: &mut App) {
    // let desired_icon_height = ui.max_rect().height() - 2.0 * ui.spacing_mut().button_padding.y;
    let desired_icon_height = ui.max_rect().height() - 4.0; // TODO(emilk): figure out this fudge
    let desired_icon_height = desired_icon_height.at_most(28.0); // figma size 2023-02-03

    let icon_image = app.re_ui.icon_image(&re_ui::icons::RERUN_MENU);
    let image_size = icon_image.size_vec2() * (desired_icon_height / icon_image.size_vec2().y);
    let texture_id = icon_image.texture_id(ui.ctx());

    ui.menu_image_button(texture_id, image_size, |ui| {
        ui.set_min_width(220.0);
        let spacing = 12.0;

        ui.menu_button("About", |ui| about_rerun_ui(ui, &app.build_info));

        main_view_selector_ui(ui, app);

        ui.add_space(spacing);

        Command::ToggleCommandPalette.menu_button_ui(ui, &mut app.pending_commands);

        ui.add_space(spacing);

        #[cfg(not(target_arch = "wasm32"))]
        {
            Command::Open.menu_button_ui(ui, &mut app.pending_commands);

            save_buttons_ui(ui, app);

            ui.add_space(spacing);

            // On the web the browser controls the zoom
            let zoom_factor = app.state.app_options.zoom_factor;
            ui.weak(format!("Zoom {:.0}%", zoom_factor * 100.0))
                .on_hover_text("The zoom factor applied on top of the OS scaling factor.");
            Command::ZoomIn.menu_button_ui(ui, &mut app.pending_commands);
            Command::ZoomOut.menu_button_ui(ui, &mut app.pending_commands);
            ui.add_enabled_ui(zoom_factor != 1.0, |ui| {
                Command::ZoomReset.menu_button_ui(ui, &mut app.pending_commands)
            });

            Command::ToggleFullscreen.menu_button_ui(ui, &mut app.pending_commands);

            ui.add_space(spacing);
        }

        {
            Command::ResetViewer.menu_button_ui(ui, &mut app.pending_commands);

            #[cfg(not(target_arch = "wasm32"))]
            Command::OpenProfiler.menu_button_ui(ui, &mut app.pending_commands);

            Command::ToggleMemoryPanel.menu_button_ui(ui, &mut app.pending_commands);
        }

        ui.add_space(spacing);

        ui.menu_button("Recordings", |ui| {
            recordings_menu(ui, app);
        });

        ui.menu_button("Options", |ui| {
            options_menu_ui(ui, frame, &mut app.state.app_options);
        });

        ui.add_space(spacing);
        ui.hyperlink_to(
            "Help",
            "https://www.rerun.io/docs/getting-started/viewer-walkthrough",
        );

        #[cfg(not(target_arch = "wasm32"))]
        {
            ui.add_space(spacing);
            Command::Quit.menu_button_ui(ui, &mut app.pending_commands);
        }
    });
}

fn about_rerun_ui(ui: &mut egui::Ui, build_info: &re_build_info::BuildInfo) {
    let re_build_info::BuildInfo {
        crate_name,
        version,
        rustc_version,
        llvm_version,
        git_hash,
        git_branch: _,
        is_in_rerun_workspace: _,
        target_triple,
        datetime,
    } = *build_info;

    ui.style_mut().wrap = Some(false);

    let rustc_version = if rustc_version.is_empty() {
        "unknown"
    } else {
        rustc_version
    };

    let llvm_version = if llvm_version.is_empty() {
        "unknown"
    } else {
        llvm_version
    };

    let short_git_hash = &git_hash[..std::cmp::min(git_hash.len(), 7)];

    ui.label(format!(
        "{crate_name} {version} ({short_git_hash})\n\
        {target_triple}\n\
        rustc {rustc_version}\n\
        LLVM {llvm_version}\n\
        Built {datetime}",
    ));

    ui.add_space(12.0);
    ui.hyperlink_to("www.rerun.io", "https://www.rerun.io/");
}

fn top_bar_ui(
    ui: &mut egui::Ui,
    frame: &mut eframe::Frame,
    app: &mut App,
    gpu_resource_stats: &WgpuResourcePoolStatistics,
) {
    rerun_menu_button_ui(ui, frame, app);

    if app.state.app_options.show_metrics {
        ui.separator();
        frame_time_label_ui(ui, app);
        memory_use_label_ui(ui, gpu_resource_stats);
        input_latency_label_ui(ui, app);
    }

    if let Some(log_db) = app.log_dbs.get(&app.state.selected_rec_id) {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let selected_app_id = log_db
                .recording_info()
                .map_or_else(ApplicationId::unknown, |rec_info| {
                    rec_info.application_id.clone()
                });

            let blueprint = app
                .state
                .blueprints
                .entry(selected_app_id)
                .or_insert_with(|| Blueprint::new(ui.ctx()));

            // From right-to-left:

            if re_ui::CUSTOM_WINDOW_DECORATIONS && !cfg!(target_arch = "wasm32") {
                ui.add_space(8.0);
                #[cfg(not(target_arch = "wasm32"))]
                re_ui::native_window_buttons_ui(frame, ui);
                ui.separator();
            } else {
                // Make the first button the same distance form the side as from the top,
                // no matter how high the top bar is.
                let extra_margin = (ui.available_height() - 24.0) / 2.0;
                ui.add_space(extra_margin);
            }

            let mut selection_panel_expanded = blueprint.selection_panel_expanded;
            if app
                .re_ui
                .medium_icon_toggle_button(
                    ui,
                    &re_ui::icons::RIGHT_PANEL_TOGGLE,
                    &mut selection_panel_expanded,
                )
                .on_hover_text(format!(
                    "Toggle Selection View{}",
                    Command::ToggleSelectionPanel.format_shortcut_tooltip_suffix(ui.ctx())
                ))
                .clicked()
            {
                app.pending_commands.push(Command::ToggleSelectionPanel);
            }

            let mut time_panel_expanded = blueprint.time_panel_expanded;
            if app
                .re_ui
                .medium_icon_toggle_button(
                    ui,
                    &re_ui::icons::BOTTOM_PANEL_TOGGLE,
                    &mut time_panel_expanded,
                )
                .on_hover_text(format!(
                    "Toggle Timeline View{}",
                    Command::ToggleTimePanel.format_shortcut_tooltip_suffix(ui.ctx())
                ))
                .clicked()
            {
                app.pending_commands.push(Command::ToggleTimePanel);
            }

            let mut blueprint_panel_expanded = blueprint.blueprint_panel_expanded;
            if app
                .re_ui
                .medium_icon_toggle_button(
                    ui,
                    &re_ui::icons::LEFT_PANEL_TOGGLE,
                    &mut blueprint_panel_expanded,
                )
                .on_hover_text(format!(
                    "Toggle Blueprint View{}",
                    Command::ToggleBlueprintPanel.format_shortcut_tooltip_suffix(ui.ctx())
                ))
                .clicked()
            {
                app.pending_commands.push(Command::ToggleBlueprintPanel);
            }

            if cfg!(debug_assertions) && app.state.app_options.show_metrics {
                ui.vertical_centered(|ui| {
                    ui.style_mut().wrap = Some(false);
                    ui.add_space(6.0); // TODO(emilk): in egui, add a proper way of centering a single widget in a UI.
                    egui::warn_if_debug_build(ui);
                });
            }
        });
    }
}

fn frame_time_label_ui(ui: &mut egui::Ui, app: &mut App) {
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
            .on_hover_text("CPU time used by Depthai Viewer each frame. Lower is better.");
    }
}

fn memory_use_label_ui(ui: &mut egui::Ui, gpu_resource_stats: &WgpuResourcePoolStatistics) {
    if let Some(count) = re_memory::accounting_allocator::global_allocs() {
        // we use monospace so the width doesn't fluctuate as the numbers change.

        let bytes_used_text = re_format::format_bytes(count.size as _);
        ui.label(
            egui::RichText::new(&bytes_used_text)
                .monospace()
                .color(ui.visuals().weak_text_color()),
        )
        .on_hover_text(format!(
            "Depthai Viewer is using {} of RAM in {} separate allocations,\n\
            plus {} of GPU memory in {} textures and {} buffers.",
            bytes_used_text,
            format_number(count.count),
            re_format::format_bytes(gpu_resource_stats.total_bytes() as _),
            format_number(gpu_resource_stats.num_textures),
            format_number(gpu_resource_stats.num_buffers),
        ));
    }
}

fn input_latency_label_ui(ui: &mut egui::Ui, app: &mut App) {
    // TODO(emilk): it would be nice to know if the network stream is still open
    let is_latency_interesting = app.rx.source().is_network();

    let queue_len = app.rx.len();

    // empty queue == unreliable latency
    let latency_sec = app.rx.latency_ns() as f32 / 1e9;
    if queue_len > 0
        && (!is_latency_interesting || app.state.app_options.warn_latency < latency_sec)
    {
        // we use this to avoid flicker
        app.latest_queue_interest = instant::Instant::now();
    }

    if app.latest_queue_interest.elapsed().as_secs_f32() < 1.0 {
        ui.separator();
        if is_latency_interesting {
            let text = format!(
                "Latency: {:.2}s, queue: {}",
                latency_sec,
                format_number(queue_len),
            );
            let hover_text =
                    "When more data is arriving over network than the Depthai Viewer can index, a queue starts building up, leading to latency and increased RAM use.\n\
                    This latency does NOT include network latency.";

            if latency_sec < app.state.app_options.warn_latency {
                ui.weak(text).on_hover_text(hover_text);
            } else {
                ui.label(app.re_ui.warning_text(text))
                    .on_hover_text(hover_text);
            }
        } else {
            ui.weak(format!("Queue: {}", format_number(queue_len)))
                .on_hover_text("Number of messages in the inbound queue");
        }
    }
}

// ----------------------------------------------------------------------------

const FILE_SAVER_PROMISE: &str = "file_saver";

fn file_saver_progress_ui(egui_ctx: &egui::Context, app: &mut App) {
    use std::path::PathBuf;

    let file_save_in_progress = app.promise_exists(FILE_SAVER_PROMISE);
    if file_save_in_progress {
        // There's already a file save running in the background.

        if let Some(res) = app.poll_promise::<anyhow::Result<PathBuf>>(FILE_SAVER_PROMISE) {
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

// TODO(emilk): support saving data on web
#[cfg(not(target_arch = "wasm32"))]
fn save_buttons_ui(ui: &mut egui::Ui, app: &mut App) {
    let file_save_in_progress = app.promise_exists(FILE_SAVER_PROMISE);

    let save_button = Command::Save.menu_button(ui.ctx());
    let save_selection_button = Command::SaveSelection.menu_button(ui.ctx());

    if file_save_in_progress {
        ui.add_enabled_ui(false, |ui| {
            ui.horizontal(|ui| {
                ui.add(save_button);
                ui.spinner();
            });
            ui.horizontal(|ui| {
                ui.add(save_selection_button);
                ui.spinner();
            });
        });
    } else {
        ui.add_enabled_ui(app.log_db_is_nonempty(), |ui| {
            if ui
                .add(save_button)
                .on_hover_text("Save all data to a Rerun data file (.rrd)")
                .clicked()
            {
                ui.close_menu();
                app.pending_commands.push(Command::Save);
            }

            // We need to know the loop selection _before_ we can even display the
            // button, as this will determine whether its grayed out or not!
            // TODO(cmc): In practice the loop (green) selection is always there
            // at the moment so...
            let loop_selection = app.loop_selection();

            if ui
                .add_enabled(loop_selection.is_some(), save_selection_button)
                .on_hover_text(
                    "Save data for the current loop selection to a Rerun data file (.rrd)",
                )
                .clicked()
            {
                ui.close_menu();
                app.pending_commands.push(Command::SaveSelection);
            }
        });
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn open(app: &mut App) {
    if let Some(path) = rfd::FileDialog::new()
        .add_filter("rerun data file", &["rrd"])
        .pick_file()
    {
        if let Some(log_db) = load_file_path(&path) {
            app.show_log_db(log_db);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn save(app: &mut App, loop_selection: Option<(re_data_store::Timeline, TimeRangeF)>) {
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
        let f = match save_database_to_file(app.log_db(), path, loop_selection) {
            Ok(f) => f,
            Err(err) => {
                re_log::error!("File saving failed: {err}");
                return;
            }
        };
        if let Err(err) = app.spawn_threaded_promise(FILE_SAVER_PROMISE, f) {
            // NOTE: Shouldn't even be possible as the "Save" button is already
            // grayed out at this point... better safe than sorry though.
            re_log::error!("File saving failed: {err}");
        }
    }
}

fn main_view_selector_ui(ui: &mut egui::Ui, app: &mut App) {
    if app.log_db_is_nonempty() {
        ui.horizontal(|ui| {
            ui.label("Main view:");
            if ui
                .selectable_value(
                    &mut app.state.panel_selection,
                    PanelSelection::Viewport,
                    "Viewport",
                )
                .clicked()
            {
                ui.close_menu();
            }
        });
    }
}

fn recordings_menu(ui: &mut egui::Ui, app: &mut App) {
    let log_dbs = app
        .log_dbs
        .values()
        .sorted_by_key(|log_db| log_db.recording_info().map(|ri| ri.started))
        .collect_vec();

    if log_dbs.is_empty() {
        ui.weak("(empty)");
        return;
    }

    ui.style_mut().wrap = Some(false);
    for log_db in log_dbs {
        let info = if let Some(rec_info) = log_db.recording_info() {
            format!(
                "{} - {}",
                rec_info.application_id,
                rec_info.started.format()
            )
        } else {
            "<UNKNOWN>".to_owned()
        };
        if ui
            .radio(app.state.selected_rec_id == log_db.recording_id(), info)
            .clicked()
        {
            app.state.selected_rec_id = log_db.recording_id();
        }
    }
}

fn options_menu_ui(ui: &mut egui::Ui, _frame: &mut eframe::Frame, options: &mut AppOptions) {
    ui.style_mut().wrap = Some(false);

    if ui
        .checkbox(&mut options.show_metrics, "Show performance metrics")
        .on_hover_text("Show metrics for milliseconds/frame and RAM usage in the top bar.")
        .clicked()
    {
        ui.close_menu();
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        if ui
            .checkbox(&mut options.experimental_space_view_screenshots, "(experimental) Space View screenshots")
            .on_hover_text("Allow taking screenshots of 2D & 3D space views via their context menu. Does not contain labels.")
            .clicked()
        {
            ui.close_menu();
        }
    }

    #[cfg(debug_assertions)]
    {
        ui.separator();
        ui.label("Debug:");

        egui_debug_options_ui(ui);
        ui.separator();
        debug_menu_options_ui(ui, options, _frame);
    }
}

#[cfg(debug_assertions)]
fn egui_debug_options_ui(ui: &mut egui::Ui) {
    let mut debug = ui.style().debug;
    let mut any_clicked = false;

    any_clicked |= ui
        .checkbox(&mut debug.debug_on_hover, "Ui debug on hover")
        .on_hover_text("However over widgets to see their rectangles")
        .changed();
    any_clicked |= ui
        .checkbox(&mut debug.show_expand_width, "Show expand width")
        .on_hover_text("Show which widgets make their parent wider")
        .changed();
    any_clicked |= ui
        .checkbox(&mut debug.show_expand_height, "Show expand height")
        .on_hover_text("Show which widgets make their parent higher")
        .changed();
    any_clicked |= ui.checkbox(&mut debug.show_resize, "Show resize").changed();
    any_clicked |= ui
        .checkbox(
            &mut debug.show_interactive_widgets,
            "Show interactive widgets",
        )
        .on_hover_text("Show an overlay on all interactive widgets.")
        .changed();

    // This option currently causes the viewer to hang.
    // any_clicked |= ui
    //     .checkbox(&mut debug.show_blocking_widget, "Show blocking widgets")
    //     .on_hover_text("Show what widget blocks the interaction of another widget.")
    //     .changed();

    if any_clicked {
        let mut style = (*ui.ctx().style()).clone();
        style.debug = debug;
        ui.ctx().set_style(style);
    }
}

#[cfg(debug_assertions)]
fn debug_menu_options_ui(ui: &mut egui::Ui, options: &mut AppOptions, _frame: &mut eframe::Frame) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if ui.button("Mobile size").clicked() {
            // frame.set_window_size(egui::vec2(375.0, 812.0)); // iPhone 12 mini
            _frame.set_window_size(egui::vec2(375.0, 667.0)); //  iPhone SE 2nd gen
            _frame.set_fullscreen(false);
            ui.close_menu();
        }
        ui.separator();
    }

    if ui.button("Log info").clicked() {
        re_log::info!("Logging some info");
    }

    ui.checkbox(
        &mut options.show_picking_debug_overlay,
        "Picking Debug Overlay",
    )
    .on_hover_text("Show a debug overlay that renders the picking layer information using the `debug_overlay.wgsl` shader.");

    ui.menu_button("Crash", |ui| {
        #[allow(clippy::manual_assert)]
        if ui.button("panic!").clicked() {
            panic!("Intentional panic");
        }

        if ui.button("panic! during unwind").clicked() {
            struct PanicOnDrop {}

            impl Drop for PanicOnDrop {
                fn drop(&mut self) {
                    panic!("Second intentional panic in Drop::drop");
                }
            }

            let _this_will_panic_when_dropped = PanicOnDrop {};
            panic!("First intentional panic");
        }

        if ui.button("SEGFAULT").clicked() {
            // Taken from https://github.com/EmbarkStudios/crash-handling/blob/main/sadness-generator/src/lib.rs

            /// This is the fixed address used to generate a segfault. It's possible that
            /// this address can be mapped and writable by the your process in which case a
            /// crash may not occur
            #[cfg(target_pointer_width = "64")]
            pub const SEGFAULT_ADDRESS: u64 = u32::MAX as u64 + 0x42;
            #[cfg(target_pointer_width = "32")]
            pub const SEGFAULT_ADDRESS: u32 = 0x42;

            let bad_ptr: *mut u8 = SEGFAULT_ADDRESS as _;
            #[allow(unsafe_code)]
            // SAFETY: this is not safe. We are _trying_ to crash.
            unsafe {
                std::ptr::write_volatile(bad_ptr, 1);
            }
        }

        if ui.button("Stack overflow").clicked() {
            // Taken from https://github.com/EmbarkStudios/crash-handling/blob/main/sadness-generator/src/lib.rs
            fn recurse(data: u64) -> u64 {
                let mut buff = [0u8; 256];
                buff[..9].copy_from_slice(b"junk data");

                let mut result = data;
                for c in buff {
                    result += c as u64;
                }

                if result == 0 {
                    result
                } else {
                    recurse(result) + 1
                }
            }

            recurse(42);
        }
    });
}

// ---

/// Returns a closure that, when run, will save the contents of the current database
/// to disk, at the specified `path`.
///
/// If `time_selection` is specified, then only data for that specific timeline over that
/// specific time range will be accounted for.
#[cfg(not(target_arch = "wasm32"))]
fn save_database_to_file(
    log_db: &LogDb,
    path: std::path::PathBuf,
    time_selection: Option<(re_data_store::Timeline, TimeRangeF)>,
) -> anyhow::Result<impl FnOnce() -> anyhow::Result<std::path::PathBuf>> {
    use re_arrow_store::TimeRange;

    crate::profile_scope!("dump_messages");

    let begin_rec_msg = log_db
        .recording_msg()
        .map(|msg| LogMsg::BeginRecordingMsg(msg.clone()));

    let ent_op_msgs = log_db
        .iter_entity_op_msgs()
        .map(|msg| LogMsg::EntityPathOpMsg(log_db.recording_id(), msg.clone()))
        .collect_vec();

    let time_filter = time_selection.map(|(timeline, range)| {
        (
            timeline,
            TimeRange::new(range.min.floor(), range.max.ceil()),
        )
    });
    let data_msgs: Result<Vec<_>, _> = log_db
        .entity_db
        .data_store
        .to_data_tables(time_filter)
        .map(|table| {
            table
                .to_arrow_msg()
                .map(|msg| LogMsg::ArrowMsg(log_db.recording_id(), msg))
        })
        .collect();

    use anyhow::Context as _;
    let data_msgs = data_msgs.with_context(|| "Failed to export to data tables")?;

    let msgs = std::iter::once(begin_rec_msg)
        .flatten() // option
        .chain(ent_op_msgs)
        .chain(data_msgs);

    Ok(move || {
        crate::profile_scope!("save_to_file");

        use anyhow::Context as _;
        let file = std::fs::File::create(path.as_path())
            .with_context(|| format!("Failed to create file at {path:?}"))?;

        re_log_encoding::encoder::encode_owned(msgs, file)
            .map(|_| path)
            .context("Message encode")
    })
}

#[allow(unused_mut)]
fn load_rrd_to_log_db(mut read: impl std::io::Read) -> anyhow::Result<LogDb> {
    crate::profile_function!();

    let decoder = re_log_encoding::decoder::Decoder::new(read)?;

    let mut log_db = LogDb::default();
    for msg in decoder {
        log_db.add(&msg?)?;
    }
    Ok(log_db)
}

#[cfg(not(target_arch = "wasm32"))]
#[must_use]
fn load_file_path(path: &std::path::Path) -> Option<LogDb> {
    fn load_file_path_impl(path: &std::path::Path) -> anyhow::Result<LogDb> {
        crate::profile_function!();
        use anyhow::Context as _;
        let file = std::fs::File::open(path).context("Failed to open file")?;
        load_rrd_to_log_db(file)
    }

    re_log::info!("Loading {path:?}…");

    match load_file_path_impl(path) {
        Ok(mut new_log_db) => {
            re_log::info!("Loaded {path:?}");
            new_log_db.data_source = Some(re_smart_channel::Source::File { path: path.into() });
            Some(new_log_db)
        }
        Err(err) => {
            let msg = format!("Failed loading {path:?}: {}", re_error::format(&err));
            re_log::error!("{msg}");
            rfd::MessageDialog::new()
                .set_level(rfd::MessageLevel::Error)
                .set_description(&msg)
                .show();
            None
        }
    }
}

#[must_use]
fn load_file_contents(name: &str, read: impl std::io::Read) -> Option<LogDb> {
    match load_rrd_to_log_db(read) {
        Ok(mut log_db) => {
            re_log::info!("Loaded {name:?}");
            log_db.data_source = Some(re_smart_channel::Source::File { path: name.into() });
            Some(log_db)
        }
        Err(err) => {
            let msg = format!("Failed loading {name:?}: {}", re_error::format(&err));
            re_log::error!("{msg}");
            rfd::MessageDialog::new()
                .set_level(rfd::MessageLevel::Error)
                .set_description(&msg)
                .show();
            None
        }
    }
}

fn recording_config_entry<'cfgs>(
    configs: &'cfgs mut IntMap<RecordingId, RecordingConfig>,
    id: RecordingId,
    data_source: &'_ re_smart_channel::Source,
    log_db: &'_ LogDb,
) -> &'cfgs mut RecordingConfig {
    configs
        .entry(id)
        .or_insert_with(|| new_recording_confg(data_source, log_db))
}

fn new_recording_confg(
    data_source: &'_ re_smart_channel::Source,
    log_db: &'_ LogDb,
) -> RecordingConfig {
    use crate::misc::time_control::PlayState;

    let play_state = match data_source {
        // Play files from the start by default - it feels nice and alive./
        // RrdHttpStream downloads the whole file before decoding it, so we treat it the same as a file.
        re_smart_channel::Source::File { .. }
        | re_smart_channel::Source::RrdHttpStream { .. }
        | re_smart_channel::Source::RrdWebEventListener => PlayState::Playing,

        // Live data - follow it!
        re_smart_channel::Source::Sdk
        | re_smart_channel::Source::WsClient { .. }
        | re_smart_channel::Source::TcpServer { .. } => PlayState::Following,
    };

    let mut rec_cfg = RecordingConfig::default();

    rec_cfg
        .time_ctrl
        .set_play_state(log_db.times_per_timeline(), play_state);

    rec_cfg
}
