use std::{any::Any, hash::Hash};

use ahash::HashMap;
use egui::NumExt as _;
use egui_notify::Toasts;
use instant::Instant;
use itertools::Itertools as _;
use nohash_hasher::IntMap;
use poll_promise::Promise;

use re_arrow_store::DataStoreStats;
use re_data_store::log_db::LogDb;
use re_format::format_number;
use re_log_types::{ApplicationId, LogMsg, RecordingId};
use re_renderer::WgpuResourcePoolStatistics;
use re_smart_channel::Receiver;
use re_ui::Command;

use crate::{
    misc::{AppOptions, Caches, RecordingConfig, ViewerContext},
    ui::{data_ui::ComponentUiRegistry, Blueprint},
};

#[cfg(not(target_arch = "wasm32"))]
use re_log_types::TimeRangeF;

const WATERMARK: bool = false; // Nice for recording media material

// ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum TimeControlCommand {
    TogglePlayPause,
    StepBack,
    StepForward,
}

// ----------------------------------------------------------------------------

/// Settings set once at startup (e.g. via command-line options) and not serialized.
#[derive(Clone, Copy, Default)]
pub struct StartupOptions {
    pub memory_limit: re_memory::MemoryLimit,
}

// ----------------------------------------------------------------------------

/// The Rerun viewer as an [`eframe`] application.
pub struct App {
    startup_options: StartupOptions,
    re_ui: re_ui::ReUi,

    component_ui_registry: ComponentUiRegistry,

    rx: Option<Receiver<LogMsg>>,

    /// Where the logs are stored.
    log_dbs: IntMap<RecordingId, LogDb>,

    /// What is serialized
    state: AppState,

    /// Set to `true` on Ctrl-C.
    #[cfg(not(target_arch = "wasm32"))]
    ctrl_c: std::sync::Arc<std::sync::atomic::AtomicBool>,

    /// Pending background tasks, using `poll_promise`.
    pending_promises: HashMap<String, Promise<Box<dyn Any + Send>>>,

    /// Toast notifications, using `egui-notify`.
    toasts: Toasts,

    latest_memory_purge: instant::Instant,
    memory_panel: crate::memory_panel::MemoryPanel,
    memory_panel_open: bool,

    latest_queue_interest: instant::Instant,

    /// Measures how long a frame takes to paint
    frame_time_history: egui::util::History<f32>,

    /// Commands to run at the end of the frame.
    pending_commands: Vec<Command>,
    cmd_palette: re_ui::CommandPalette,

    // NOTE: Optional because it is possible to have the `analytics` feature flag enabled while at
    // the same time opting out of analytics at run-time.
    #[cfg(all(not(target_arch = "wasm32"), feature = "analytics"))]
    analytics: Option<re_analytics::Analytics>,
}

impl App {
    /// Create a viewer that receives new log messages over time
    pub fn from_receiver(
        startup_options: StartupOptions,
        re_ui: re_ui::ReUi,
        storage: Option<&dyn eframe::Storage>,
        rx: Receiver<LogMsg>,
    ) -> Self {
        Self::new(
            startup_options,
            re_ui,
            storage,
            Some(rx),
            Default::default(),
        )
    }

    fn new(
        startup_options: StartupOptions,
        re_ui: re_ui::ReUi,
        storage: Option<&dyn eframe::Storage>,
        rx: Option<Receiver<LogMsg>>,
        log_db: LogDb,
    ) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        let ctrl_c = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

        #[cfg(not(target_arch = "wasm32"))]
        {
            // Close viewer on Ctrl-C. TODO(emilk): maybe add to `eframe`?

            let ctrl_c = ctrl_c.clone();
            let egui_ctx = re_ui.egui_ctx.clone();

            ctrlc::set_handler(move || {
                re_log::debug!("Ctrl-C detected - Closing viewer.");
                ctrl_c.store(true, std::sync::atomic::Ordering::SeqCst);
                egui_ctx.request_repaint(); // so that we notice that we should close
            })
            .expect("Error setting Ctrl-C handler");
        }

        let mut state: AppState = storage
            .and_then(|storage| eframe::get_value(storage, eframe::APP_KEY))
            .unwrap_or_default();

        let mut log_dbs = IntMap::default();
        if !log_db.is_empty() {
            state.selected_rec_id = log_db.recording_id();
            log_dbs.insert(log_db.recording_id(), log_db);
        }

