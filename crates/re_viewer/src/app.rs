use re_data_source::{DataSource, FileContents};
use re_entity_db::entity_db::EntityDb;
use re_log_types::{ApplicationId, FileSource, LogMsg, StoreKind};
use re_renderer::WgpuResourcePoolStatistics;
use re_smart_channel::{ReceiveSet, SmartChannelSource};
use re_ui::{toasts, UICommand, UICommandSender};
use re_viewer_context::{
    command_channel,
    store_hub::{BlueprintPersistence, StoreHub, StoreHubStats},
    AppOptions, CommandReceiver, CommandSender, ComponentUiRegistry, PlayState, SpaceViewClass,
    SpaceViewClassRegistry, SpaceViewClassRegistryError, StoreContext, SystemCommand,
    SystemCommandSender,
};

use crate::{app_blueprint::AppBlueprint, background_tasks::BackgroundTasks, AppState};

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
    /// When the total process RAM reaches this limit, we GC old data.
    pub memory_limit: re_memory::MemoryLimit,

    pub persist_state: bool,

    /// Whether or not the app is running in the context of a Jupyter Notebook.
    pub is_in_notebook: bool,

    /// Set to identify the web page the viewer is running on.
    #[cfg(target_arch = "wasm32")]
    pub location: Option<eframe::Location>,

    /// Take a screenshot of the app and quit.
    /// We use this to generate screenshots of our exmples.
    #[cfg(not(target_arch = "wasm32"))]
    pub screenshot_to_path_then_quit: Option<std::path::PathBuf>,

    /// Set the screen resolution in logical points.
    #[cfg(not(target_arch = "wasm32"))]
    pub resolution_in_points: Option<[f32; 2]>,

    /// This is a hint that we expect a recording to stream in very soon.
    ///
    /// This is set by the `spawn()` method in our logging SDK.
    ///
    /// The viewer will respond by fading in the welcome screen,
    /// instead of showing it directly.
    /// This ensures that it won't blink for a few frames before switching to the recording.
    pub expect_data_soon: Option<bool>,

    /// Forces wgpu backend to use the specified graphics API.
    pub force_wgpu_backend: Option<String>,
}

impl Default for StartupOptions {
    fn default() -> Self {
        Self {
            memory_limit: re_memory::MemoryLimit::from_fraction_of_total(0.75),
            persist_state: true,
            is_in_notebook: false,

            #[cfg(target_arch = "wasm32")]
            location: None,

            #[cfg(not(target_arch = "wasm32"))]
            screenshot_to_path_then_quit: None,

            #[cfg(not(target_arch = "wasm32"))]
            resolution_in_points: None,

            expect_data_soon: None,
            force_wgpu_backend: None,
        }
    }
}

// ----------------------------------------------------------------------------

#[cfg(not(target_arch = "wasm32"))]
const MIN_ZOOM_FACTOR: f32 = 0.2;
#[cfg(not(target_arch = "wasm32"))]
const MAX_ZOOM_FACTOR: f32 = 5.0;

/// The Rerun Viewer as an [`eframe`] application.
pub struct App {
    build_info: re_build_info::BuildInfo,
    startup_options: StartupOptions,
    start_time: web_time::Instant,
    ram_limit_warner: re_memory::RamLimitWarner,
    pub(crate) re_ui: re_ui::ReUi,
    screenshotter: crate::screenshotter::Screenshotter,

    #[cfg(not(target_arch = "wasm32"))]
    profiler: re_tracing::Profiler,

    /// Listens to the local text log stream
    text_log_rx: std::sync::mpsc::Receiver<re_log::LogMsg>,

    component_ui_registry: ComponentUiRegistry,

    rx: ReceiveSet<LogMsg>,

    #[cfg(target_arch = "wasm32")]
    open_files_promise: Option<poll_promise::Promise<Vec<re_data_source::FileContents>>>,

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

    egui_debug_panel_open: bool,

    pub(crate) latest_queue_interest: web_time::Instant,

    /// Measures how long a frame takes to paint
    pub(crate) frame_time_history: egui::util::History<f32>,

    /// Commands to run at the end of the frame.
    pub command_sender: CommandSender,
    command_receiver: CommandReceiver,
    cmd_palette: re_ui::CommandPalette,

    analytics: crate::viewer_analytics::ViewerAnalytics,

    /// All known space view types.
    space_view_class_registry: SpaceViewClassRegistry,
}