        #[cfg(all(not(target_arch = "wasm32"), feature = "analytics"))]
        let analytics = match re_analytics::Analytics::new(std::time::Duration::from_secs(2)) {
            Ok(analytics) => {
                analytics.record(re_analytics::Event::viewer_started());
                Some(analytics)
            }
            Err(err) => {
                re_log::error!(%err, "failed to initialize analytics SDK");
                None
            }
        };

        Self {
            startup_options,
            re_ui,
            component_ui_registry: Default::default(),
            rx,
            log_dbs,
            state,
            #[cfg(not(target_arch = "wasm32"))]
            ctrl_c,
            pending_promises: Default::default(),
            toasts: Toasts::new(),
            latest_memory_purge: instant::Instant::now(), // TODO(emilk): `Instant::MIN` when we have our own `Instant` that supports it.
            memory_panel: Default::default(),
            memory_panel_open: false,

            latest_queue_interest: instant::Instant::now(), // TODO(emilk): `Instant::MIN` when we have our own `Instant` that supports it.

            frame_time_history: egui::util::History::new(1..100, 0.5),

            pending_commands: Default::default(),
            cmd_palette: Default::default(),

            #[cfg(all(not(target_arch = "wasm32"), feature = "analytics"))]
            analytics,
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

    fn check_keyboard_shortcuts(&mut self, egui_ctx: &egui::Context, frame: &mut eframe::Frame) {
        if let Some(cmd) = Command::listen_for_kb_shortcut(egui_ctx) {
            self.pending_commands.push(cmd);
        }

        if !frame.is_web() {
            egui::gui_zoom::zoom_with_keyboard_shortcuts(
                egui_ctx,
                frame.info().native_pixels_per_point,
            );
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
                self.blueprint_mut().blueprint_panel_expanded ^= true;
            }
            Command::ToggleSelectionPanel => {
                self.blueprint_mut().selection_panel_expanded ^= true;
            }
            Command::ToggleTimePanel => {
                self.blueprint_mut().time_panel_expanded ^= true;
            }

            #[cfg(not(target_arch = "wasm32"))]
            Command::ToggleFullscreen => {
                _frame.set_fullscreen(!_frame.info().window_info.fullscreen);
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
        }
    }

    fn selected_app_id(&mut self) -> ApplicationId {
        let log_db = self.log_dbs.entry(self.state.selected_rec_id).or_default();
        let selected_app_id = log_db
            .recording_info()
            .map_or_else(ApplicationId::unknown, |rec_info| {
                rec_info.application_id.clone()
            });
        selected_app_id
    }

    fn blueprint_mut(&mut self) -> &mut Blueprint {
        let selected_app_id = self.selected_app_id();
        self.state.blueprints.entry(selected_app_id).or_default()
    }

    fn memory_panel_ui(
        &mut self,
        ui: &mut egui::Ui,
        gpu_resource_stats: &WgpuResourcePoolStatistics,
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
                    store_stats,
                );
            });
    }
}

impl eframe::App for App {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0; 4] // transparent so we can get rounded corners when doing [`re_ui::CUSTOM_WINDOW_DECORATIONS`]
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.state);
    }

    fn update(&mut self, egui_ctx: &egui::Context, frame: &mut eframe::Frame) {
        let frame_start = Instant::now();

        #[cfg(not(target_arch = "wasm32"))]
        if self.ctrl_c.load(std::sync::atomic::Ordering::Relaxed) {
            frame.close();
            return;
        }

        let gpu_resource_stats = {
            // TODO(andreas): store the re_renderer somewhere else.
            let egui_renderer = {
                let render_state = frame.wgpu_render_state().unwrap();
                &mut render_state.renderer.read()
            };
            let render_ctx = egui_renderer
                .paint_callback_resources
                .get::<re_renderer::RenderContext>()
                .unwrap();
            // Query statistics before frame_maintenance as this might be more accurate if there's resources that we recreate every frame.
            render_ctx.gpu_resources.statistics()
        };

        let store_stats = DataStoreStats::from_store(&self.log_db().entity_db.data_store);

        self.memory_panel.update(&gpu_resource_stats, &store_stats); // do first, before doing too many allocations

        self.check_keyboard_shortcuts(egui_ctx, frame);

        self.purge_memory_if_needed();

        self.state.cache.new_frame();

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

                top_panel(ui, frame, self, &gpu_resource_stats);

                self.memory_panel_ui(ui, &gpu_resource_stats, &store_stats);

                let log_db = self.log_dbs.entry(self.state.selected_rec_id).or_default();
                let selected_app_id = log_db
                    .recording_info()
                    .map_or_else(ApplicationId::unknown, |rec_info| {
                        rec_info.application_id.clone()
                    });
                let blueprint = self.state.blueprints.entry(selected_app_id).or_default();

                self.state
                    .recording_configs
                    .entry(self.state.selected_rec_id)
                    .or_default()
                    .selection_state
                    .on_frame_start(log_db, blueprint);

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
                    render_ctx.frame_maintenance();

                    if let (true, Some(rx)) = (log_db.is_empty(), &self.rx) {
                        wait_screen_ui(ui, rx);
                    } else {
                        self.state.show(
                            ui,
                            render_ctx,
                            log_db,
                            &self.re_ui,
                            &self.component_ui_registry,
                        );
                    }
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
            re_smart_channel::Source::Sdk => {
                ready_and_waiting(ui, "Waiting for logging data from SDK");
            }
            re_smart_channel::Source::WsClient { ws_server_url } => {
                // TODO(emilk): it would be even better to know wether or not we are connected, or are attempting to connect
                ready_and_waiting(ui, &format!("Waiting for data from {ws_server_url}"));
            }
            re_smart_channel::Source::TcpServer { port } => {
                ready_and_waiting(ui, &format!("Listening on port {port}"));
            }
        };
    });
}