impl App {
    /// Create a viewer that receives new log messages over time
    pub fn new(
        build_info: re_build_info::BuildInfo,
        app_env: &crate::AppEnvironment,
        startup_options: StartupOptions,
        re_ui: re_ui::ReUi,
        storage: Option<&dyn eframe::Storage>,
    ) -> Self {
        re_tracing::profile_function!();

        let analytics =
            crate::viewer_analytics::ViewerAnalytics::new(&startup_options, app_env.clone());

        let (logger, text_log_rx) = re_log::ChannelLogger::new(re_log::LevelFilter::Info);
        if re_log::add_boxed_logger(Box::new(logger)).is_err() {
            // This can happen when `rerun` crate users call `spawn`. TODO(emilk): make `spawn` spawn a new process.
            re_log::debug!(
                "re_log not initialized - we won't see any log messages as GUI notifications"
            );
        }

        let state: AppState = if startup_options.persist_state {
            storage
                .and_then(|storage| {
                    // This re-implements: `eframe::get_value` so we can customize the warning message.
                    // TODO(#2849): More thorough error-handling.
                    storage.get_string(eframe::APP_KEY).and_then(|value| {
                        match ron::from_str(&value) {
                            Ok(value) => Some(value),
                            Err(err) => {
                                re_log::warn!("Failed to restore application state. This is expected if you have just upgraded Rerun versions.");
                                re_log::debug!("Failed to decode RON for app state: {err}");
                                None
                            }
                        }
                    })
                })
                .unwrap_or_default()
        } else {
            AppState::default()
        };

        let mut space_view_class_registry = SpaceViewClassRegistry::default();
        if let Err(err) = populate_space_view_class_registry_with_builtin(
            &mut space_view_class_registry,
            state.app_options(),
        ) {
            re_log::error!(
                "Failed to populate space view type registry with built-in space views: {}",
                err
            );
        }

        #[allow(unused_mut, clippy::needless_update)] // false positive on web
        let mut screenshotter = crate::screenshotter::Screenshotter::default();

        #[cfg(not(target_arch = "wasm32"))]
        if let Some(screenshot_path) = startup_options.screenshot_to_path_then_quit.clone() {
            screenshotter.screenshot_to_path_then_quit(&re_ui.egui_ctx, screenshot_path);
        }

        let (command_sender, command_receiver) = command_channel();

        let component_ui_registry = re_data_ui::create_component_ui_registry();

        // TODO(emilk): `Instant::MIN` when we have our own `Instant` that supports it.;
        let long_time_ago = web_time::Instant::now()
            .checked_sub(web_time::Duration::from_secs(1_000_000_000))
            .unwrap_or(web_time::Instant::now());

        analytics.on_viewer_started(build_info);

        Self {
            build_info,
            startup_options,
            start_time: web_time::Instant::now(),
            ram_limit_warner: re_memory::RamLimitWarner::warn_at_fraction_of_max(0.75),
            re_ui,
            screenshotter,

            #[cfg(not(target_arch = "wasm32"))]
            profiler: Default::default(),

            text_log_rx,
            component_ui_registry,
            rx: Default::default(),
            #[cfg(target_arch = "wasm32")]
            open_files_promise: Default::default(),
            state,
            background_tasks: Default::default(),
            store_hub: Some(StoreHub::new(
                blueprint_loader(),
                &crate::app_blueprint::setup_welcome_screen_blueprint,
            )),
            toasts: toasts::Toasts::new(),
            memory_panel: Default::default(),
            memory_panel_open: false,

            egui_debug_panel_open: false,

            latest_queue_interest: long_time_ago,

            frame_time_history: egui::util::History::new(1..100, 0.5),

            command_sender,
            command_receiver,
            cmd_palette: Default::default(),

            space_view_class_registry,

            analytics,
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn set_profiler(&mut self, profiler: re_tracing::Profiler) {
        self.profiler = profiler;
    }

    pub fn set_examples_manifest_url(&mut self, url: String) {
        re_log::info!("Using manifest_url={url:?}");
        self.state
            .set_examples_manifest_url(&self.re_ui.egui_ctx, url);
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

    pub fn add_receiver(&mut self, rx: re_smart_channel::Receiver<LogMsg>) {
        // Make sure we wake up when a message is sent.
        #[cfg(not(target_arch = "wasm32"))]
        let rx = crate::wake_up_ui_thread_on_each_msg(rx, self.re_ui.egui_ctx.clone());

        self.rx.add(rx);
    }

    pub fn msg_receive_set(&self) -> &ReceiveSet<LogMsg> {
        &self.rx
    }

    /// Adds a new space view class to the viewer.
    pub fn add_space_view_class<T: SpaceViewClass + Default + 'static>(
        &mut self,
    ) -> Result<(), SpaceViewClassRegistryError> {
        self.space_view_class_registry.add_class::<T>()
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
        egui_ctx: &egui::Context,
        app_blueprint: &AppBlueprint<'_>,
        store_context: Option<&StoreContext<'_>>,
    ) {
        while let Some(cmd) = self.command_receiver.recv_ui() {
            self.run_ui_command(egui_ctx, app_blueprint, store_context, cmd);
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
            SystemCommand::ActivateApp(app_id) => {
                store_hub.set_active_app(app_id);
            }

            SystemCommand::CloseApp(app_id) => {
                store_hub.close_app(&app_id);
            }

            SystemCommand::ActivateRecording(store_id) => {
                store_hub.set_activate_recording(store_id);
            }

            SystemCommand::CloseStore(store_id) => {
                store_hub.remove(&store_id);
            }

            SystemCommand::CloseAllRecordings => {
                store_hub.clear_recordings();

                // Stop receiving into the old recordings.
                // This is most important when going back to the example screen by using the "Back"
                // button in the browser, and there is still a connection downloading an .rrd.
                // That's the case of `SmartChannelSource::RrdHttpStream`.
                // TODO(emilk): exactly what things get kept and what gets cleared?
                self.rx.retain(|r| match r.source() {
                    SmartChannelSource::File(_) | SmartChannelSource::RrdHttpStream { .. } => false,

                    SmartChannelSource::WsClient { .. }
                    | SmartChannelSource::RrdWebEventListener
                    | SmartChannelSource::Sdk
                    | SmartChannelSource::TcpServer { .. }
                    | SmartChannelSource::Stdin => true,
                });
            }

            SystemCommand::ClearSourceAndItsStores(source) => {
                self.rx.retain(|r| r.source() != &source);
                store_hub.retain(|db| db.data_source.as_ref() != Some(&source));
            }

            SystemCommand::AddReceiver(rx) => {
                re_log::debug!("Received AddReceiver");
                self.add_receiver(rx);
            }

            SystemCommand::LoadDataSource(data_source) => {
                let egui_ctx = self.re_ui.egui_ctx.clone();

                // On native, `add_receiver` spawns a thread that wakes up the ui thread
                // on any new message. On web we cannot spawn threads, so instead we need
                // to supply a waker that is called when new messages arrive in background tasks
                let waker = Box::new(move || {
                    // Spend a few more milliseconds decoding incoming messages,
                    // then trigger a repaint (https://github.com/rerun-io/rerun/issues/963):
                    egui_ctx.request_repaint_after(std::time::Duration::from_millis(10));
                });

                match data_source.stream(Some(waker)) {
                    Ok(rx) => {
                        self.add_receiver(rx);
                    }
                    Err(err) => {
                        re_log::error!("Failed to open data source: {}", re_error::format(err));
                    }
                }
            }

            SystemCommand::ResetViewer => self.reset_viewer(store_hub, egui_ctx),
            SystemCommand::ClearAndGenerateBlueprint => {
                re_log::debug!("Clear and generate new blueprint");
                // By clearing the default blueprint and the active blueprint
                // it will be re-generated based on the default auto behavior.
                store_hub.clear_default_blueprint();
                store_hub.clear_active_blueprint();
            }
            SystemCommand::ClearActiveBlueprint => {
                // By clearing the blueprint the default blueprint will be restored
                // at the beginning of the next frame.
                re_log::debug!("Reset blueprint to default");
                store_hub.clear_active_blueprint();
                egui_ctx.request_repaint(); // Many changes take a frame delay to show up.
            }
            SystemCommand::UpdateBlueprint(blueprint_id, updates) => {
                // We only want to update the blueprint if the "inspect blueprint timeline" mode is
                // disabled. This is because the blueprint inspector allows you to change the
                // blueprint query time, which in turn updates the displayed state of the UI itself.
                // This means any updates we receive while in this mode may be relative to a historical
                // blueprint state and would conflict with the current true blueprint state.

                // TODO(jleibs): When the blueprint is in "follow-mode" we should actually be able
                // to apply updates here, but this needs more validation and testing to be safe.
                if !self.state.app_options.inspect_blueprint_timeline {
                    let blueprint_db = store_hub.entity_db_mut(&blueprint_id);
                    for row in updates {
                        match blueprint_db.add_data_row(row) {
                            Ok(()) => {}
                            Err(err) => {
                                re_log::warn_once!("Failed to store blueprint delta: {err}");
                            }
                        }
                    }
                }
            }
            #[cfg(debug_assertions)]
            SystemCommand::EnableInspectBlueprintTimeline(show) => {
                self.app_options_mut().inspect_blueprint_timeline = show;
            }
            SystemCommand::EnableExperimentalDataframeSpaceView(enabled) => {
                let result = if enabled {
                    self.space_view_class_registry
                        .add_class::<re_space_view_dataframe::DataframeSpaceView>()
                } else {
                    self.space_view_class_registry
                        .remove_class::<re_space_view_dataframe::DataframeSpaceView>()
                };

                if let Err(err) = result {
                    re_log::warn_once!(
                        "Failed to {} experimental dataframe space view: {err}",
                        if enabled { "enable" } else { "disable" }
                    );
                }
            }

            SystemCommand::SetSelection(item) => {
                self.state.selection_state.set_selection(item);
            }

            SystemCommand::SetFocus(item) => {
                self.state.focused_item = Some(item);
            }
        }
    }

    fn run_ui_command(
        &mut self,
        egui_ctx: &egui::Context,
        app_blueprint: &AppBlueprint<'_>,
        store_context: Option<&StoreContext<'_>>,
        cmd: UICommand,
    ) {
        match cmd {
            UICommand::SaveRecording => {
                if let Err(err) = save_recording(self, store_context, None) {
                    re_log::error!("Failed to save recording: {err}");
                }
            }
            UICommand::SaveRecordingSelection => {
                if let Err(err) = save_recording(
                    self,
                    store_context,
                    self.state.loop_selection(store_context),
                ) {
                    re_log::error!("Failed to save recording: {err}");
                }
            }

            UICommand::SaveBlueprint => {
                if let Err(err) = save_blueprint(self, store_context) {
                    re_log::error!("Failed to save blueprint: {err}");
                }
            }

            #[cfg(not(target_arch = "wasm32"))]
            UICommand::Open => {
                for file_path in open_file_dialog_native() {
                    self.command_sender
                        .send_system(SystemCommand::LoadDataSource(DataSource::FilePath(
                            FileSource::FileDialog,
                            file_path,
                        )));
                }
            }
            #[cfg(target_arch = "wasm32")]
            UICommand::Open => {
                let egui_ctx = egui_ctx.clone();
                self.open_files_promise = Some(poll_promise::Promise::spawn_local(async move {
                    let file = async_open_rrd_dialog().await;
                    egui_ctx.request_repaint(); // Wake ui thread
                    file
                }));
            }
            UICommand::CloseCurrentRecording => {
                let cur_rec = store_context.map(|ctx| ctx.recording.store_id());
                if let Some(cur_rec) = cur_rec {
                    self.command_sender
                        .send_system(SystemCommand::CloseStore(cur_rec.clone()));
                }
            }
            UICommand::CloseAllRecordings => {
                self.command_sender
                    .send_system(SystemCommand::CloseAllRecordings);
            }

            #[cfg(not(target_arch = "wasm32"))]
            UICommand::Quit => {
                egui_ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }

            UICommand::OpenWebHelp => {
                egui_ctx.open_url(egui::output::OpenUrl {
                    url: "https://www.rerun.io/docs/getting-started/viewer-walkthrough".to_owned(),
                    new_tab: true,
                });
            }

            UICommand::OpenRerunDiscord => {
                egui_ctx.open_url(egui::output::OpenUrl {
                    url: "https://discord.gg/PXtCgFBSmH".to_owned(),
                    new_tab: true,
                });
            }

            UICommand::ResetViewer => self.command_sender.send_system(SystemCommand::ResetViewer),
            UICommand::ClearAndGenerateBlueprint => {
                self.command_sender
                    .send_system(SystemCommand::ClearAndGenerateBlueprint);
            }

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

            #[cfg(debug_assertions)]
            UICommand::ToggleBlueprintInspectionPanel => {
                self.app_options_mut().inspect_blueprint_timeline ^= true;
            }

            #[cfg(debug_assertions)]
            UICommand::ToggleEguiDebugPanel => {
                self.egui_debug_panel_open ^= true;
            }

            #[cfg(not(target_arch = "wasm32"))]
            UICommand::ToggleFullscreen => {
                let fullscreen = egui_ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
                egui_ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fullscreen));
            }
            #[cfg(not(target_arch = "wasm32"))]
            UICommand::ZoomIn => {
                let mut zoom_factor = egui_ctx.zoom_factor();
                zoom_factor += 0.1;
                zoom_factor = zoom_factor.clamp(MIN_ZOOM_FACTOR, MAX_ZOOM_FACTOR);
                zoom_factor = (zoom_factor * 10.).round() / 10.;
                egui_ctx.set_zoom_factor(zoom_factor);
            }
            #[cfg(not(target_arch = "wasm32"))]
            UICommand::ZoomOut => {
                let mut zoom_factor = egui_ctx.zoom_factor();
                zoom_factor -= 0.1;
                zoom_factor = zoom_factor.clamp(MIN_ZOOM_FACTOR, MAX_ZOOM_FACTOR);
                zoom_factor = (zoom_factor * 10.).round() / 10.;
                egui_ctx.set_zoom_factor(zoom_factor);
            }
            #[cfg(not(target_arch = "wasm32"))]
            UICommand::ZoomReset => {
                egui_ctx.set_zoom_factor(1.0);
            }

            UICommand::SelectionPrevious => {
                self.state.selection_state.select_previous();
            }
            UICommand::SelectionNext => {
                self.state.selection_state.select_next();
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
                self.screenshotter.request_screenshot(egui_ctx);
            }
            #[cfg(not(target_arch = "wasm32"))]
            UICommand::PrintDataStore => {
                if let Some(ctx) = store_context {
                    let table = ctx.recording.store().to_data_table();
                    match table {
                        Ok(table) => {
                            let text = format!("{table}");
                            self.re_ui
                                .egui_ctx
                                .output_mut(|o| o.copied_text = text.clone());
                            println!("{text}");
                        }
                        Err(err) => {
                            println!("{err}");
                        }
                    }
                }
            }
            #[cfg(not(target_arch = "wasm32"))]
            UICommand::PrintBlueprintStore => {
                if let Some(ctx) = store_context {
                    let table = ctx.blueprint.store().to_data_table();
                    match table {
                        Ok(table) => {
                            let text = format!("{table}");
                            self.re_ui
                                .egui_ctx
                                .output_mut(|o| o.copied_text = text.clone());
                            println!("{text}");
                        }
                        Err(err) => {
                            println!("{err}");
                        }
                    }
                }
            }
            #[cfg(not(target_arch = "wasm32"))]
            UICommand::ClearPrimaryCache => {
                if let Some(ctx) = store_context {
                    ctx.recording.query_caches().clear();
                }
            }
            #[cfg(not(target_arch = "wasm32"))]
            UICommand::PrintPrimaryCache => {
                if let Some(ctx) = store_context {
                    let text = format!("{:?}", ctx.recording.query_caches2());
                    self.re_ui
                        .egui_ctx
                        .output_mut(|o| o.copied_text = text.clone());
                    println!("{text}");
                }
            }

            #[cfg(target_arch = "wasm32")]
            UICommand::CopyDirectLink => {
                self.run_copy_direct_link_command(store_context);
            }

            #[cfg(target_arch = "wasm32")]
            UICommand::RestartWithWebGl => {
                if crate::web_tools::set_url_parameter_and_refresh("renderer", "webgl").is_err() {
                    re_log::error!("Failed to set URL parameter `renderer=webgl` & refresh page.");
                }
            }

            #[cfg(target_arch = "wasm32")]
            UICommand::RestartWithWebGpu => {
                if crate::web_tools::set_url_parameter_and_refresh("renderer", "webgpu").is_err() {
                    re_log::error!("Failed to set URL parameter `renderer=webgpu` & refresh page.");
                }
            }
        }
    }