impl App {
    fn receive_messages(&mut self, egui_ctx: &egui::Context) {
        if let Some(rx) = &mut self.rx {
            crate::profile_function!();
            let start = instant::Instant::now();

            while let Ok(msg) = rx.try_recv() {
                if let LogMsg::BeginRecordingMsg(msg) = &msg {
                    re_log::debug!("Beginning a new recording: {:?}", msg.info);
                    self.state.selected_rec_id = msg.info.recording_id;

                    #[cfg(all(not(target_arch = "wasm32"), feature = "analytics"))]
                    if let Some(analytics) = self.analytics.as_mut() {
                        use re_analytics::Property;
                        analytics.default_append_props_mut().extend([
                            ("application_id".into(), {
                                let prop: Property = msg.info.application_id.0.clone().into();
                                if msg.info.is_official_example {
                                    prop
                                } else {
                                    prop.hashed()
                                }
                            }),
                            ("recording_id".into(), {
                                let prop: Property = msg.info.recording_id.to_string().into();
                                if msg.info.is_official_example {
                                    prop
                                } else {
                                    prop.hashed()
                                }
                            }),
                            (
                                "recording_source".into(),
                                msg.info.recording_source.to_string().into(),
                            ),
                            (
                                "is_official_example".into(),
                                msg.info.is_official_example.into(),
                            ),
                        ]);

                        analytics.record(re_analytics::Event::data_source_opened());
                    }
                }

                let log_db = self.log_dbs.entry(self.state.selected_rec_id).or_default();

                if let Err(err) = log_db.add(msg) {
                    re_log::error!("Failed to add incoming msg: {err}");
                };
                if start.elapsed() > instant::Duration::from_millis(10) {
                    egui_ctx.request_repaint(); // make sure we keep receiving messages asap
                    break; // don't block the main thread for too long
                }
            }
        }
    }