    fn run_time_control_command(
        &mut self,
        store_context: Option<&StoreContext<'_>>,
        command: TimeControlCommand,
    ) {
        let Some(entity_db) = store_context.as_ref().map(|ctx| ctx.recording) else {
            return;
        };
        let rec_id = entity_db.store_id();
        let Some(rec_cfg) = self.state.recording_config_mut(rec_id) else {
            return;
        };
        let time_ctrl = rec_cfg.time_ctrl.get_mut();

        let times_per_timeline = entity_db.times_per_timeline();

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

    #[cfg(target_arch = "wasm32")]
    fn run_copy_direct_link_command(&mut self, store_context: Option<&StoreContext<'_>>) {
        let location = web_sys::window().unwrap().location();
        let origin = location.origin().unwrap();
        let host = location.host().unwrap();
        let pathname = location.pathname().unwrap();

        let hosted_viewer_path = if self.build_info.is_final() {
            // final release, use version tag
            format!("version/{}", self.build_info.version)
        } else {
            // not a final release, use commit hash
            format!("commit/{}", self.build_info.short_git_hash())
        };

        // links to `app.rerun.io` can be made into permanent links:
        let href = if host == "app.rerun.io" {
            format!("https://app.rerun.io/{hosted_viewer_path}")
        } else if host == "rerun.io" && pathname.starts_with("/viewer") {
            format!("https://rerun.io/viewer/{hosted_viewer_path}")
        } else {
            format!("{origin}{pathname}")
        };

        let direct_link = match store_context
            .map(|ctx| ctx.recording)
            .and_then(|rec| rec.data_source.as_ref())
        {
            Some(SmartChannelSource::RrdHttpStream { url }) => format!("{href}?url={url}"),
            _ => href,
        };

        self.re_ui
            .egui_ctx
            .output_mut(|o| o.copied_text = direct_link.clone());
        self.toasts.add(toasts::Toast {
            kind: toasts::ToastKind::Success,
            text: format!("Copied {direct_link:?} to clipboard"),
            options: toasts::ToastOptions::with_ttl_in_seconds(4.0),
        });
    }

    fn memory_panel_ui(
        &mut self,
        ui: &mut egui::Ui,
        gpu_resource_stats: &WgpuResourcePoolStatistics,
        store_stats: Option<&StoreHubStats>,
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
                    self.re_ui(),
                    &self.startup_options.memory_limit,
                    gpu_resource_stats,
                    store_stats,
                );
            });
    }

    fn egui_debug_panel_ui(&mut self, ui: &mut egui::Ui) {
        let egui_ctx = ui.ctx().clone();

        egui::SidePanel::left("style_panel")
            .default_width(300.0)
            .resizable(true)
            .frame(self.re_ui.top_panel_frame())
            .show_animated_inside(ui, self.egui_debug_panel_open, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
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

    /// Top-level ui function.
    ///
    /// Shows the viewer ui.
    #[allow(clippy::too_many_arguments)]
    fn ui(
        &mut self,
        egui_ctx: &egui::Context,
        frame: &eframe::Frame,
        app_blueprint: &AppBlueprint<'_>,
        gpu_resource_stats: &WgpuResourcePoolStatistics,
        store_context: Option<&StoreContext<'_>>,
        store_stats: Option<&StoreHubStats>,
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
                    frame,
                    self,
                    app_blueprint,
                    store_context,
                    gpu_resource_stats,
                    ui,
                );

                self.memory_panel_ui(ui, gpu_resource_stats, store_stats);

                self.egui_debug_panel_ui(ui);

                // TODO(andreas): store the re_renderer somewhere else.
                let egui_renderer = {
                    let render_state = frame.wgpu_render_state().unwrap();
                    &mut render_state.renderer.write()
                };

                if let Some(render_ctx) = egui_renderer
                    .callback_resources
                    .get_mut::<re_renderer::RenderContext>()
                {
                    // TODO(#5283): There's no great reason to do this if we have no store-view and
                    // subsequently won't actually be rendering anything. However, doing this here
                    // avoids a hang on linux. Consider moving this back inside the below `if let`.
                    // once the upstream issues that fix the hang properly have been resolved.
                    render_ctx.begin_frame();
                    if let Some(store_view) = store_context {
                        let entity_db = store_view.recording;

                        self.state.show(
                            app_blueprint,
                            ui,
                            render_ctx,
                            entity_db,
                            store_view,
                            &self.re_ui,
                            &self.component_ui_registry,
                            &self.space_view_class_registry,
                            &self.rx,
                            &self.command_sender,
                            self.welcome_screen_opacity(egui_ctx),
                        );
                    }
                    render_ctx.before_submit();
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

        while let Some((channel_source, msg)) = self.rx.try_recv() {
            let msg = match msg.payload {
                re_smart_channel::SmartMessagePayload::Msg(msg) => msg,
                re_smart_channel::SmartMessagePayload::Quit(err) => {
                    if let Some(err) = err {
                        re_log::warn!("Data source {} has left unexpectedly: {err}", msg.source);
                    } else {
                        re_log::debug!("Data source {} has finished", msg.source);
                    }
                    continue;
                }
            };

            let store_id = msg.store_id();

            if store_hub.is_active_blueprint(store_id) {
                // TODO(#5514): handle loading of active blueprints.
                re_log::warn_once!("Loading a blueprint {store_id} that is active. See https://github.com/rerun-io/rerun/issues/5514 for details.");
            }

            let entity_db = store_hub.entity_db_mut(store_id);

            if entity_db.data_source.is_none() {
                entity_db.data_source = Some((*channel_source).clone());
            }

            if let Err(err) = entity_db.add(&msg) {
                re_log::error_once!("Failed to add incoming msg: {err}");
            };

            match &msg {
                LogMsg::SetStoreInfo(_) => {
                    // Set the recording-id after potentially creating the store in the hub.
                    // This ordering is important because the `StoreHub` internally
                    // updates the app-id when changing the recording.
                    match store_id.kind {
                        StoreKind::Recording => {
                            re_log::debug!("Opening a new recording: {store_id}");
                            store_hub.set_active_recording_id(store_id.clone());

                            // Also select the new recording:
                            self.command_sender.send_system(SystemCommand::SetSelection(
                                re_viewer_context::Item::StoreId(store_id.clone()),
                            ));

                            // If the viewer is in the background, tell the user that it has received something new.
                            egui_ctx.send_viewport_cmd(
                                egui::ViewportCommand::RequestUserAttention(
                                    egui::UserAttentionType::Informational,
                                ),
                            );
                        }
                        StoreKind::Blueprint => {
                            // We wait with activating blueprints until they are fully loaded,
                            // so that we don't run heuristics on half-loaded blueprints.
                            // TODO(#5297): heed special "end-of-blueprint" message to activate blueprint.
                            // Otherwise on a mixed connection (SDK sending both blueprint and recording)
                            // the blueprint won't be activated until the whole _recording_ has finished loading.
                        }
                    }
                }

                LogMsg::ArrowMsg(_, _) => {
                    // Handled by `EntityDb::add`
                }

                LogMsg::BlueprintActivationCommand(cmd) => match store_id.kind {
                    StoreKind::Recording => {
                        re_log::debug!(
                            "Unexpected `BlueprintActivationCommand` message for {store_id}"
                        );
                    }
                    StoreKind::Blueprint => {
                        if let Some(info) = entity_db.store_info() {
                            re_log::debug!(
                                "Activating blueprint that was loaded from {channel_source}"
                            );
                            let app_id = info.application_id.clone();
                            if cmd.make_default {
                                store_hub.set_default_blueprint_for_app(&app_id, store_id);
                            }
                            if cmd.make_active {
                                store_hub
                                    .set_cloned_blueprint_active_for_app(&app_id, store_id)
                                    .unwrap_or_else(|err| {
                                        re_log::warn!("Failed to make blueprint active: {err}");
                                    });
                                store_hub.set_active_app(app_id); // Switch to this app, e.g. on drag-and-drop of a blueprint file

                                // If the viewer is in the background, tell the user that it has received something new.
                                egui_ctx.send_viewport_cmd(
                                    egui::ViewportCommand::RequestUserAttention(
                                        egui::UserAttentionType::Informational,
                                    ),
                                );
                            }
                        } else {
                            re_log::warn!(
                                "Got ActivateStore message without first receiving a SetStoreInfo"
                            );
                        }
                    }
                },
            }

            // Do analytics after ingesting the new message,
            // because that's when the `entity_db.store_info` is set,
            // which we use in the analytics call.
            let entity_db = store_hub.entity_db_mut(store_id);
            let is_new_store = matches!(&msg, LogMsg::SetStoreInfo(_msg));
            if is_new_store && entity_db.store_kind() == StoreKind::Recording {
                self.analytics.on_open_recording(entity_db);
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
            re_log::info_once!(
                "Reached memory limit of {}, dropping oldest data.",
                format_limit(limit.max_bytes)
            );

            let fraction_to_purge = (minimum_fraction_to_purge + 0.2).clamp(0.25, 1.0);

            re_log::trace!("RAM limit: {}", format_limit(limit.max_bytes));
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
    fn reset_viewer(&mut self, store_hub: &mut StoreHub, egui_ctx: &egui::Context) {
        self.state = Default::default();

        store_hub.clear_all_cloned_blueprints();

        // Reset egui, but keep the style:
        let style = egui_ctx.style();
        egui_ctx.memory_mut(|mem| *mem = Default::default());
        egui_ctx.set_style((*style).clone());

        if let Err(err) = crate::reset_viewer_persistence() {
            re_log::warn!("Failed to reset viewer: {err}");
        }
    }

    pub fn recording_db(&self) -> Option<&EntityDb> {
        self.store_hub
            .as_ref()
            .and_then(|store_hub| store_hub.active_recording())
    }

    fn handle_dropping_files(&mut self, egui_ctx: &egui::Context) {
        preview_files_being_dropped(egui_ctx);

        let dropped_files = egui_ctx.input_mut(|i| std::mem::take(&mut i.raw.dropped_files));

        for file in dropped_files {
            if let Some(bytes) = file.bytes {
                // This is what we get on Web.
                self.command_sender
                    .send_system(SystemCommand::LoadDataSource(DataSource::FileContents(
                        FileSource::DragAndDrop,
                        FileContents {
                            name: file.name.clone(),
                            bytes: bytes.clone(),
                        },
                    )));
                continue;
            }

            #[cfg(not(target_arch = "wasm32"))]
            if let Some(path) = file.path {
                self.command_sender
                    .send_system(SystemCommand::LoadDataSource(DataSource::FilePath(
                        FileSource::DragAndDrop,
                        path,
                    )));
            }
        }
    }

    fn should_fade_in_welcome_screen(&self) -> bool {
        if let Some(expect_data_soon) = self.startup_options.expect_data_soon {
            return expect_data_soon;
        }

        // The reason for the fade-in is to avoid the welcome screen
        // flickering quickly before receiving some data.
        // So: if we expect data very soon, we do a fade-in.

        for source in self.rx.sources() {
            #[allow(clippy::match_same_arms)]
            match &*source {
                SmartChannelSource::File(_)
                | SmartChannelSource::RrdHttpStream { .. }
                | SmartChannelSource::Stdin
                | SmartChannelSource::RrdWebEventListener
                | SmartChannelSource::Sdk
                | SmartChannelSource::WsClient { .. } => {
                    return true; // We expect data soon, so fade-in
                }

                SmartChannelSource::TcpServer { .. } => {
                    // We start a TCP server by default in native rerun, i.e. when just running `rerun`,
                    // and in that case fading in the welcome screen would be slightly annoying.
                    // However, we also use the TCP server for sending data from the logging SDKs
                    // when they call `spawn()`, and in that case we really want to fade in the welcome screen.
                    // Therefore `spawn()` uses the special `--expect-data-soon` flag
                    // (handled earlier in this function), so here we know we are in the other case:
                    // a user calling `rerun` in their terminal (don't fade in).
                }
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
}

#[cfg(target_arch = "wasm32")]
fn blueprint_loader() -> BlueprintPersistence {
    // TODO(#2579): implement persistence for web
    BlueprintPersistence {
        loader: None,
        saver: None,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn blueprint_loader() -> BlueprintPersistence {
    use re_entity_db::StoreBundle;

    fn load_blueprint_from_disk(app_id: &ApplicationId) -> anyhow::Result<Option<StoreBundle>> {
        let blueprint_path = crate::saving::default_blueprint_path(app_id)?;
        if !blueprint_path.exists() {
            return Ok(None);
        }

        re_log::debug!("Trying to load blueprint for {app_id} from {blueprint_path:?}");

        let with_notifications = false;

        if let Some(bundle) =
            crate::loading::load_blueprint_file(&blueprint_path, with_notifications)
        {
            for store in bundle.entity_dbs() {
                if store.store_kind() == StoreKind::Blueprint
                    && !crate::blueprint::is_valid_blueprint(store)
                {
                    re_log::warn_once!("Blueprint for {app_id} at {blueprint_path:?} appears invalid - will ignore. This is expected if you have just upgraded Rerun versions.");
                    return Ok(None);
                }
            }
            Ok(Some(bundle))
        } else {
            Ok(None)
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn save_blueprint_to_disk(app_id: &ApplicationId, blueprint: &EntityDb) -> anyhow::Result<()> {
        let blueprint_path = crate::saving::default_blueprint_path(app_id)?;

        let messages = blueprint.to_messages(None)?;

        // TODO(jleibs): Should we push this into a background thread? Blueprints should generally
        // be small & fast to save, but maybe not once we start adding big pieces of user data?
        crate::saving::encode_to_file(&blueprint_path, messages.iter())?;

        re_log::debug!("Saved blueprint for {app_id} to {blueprint_path:?}");

        Ok(())
    }

    BlueprintPersistence {
        loader: Some(Box::new(load_blueprint_from_disk)),
        saver: Some(Box::new(save_blueprint_to_disk)),
    }
}

impl eframe::App for App {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0; 4] // transparent so we can get rounded corners when doing [`re_ui::CUSTOM_WINDOW_DECORATIONS`]
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        if !self.startup_options.persist_state {
            return;
        }

        re_tracing::profile_function!();

        // Save the app state
        eframe::set_value(storage, eframe::APP_KEY, &self.state);

        // Save the blueprints
        // TODO(#2579): implement web-storage for blueprints as well
        if let Some(hub) = &mut self.store_hub {
            if self.state.app_options.blueprint_gc {
                hub.gc_blueprints();
            }

            if let Err(err) = hub.save_app_blueprints() {
                re_log::error!("Saving blueprints failed: {err}");
            }
        } else {
            re_log::error!("Could not save blueprints: the store hub is not available");
        }
    }

    fn update(&mut self, egui_ctx: &egui::Context, frame: &mut eframe::Frame) {
        if let Some(seconds) = frame.info().cpu_usage {
            self.frame_time_history
                .add(egui_ctx.input(|i| i.time), seconds);
        }

        #[cfg(target_arch = "wasm32")]
        {
            // Handle pressing the back/forward mouse buttons explicitly, since eframe catches those.
            let back_pressed =
                egui_ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Extra1));
            let fwd_pressed =
                egui_ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Extra2));

            if back_pressed {
                crate::web_tools::go_back();
            }
            if fwd_pressed {
                crate::web_tools::go_forward();
            }
        }

        // Temporarily take the `StoreHub` out of the Viewer so it doesn't interfere with mutability
        let mut store_hub = self.store_hub.take().unwrap();

        #[cfg(not(target_arch = "wasm32"))]
        if let Some(resolution_in_points) = self.startup_options.resolution_in_points.take() {
            egui_ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(
                resolution_in_points.into(),
            ));
        }

        #[cfg(not(target_arch = "wasm32"))]
        if self.screenshotter.update(egui_ctx).quit {
            egui_ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        if self.startup_options.memory_limit.is_unlimited() {
            // we only warn about high memory usage if the user hasn't specified a limit
            self.ram_limit_warner.update();
        }

        #[cfg(target_arch = "wasm32")]
        if let Some(promise) = &self.open_files_promise {
            if let Some(files) = promise.ready() {
                for file in files {
                    self.command_sender
                        .send_system(SystemCommand::LoadDataSource(DataSource::FileContents(
                            FileSource::FileDialog,
                            file.clone(),
                        )));
                }
                self.open_files_promise = None;
            }
        }

        // NOTE: GPU resource stats are cheap to compute so we always do.
        // TODO(andreas): store the re_renderer somewhere else.
        let gpu_resource_stats = {
            re_tracing::profile_scope!("gpu_resource_stats");

            let egui_renderer = {
                let render_state = frame.wgpu_render_state().unwrap();
                &mut render_state.renderer.read()
            };
            let render_ctx = egui_renderer
                .callback_resources
                .get::<re_renderer::RenderContext>()
                .unwrap();

            // Query statistics before begin_frame as this might be more accurate if there's resources that we recreate every frame.
            render_ctx.gpu_resources.statistics()
        };

        // NOTE: Store and caching stats are very costly to compute: only do so if the memory panel
        // is opened.
        let store_stats = self.memory_panel_open.then(|| store_hub.stats());

        // do early, before doing too many allocations
        self.memory_panel
            .update(&gpu_resource_stats, store_stats.as_ref());

        self.check_keyboard_shortcuts(egui_ctx);

        self.purge_memory_if_needed(&mut store_hub);

        self.state.cache.begin_frame();

        self.show_text_logs_as_notifications();
        self.receive_messages(&mut store_hub, egui_ctx);

        if self.app_options().blueprint_gc {
            store_hub.gc_blueprints();
        }

        store_hub.purge_empty();
        self.state.cleanup(&store_hub);

        file_saver_progress_ui(egui_ctx, &mut self.background_tasks); // toasts for background file saver

        // Make sure some app is active
        // Must be called before `read_context` below.
        if store_hub.active_app().is_none() {
            let apps: std::collections::BTreeSet<&ApplicationId> = store_hub
                .store_bundle()
                .entity_dbs()
                .filter_map(|db| db.app_id())
                .filter(|&app_id| app_id != &StoreHub::welcome_screen_app_id())
                .collect();
            if let Some(app_id) = apps.first().cloned() {
                store_hub.set_active_app(app_id.clone());
            } else {
                store_hub.set_active_app(StoreHub::welcome_screen_app_id());
            }
        }

        let store_context = store_hub.read_context();

        let app_blueprint = AppBlueprint::new(
            store_context.as_ref(),
            &self.state.blueprint_query_for_viewer(),
            egui_ctx,
        );

        self.ui(
            egui_ctx,
            frame,
            &app_blueprint,
            &gpu_resource_stats,
            store_context.as_ref(),
            store_stats.as_ref(),
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

        self.handle_dropping_files(egui_ctx);

        // Run pending commands last (so we don't have to wait for a repaint before they are run):
        self.run_pending_ui_commands(egui_ctx, &app_blueprint, store_context.as_ref());
        self.run_pending_system_commands(&mut store_hub, egui_ctx);

        // Return the `StoreHub` to the Viewer so we have it on the next frame
        self.store_hub = Some(store_hub);

        // Check for returned screenshot:
        #[cfg(not(target_arch = "wasm32"))]
        egui_ctx.input(|i| {
            for event in &i.raw.events {
                if let egui::Event::Screenshot { image, .. } = event {
                    self.screenshotter.save(image);
                }
            }
        });

        egui_ctx.output_mut(|o| {
            // Open all links in a new tab (https://github.com/rerun-io/rerun/issues/4105)
            if let Some(open_url) = &mut o.open_url {
                open_url.new_tab = true;
            }
        });
    }

    #[cfg(target_arch = "wasm32")]
    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(&mut *self)
    }
}

/// Add built-in space views to the registry.
fn populate_space_view_class_registry_with_builtin(
    space_view_class_registry: &mut SpaceViewClassRegistry,
    app_options: &AppOptions,
) -> Result<(), SpaceViewClassRegistryError> {
    re_tracing::profile_function!();
    space_view_class_registry.add_class::<re_space_view_bar_chart::BarChartSpaceView>()?;
    space_view_class_registry.add_class::<re_space_view_spatial::SpatialSpaceView2D>()?;
    space_view_class_registry.add_class::<re_space_view_spatial::SpatialSpaceView3D>()?;
    space_view_class_registry.add_class::<re_space_view_tensor::TensorSpaceView>()?;
    space_view_class_registry.add_class::<re_space_view_text_document::TextDocumentSpaceView>()?;
    space_view_class_registry.add_class::<re_space_view_text_log::TextSpaceView>()?;
    space_view_class_registry.add_class::<re_space_view_time_series::TimeSeriesSpaceView>()?;

    if app_options.experimental_dataframe_space_view {
        space_view_class_registry.add_class::<re_space_view_dataframe::DataframeSpaceView>()?;
    }

    Ok(())
}

fn paint_background_fill(ui: &egui::Ui) {
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
fn open_file_dialog_native() -> Vec<std::path::PathBuf> {
    re_tracing::profile_function!();

    let supported: Vec<_> = if re_data_source::iter_external_loaders().len() == 0 {
        re_data_source::supported_extensions().collect()
    } else {
        vec![]
    };

    let mut dialog = rfd::FileDialog::new();

    // If there's at least one external loader registered, then literally anything goes!
    if !supported.is_empty() {
        dialog = dialog.add_filter("Supported files", &supported);
    }

    dialog.pick_files().unwrap_or_default()
}

#[cfg(target_arch = "wasm32")]
async fn async_open_rrd_dialog() -> Vec<re_data_source::FileContents> {
    let supported: Vec<_> = re_data_source::supported_extensions().collect();

    let files = rfd::AsyncFileDialog::new()
        .add_filter("Supported files", &supported)
        .pick_files()
        .await
        .unwrap_or_default();

    let mut file_contents = Vec::with_capacity(files.len());

    for file in files {
        let file_name = file.file_name();
        re_log::debug!("Reading {file_name}…");
        let bytes = file.read().await;
        re_log::debug!(
            "{file_name} was {}",
            re_format::format_bytes(bytes.len() as _)
        );
        file_contents.push(re_data_source::FileContents {
            name: file_name,
            bytes: bytes.into(),
        });
    }

    file_contents
}

fn save_recording(
    app: &mut App,
    store_context: Option<&StoreContext<'_>>,
    loop_selection: Option<(re_entity_db::Timeline, re_log_types::TimeRangeF)>,
) -> anyhow::Result<()> {
    let Some(entity_db) = store_context.as_ref().map(|view| view.recording) else {
        // NOTE: Can only happen if saving through the command palette.
        anyhow::bail!("No recording data to save");
    };

    let file_name = "data.rrd";

    let title = if loop_selection.is_some() {
        "Save loop selection"
    } else {
        "Save recording"
    };

    save_entity_db(app, file_name.to_owned(), title.to_owned(), || {
        entity_db.to_messages(loop_selection)
    })
}

fn save_blueprint(app: &mut App, store_context: Option<&StoreContext<'_>>) -> anyhow::Result<()> {
    let Some(store_context) = store_context else {
        anyhow::bail!("No blueprint to save");
    };

    re_tracing::profile_function!();

    // We change the recording id to a new random one,
    // otherwise when saving and loading a blueprint file, we can end up
    // in a situation where the store_id we're loading is the same as the currently active one,
    // which mean they will merge in a strange way.
    // This is also related to https://github.com/rerun-io/rerun/issues/5295
    let new_store_id = re_log_types::StoreId::random(StoreKind::Blueprint);
    let mut messages = store_context.blueprint.to_messages(None)?;
    for message in &mut messages {
        message.set_store_id(new_store_id.clone());
    }

    let file_name = format!(
        "{}.rbl",
        crate::saving::sanitize_app_id(&store_context.app_id)
    );
    let title = "Save blueprint";

    save_entity_db(app, file_name, title.to_owned(), || Ok(messages))
}

#[allow(clippy::needless_pass_by_ref_mut)] // `app` is only used on native
fn save_entity_db(
    #[allow(unused_variables)] app: &mut App, // only used on native
    file_name: String,
    title: String,
    to_log_messages: impl FnOnce() -> re_log_types::DataTableResult<Vec<LogMsg>>,
) -> anyhow::Result<()> {
    re_tracing::profile_function!();

    // Web
    #[cfg(target_arch = "wasm32")]
    {
        let messages = to_log_messages()?;

        wasm_bindgen_futures::spawn_local(async move {
            if let Err(err) = async_save_dialog(&file_name, &title, &messages).await {
                re_log::error!("File saving failed: {err}");
            }
        });
    }

    // Native
    #[cfg(not(target_arch = "wasm32"))]
    {
        let path = {
            re_tracing::profile_scope!("file_dialog");
            rfd::FileDialog::new()
                .set_file_name(file_name)
                .set_title(title)
                .save_file()
        };
        if let Some(path) = path {
            let messages = to_log_messages()?;
            app.background_tasks.spawn_file_saver(move || {
                crate::saving::encode_to_file(&path, messages.iter())?;
                Ok(path)
            })?;
        }
    }

    Ok(())
}

#[cfg(target_arch = "wasm32")]
async fn async_save_dialog(
    file_name: &str,
    title: &str,
    messages: &[LogMsg],
) -> anyhow::Result<()> {
    use anyhow::Context as _;

    let file_handle = rfd::AsyncFileDialog::new()
        .set_file_name(file_name)
        .set_title(title)
        .save_file()
        .await;

    let Some(file_handle) = file_handle else {
        return Ok(()); // aborted
    };

    let bytes = re_log_encoding::encoder::encode_as_bytes(
        re_log_encoding::EncodingOptions::COMPRESSED,
        messages.iter(),
    )?;
    file_handle.write(&bytes).await.context("Failed to save")
}