    fn cleanup(&mut self) {
        crate::profile_function!();

        self.log_dbs.retain(|_, log_db| !log_db.is_empty());

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

        if self.latest_memory_purge.elapsed() < instant::Duration::from_secs(10) {
            // Pruning introduces stutter, and we don't want to stutter too often.
            return;
        }

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
                    re_log::info!(
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
                re_log::info!(
                    "Freed up {} ({:.1}%)",
                    format_bytes(counted_diff as _),
                    100.0 * counted_diff as f32 / counted_before as f32
                );
            }

            self.latest_memory_purge = instant::Instant::now();

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

    fn log_db(&mut self) -> &mut LogDb {
        self.log_dbs.entry(self.state.selected_rec_id).or_default()
    }

    fn show_log_db(&mut self, log_db: LogDb) {
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

    EventLog,
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

    blueprints: HashMap<ApplicationId, crate::ui::Blueprint>,

    /// Which view panel is currently being shown
    panel_selection: PanelSelection,

    event_log_view: crate::event_log_view::EventLogView,

    selection_panel: crate::selection_panel::SelectionPanel,
    time_panel: crate::time_panel::TimePanel,

    #[cfg(not(target_arch = "wasm32"))]
    #[serde(skip)]
    profiler: crate::Profiler,
}

impl AppState {
    fn show(
        &mut self,
        ui: &mut egui::Ui,
        render_ctx: &mut re_renderer::RenderContext,
        log_db: &LogDb,
        re_ui: &re_ui::ReUi,
        component_ui_registry: &ComponentUiRegistry,
    ) {
        crate::profile_function!();

        let Self {
            app_options: options,
            cache,
            selected_rec_id,
            recording_configs,
            panel_selection,
            event_log_view,
            blueprints,
            selection_panel,
            time_panel,
            #[cfg(not(target_arch = "wasm32"))]
                profiler: _,
        } = self;

        let rec_cfg = recording_configs.entry(*selected_rec_id).or_default();
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
        };

        let blueprint = blueprints.entry(selected_app_id.clone()).or_default();
        time_panel.show_panel(&mut ctx, blueprint, ui);
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
                    .or_default()
                    .blueprint_panel_and_viewport(&mut ctx, ui),
                PanelSelection::EventLog => event_log_view.ui(&mut ctx, ui),
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

fn rerun_menu_button_ui(ui: &mut egui::Ui, _frame: &mut eframe::Frame, app: &mut App) {
    // let desired_icon_height = ui.max_rect().height() - 2.0 * ui.spacing_mut().button_padding.y;
    let desired_icon_height = ui.max_rect().height() - 4.0; // TODO(emilk): figure out this fudge
    let desired_icon_height = desired_icon_height.at_most(28.0); // figma size 2023-02-03

    let icon_image = app.re_ui.icon_image(&re_ui::icons::RERUN_MENU);
    let image_size = icon_image.size_vec2() * (desired_icon_height / icon_image.size_vec2().y);
    let texture_id = icon_image.texture_id(ui.ctx());

    ui.menu_image_button(texture_id, image_size, |ui| {
        ui.set_min_width(220.0);
        let spacing = 12.0;

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
            egui::gui_zoom::zoom_menu_buttons(ui, _frame.info().native_pixels_per_point);

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

        #[cfg(debug_assertions)]
        ui.menu_button("Debug", |ui| {
            debug_menu(&mut app.state.app_options, ui);
        });

        #[cfg(not(target_arch = "wasm32"))]
        {
            ui.add_space(spacing);
            Command::Quit.menu_button_ui(ui, &mut app.pending_commands);
        }
    });
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

            let blueprint = app.state.blueprints.entry(selected_app_id).or_default();

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

            app.re_ui
                .medium_icon_toggle_button(
                    ui,
                    &re_ui::icons::RIGHT_PANEL_TOGGLE,
                    &mut blueprint.selection_panel_expanded,
                )
                .on_hover_text(format!(
                    "Toggle Selection View{}",
                    Command::ToggleSelectionPanel.format_shortcut_tooltip_suffix(ui.ctx())
                ));

            app.re_ui
                .medium_icon_toggle_button(
                    ui,
                    &re_ui::icons::BOTTOM_PANEL_TOGGLE,
                    &mut blueprint.time_panel_expanded,
                )
                .on_hover_text(format!(
                    "Toggle Timeline View{}",
                    Command::ToggleTimePanel.format_shortcut_tooltip_suffix(ui.ctx())
                ));

            app.re_ui
                .medium_icon_toggle_button(
                    ui,
                    &re_ui::icons::LEFT_PANEL_TOGGLE,
                    &mut blueprint.blueprint_panel_expanded,
                )
                .on_hover_text(format!(
                    "Toggle Blueprint View{}",
                    Command::ToggleBlueprintPanel.format_shortcut_tooltip_suffix(ui.ctx())
                ));

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
            .on_hover_text("CPU time used by Rerun Viewer each frame. Lower is better.");
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
            "Rerun Viewer is using {} of RAM in {} separate allocations,\n\
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
    if let Some(rx) = &app.rx {
        // TODO(emilk): it would be nice to know if the network stream is still open
        let is_latency_interesting = rx.source().is_network();

        let queue_len = rx.len();

        // empty queue == unreliable latency
        let latency_sec = rx.latency_ns() as f32 / 1e9;
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
                    "When more data is arriving over network than the Rerun Viewer can index, a queue starts building up, leading to latency and increased RAM use.\n\
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
}

// ----------------------------------------------------------------------------

const FILE_SAVER_PROMISE: &str = "file_saver";
const FILE_SAVER_NOTIF_DURATION: Option<std::time::Duration> =
    Some(std::time::Duration::from_secs(4));

fn file_saver_progress_ui(egui_ctx: &egui::Context, app: &mut App) {
    use std::path::PathBuf;

    let file_save_in_progress = app.promise_exists(FILE_SAVER_PROMISE);
    if file_save_in_progress {
        // There's already a file save running in the background.

        if let Some(res) = app.poll_promise::<anyhow::Result<PathBuf>>(FILE_SAVER_PROMISE) {
            // File save promise has returned.

            match res {
                Ok(path) => {
                    let msg = format!("File saved to {path:?}.");
                    re_log::info!(msg);
                    app.toasts.info(msg).set_duration(FILE_SAVER_NOTIF_DURATION);
                }
                Err(err) => {
                    let msg = format!("{err}");
                    re_log::error!(msg);
                    app.toasts
                        .error(msg)
                        .set_duration(FILE_SAVER_NOTIF_DURATION);
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
        ui.add_enabled_ui(!app.log_db().is_empty(), |ui| {
            if ui
                .add(save_button)
                .on_hover_text("Save all data to a Rerun data file (.rrd)")
                .clicked()
            {
                ui.close_menu();
                app.pending_commands.push(Command::Save);
            }

            // We need to know the loop selection _before_ we can even display the
            // button, as this will determine wether its grayed out or not!
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
        let f = save_database_to_file(app, path, loop_selection);
        if let Err(err) = app.spawn_threaded_promise(FILE_SAVER_PROMISE, f) {
            // NOTE: Shouldn't even be possible as the "Save" button is already
            // grayed out at this point... better safe than sorry though.
            app.toasts
                .error(err.to_string())
                .set_duration(FILE_SAVER_NOTIF_DURATION);
        }
    }
}

fn main_view_selector_ui(ui: &mut egui::Ui, app: &mut App) {
    if !app.log_db().is_empty() {
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
            if ui
                .selectable_value(
                    &mut app.state.panel_selection,
                    PanelSelection::EventLog,
                    "Event Log",
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

#[cfg(debug_assertions)]
fn debug_menu(options: &mut AppOptions, ui: &mut egui::Ui) {
    ui.style_mut().wrap = Some(false);

    if ui
        .checkbox(&mut options.show_metrics, "Show metrics")
        .on_hover_text("Show status bar metrics for milliseconds, ram usage, etc")
        .clicked()
    {
        ui.close_menu();
    }

    ui.separator();

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

    ui.separator();

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
}

// ---

/// Returns a closure that, when run, will save the contents of the current database
/// to disk, at the specified `path`.
///
/// If `time_selection` is specified, then only data for that specific timeline over that
/// specific time range will be accounted for.
#[cfg(not(target_arch = "wasm32"))]
fn save_database_to_file(
    app: &mut App,
    path: std::path::PathBuf,
    time_selection: Option<(re_data_store::Timeline, TimeRangeF)>,
) -> impl FnOnce() -> anyhow::Result<std::path::PathBuf> {
    use re_log_types::{EntityPathOpMsg, TimeInt};

    let msgs = match time_selection {
        // Fast path: no query, just dump everything.
        None => app
            .log_db()
            .chronological_log_messages()
            .cloned()
            .collect::<Vec<_>>(),

        // Query path: time to filter!
        Some((timeline, range)) => {
            use std::ops::RangeInclusive;
            let range: RangeInclusive<TimeInt> = range.min.floor()..=range.max.ceil();
            app.log_db()
                .chronological_log_messages()
                .filter(|msg| {
                    match msg {
                        LogMsg::BeginRecordingMsg(_) | LogMsg::Goodbye(_) => {
                            true // timeless
                        }
                        LogMsg::EntityPathOpMsg(EntityPathOpMsg { time_point, .. }) => {
                            time_point.is_timeless() || {
                                let is_within_range = time_point
                                    .get(&timeline)
                                    .map_or(false, |t| range.contains(t));
                                is_within_range
                            }
                        }
                        LogMsg::ArrowMsg(_) => {
                            // TODO(john)
                            false
                        }
                    }
                })
                .cloned()
                .collect::<Vec<_>>()
        }
    };

    move || {
        crate::profile_scope!("save_to_file");

        use anyhow::Context as _;
        let file = std::fs::File::create(path.as_path())
            .with_context(|| format!("Failed to create file at {path:?}"))?;

        re_log_types::encoding::encode(msgs.iter(), file).map(|_| path)
    }
}

#[allow(unused_mut)]
fn load_rrd_to_log_db(mut read: impl std::io::Read) -> anyhow::Result<LogDb> {
    crate::profile_function!();

    let decoder = re_log_types::encoding::Decoder::new(read)?;

    let mut log_db = LogDb::default();
    for msg in decoder {
        log_db.add(msg?)?;
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
        Ok(new_log_db) => {
            re_log::info!("Loaded {path:?}");
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
        Ok(log_db) => {
            re_log::info!("Loaded {name:?}");
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
