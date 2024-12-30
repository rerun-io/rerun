use std::sync::Arc;

use itertools::Itertools as _;

use re_build_info::CrateVersion;
use re_capabilities::MainThreadToken;
use re_data_source::{DataSource, FileContents};
use re_entity_db::entity_db::EntityDb;
use re_log_types::{ApplicationId, FileSource, LogMsg, StoreKind};
use re_renderer::WgpuResourcePoolStatistics;
use re_smart_channel::{ReceiveSet, SmartChannelSource};
use re_ui::{notifications, DesignTokens, UICommand, UICommandSender};
use re_viewer_context::{
    command_channel,
    store_hub::{BlueprintPersistence, StoreHub, StoreHubStats},
    AppOptions, BlueprintUndoState, CommandReceiver, CommandSender, ComponentUiRegistry, PlayState,
    StoreContext, SystemCommand, SystemCommandSender, ViewClass, ViewClassRegistry,
    ViewClassRegistryError,
};

use crate::app_blueprint::PanelStateOverrides;
use crate::{
    app_blueprint::AppBlueprint, app_state::WelcomeScreenState, background_tasks::BackgroundTasks,
    AppState,
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
    /// When the total process RAM reaches this limit, we GC old data.
    pub memory_limit: re_memory::MemoryLimit,

    pub persist_state: bool,

    /// Whether or not the app is running in the context of a Jupyter Notebook.
    pub is_in_notebook: bool,

    /// Set to identify the web page the viewer is running on.
    #[cfg(target_arch = "wasm32")]
    pub location: Option<eframe::Location>,

    /// Take a screenshot of the app and quit.
    /// We use this to generate screenshots of our examples.
    #[cfg(not(target_arch = "wasm32"))]
    pub screenshot_to_path_then_quit: Option<std::path::PathBuf>,

    /// A user has specifically requested the welcome screen be hidden.
    pub hide_welcome_screen: bool,

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

    /// Forces wgpu backend to use the specified graphics API, e.g. `webgl` or `webgpu`.
    pub force_wgpu_backend: Option<String>,

    /// Overwrites hardware acceleration option for video decoding.
    ///
    /// By default uses the last provided setting, which is `auto` if never configured.
    /// This also can be changed in the viewer's option menu.
    pub video_decoder_hw_acceleration: Option<re_video::decode::DecodeHardwareAcceleration>,

    /// Fullscreen is handled by JS on web.
    ///
    /// This holds some callbacks which we use to communicate
    /// about fullscreen state to JS.
    #[cfg(target_arch = "wasm32")]
    pub fullscreen_options: Option<crate::web::FullscreenOptions>,

    /// Default overrides for state of top/side/bottom panels.
    pub panel_state_overrides: PanelStateOverrides,

    /// Whether or not to enable usage of the `History` API on web.
    ///
    /// It is disabled by default.
    ///
    /// This should only be enabled when it is acceptable for `rerun`
    /// to push its own entries into browser history.
    ///
    /// That only makes sense if it has "taken over" a page, and is
    /// the only thing on that page. If you are embedding multiple
    /// viewers onto the same page, then it's better to turn this off.
    ///
    /// We use browser history in a limited way to track the currently
    /// open example recording, see [`crate::history`].
    #[cfg(target_arch = "wasm32")]
    pub enable_history: bool,
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

            hide_welcome_screen: false,

            #[cfg(not(target_arch = "wasm32"))]
            resolution_in_points: None,

            expect_data_soon: None,
            force_wgpu_backend: None,
            video_decoder_hw_acceleration: None,

            #[cfg(target_arch = "wasm32")]
            fullscreen_options: Default::default(),

            panel_state_overrides: Default::default(),

            #[cfg(target_arch = "wasm32")]
            enable_history: false,
        }
    }
}

// ----------------------------------------------------------------------------

#[cfg(not(target_arch = "wasm32"))]
const MIN_ZOOM_FACTOR: f32 = 0.2;
#[cfg(not(target_arch = "wasm32"))]
const MAX_ZOOM_FACTOR: f32 = 5.0;

#[cfg(target_arch = "wasm32")]
struct PendingFilePromise {
    recommended_application_id: Option<ApplicationId>,
    recommended_recording_id: Option<re_log_types::StoreId>,
    force_store_info: bool,
    promise: poll_promise::Promise<Vec<re_data_source::FileContents>>,
}

/// The Rerun Viewer as an [`eframe`] application.
pub struct App {
    #[allow(dead_code)] // Unused on wasm32
    main_thread_token: MainThreadToken,
    build_info: re_build_info::BuildInfo,
    startup_options: StartupOptions,
    start_time: web_time::Instant,
    ram_limit_warner: re_memory::RamLimitWarner,
    pub(crate) egui_ctx: egui::Context,
    screenshotter: crate::screenshotter::Screenshotter,

    #[cfg(target_arch = "wasm32")]
    pub(crate) popstate_listener: Option<crate::history::PopstateListener>,

    #[cfg(not(target_arch = "wasm32"))]
    profiler: re_tracing::Profiler,

    /// Listens to the local text log stream
    text_log_rx: std::sync::mpsc::Receiver<re_log::LogMsg>,

    component_ui_registry: ComponentUiRegistry,

    rx: ReceiveSet<LogMsg>,

    #[cfg(target_arch = "wasm32")]
    open_files_promise: Option<PendingFilePromise>,

    /// What is serialized
    pub(crate) state: AppState,

    /// Pending background tasks, e.g. files being saved.
    pub(crate) background_tasks: BackgroundTasks,

    /// Interface for all recordings and blueprints
    pub(crate) store_hub: Option<StoreHub>,

    /// Notification panel.
    pub(crate) notifications: notifications::NotificationUi,

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

    /// All known view types.
    view_class_registry: ViewClassRegistry,

    pub(crate) panel_state_overrides_active: bool,
    pub(crate) panel_state_overrides: PanelStateOverrides,

    reflection: re_types_core::reflection::Reflection,
}

impl App {
    /// Create a viewer that receives new log messages over time
    pub fn new(
        main_thread_token: MainThreadToken,
        build_info: re_build_info::BuildInfo,
        app_env: &crate::AppEnvironment,
        startup_options: StartupOptions,
        egui_ctx: egui::Context,
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

        let mut state: AppState = if startup_options.persist_state {
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

        if let Some(video_decoder_hw_acceleration) = startup_options.video_decoder_hw_acceleration {
            state.app_options.video_decoder_hw_acceleration = video_decoder_hw_acceleration;
        }

        let mut view_class_registry = ViewClassRegistry::default();
        if let Err(err) = populate_view_class_registry_with_builtin(&mut view_class_registry) {
            re_log::error!(
                "Failed to populate the view type registry with built-in views: {}",
                err
            );
        }

        #[allow(unused_mut, clippy::needless_update)] // false positive on web
        let mut screenshotter = crate::screenshotter::Screenshotter::default();

        #[cfg(not(target_arch = "wasm32"))]
        if let Some(screenshot_path) = startup_options.screenshot_to_path_then_quit.clone() {
            screenshotter.screenshot_to_path_then_quit(&egui_ctx, screenshot_path);
        }

        let (command_sender, command_receiver) = command_channel();

        let mut component_ui_registry = re_component_ui::create_component_ui_registry();
        re_data_ui::register_component_uis(&mut component_ui_registry);

        // TODO(emilk): `Instant::MIN` when we have our own `Instant` that supports it.;
        let long_time_ago = web_time::Instant::now()
            .checked_sub(web_time::Duration::from_secs(1_000_000_000))
            .unwrap_or(web_time::Instant::now());

        analytics.on_viewer_started(build_info);

        let panel_state_overrides = startup_options.panel_state_overrides;

        let reflection = re_types::reflection::generate_reflection().unwrap_or_else(|err| {
            re_log::error!(
                "Failed to create list of serialized default values for components: {err}"
            );
            Default::default()
        });

        Self {
            main_thread_token,
            build_info,
            startup_options,
            start_time: web_time::Instant::now(),
            ram_limit_warner: re_memory::RamLimitWarner::warn_at_fraction_of_max(0.75),
            egui_ctx,
            screenshotter,

            #[cfg(target_arch = "wasm32")]
            popstate_listener: None,

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
            notifications: notifications::NotificationUi::new(),

            memory_panel: Default::default(),
            memory_panel_open: false,

            egui_debug_panel_open: false,

            latest_queue_interest: long_time_ago,

            frame_time_history: egui::util::History::new(1..100, 0.5),

            command_sender,
            command_receiver,
            cmd_palette: Default::default(),

            view_class_registry,

            analytics,

            panel_state_overrides_active: true,
            panel_state_overrides,

            reflection,
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn set_profiler(&mut self, profiler: re_tracing::Profiler) {
        self.profiler = profiler;
    }

    pub fn set_examples_manifest_url(&mut self, url: String) {
        re_log::info!("Using manifest_url={url:?}");
        self.state.set_examples_manifest_url(&self.egui_ctx, url);
    }

    pub fn build_info(&self) -> &re_build_info::BuildInfo {
        &self.build_info
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

    #[allow(clippy::needless_pass_by_ref_mut)]
    pub fn add_receiver(&mut self, rx: re_smart_channel::Receiver<LogMsg>) {
        // Make sure we wake up when a message is sent.
        #[cfg(not(target_arch = "wasm32"))]
        let rx = crate::wake_up_ui_thread_on_each_msg(rx, self.egui_ctx.clone());

        self.rx.add(rx);
    }

    pub fn msg_receive_set(&self) -> &ReceiveSet<LogMsg> {
        &self.rx
    }

    /// Adds a new view class to the viewer.
    pub fn add_view_class<T: ViewClass + Default + 'static>(
        &mut self,
    ) -> Result<(), ViewClassRegistryError> {
        self.view_class_registry.add_class::<T>()
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
                    SmartChannelSource::File(_)
                    | SmartChannelSource::RrdHttpStream { .. }
                    | SmartChannelSource::RerunGrpcStream { .. } => false,

                    SmartChannelSource::WsClient { .. }
                    | SmartChannelSource::JsChannel { .. }
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
                let egui_ctx = egui_ctx.clone();
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
            SystemCommand::UpdateBlueprint(blueprint_id, chunks) => {
                re_log::trace!(
                    "Update blueprint entities: {}",
                    chunks.iter().map(|c| c.entity_path()).join(", ")
                );

                let blueprint_db = store_hub.entity_db_mut(&blueprint_id);

                self.state
                    .blueprint_undo_state
                    .entry(blueprint_id)
                    .or_default()
                    .clear_redo_buffer(blueprint_db);

                for chunk in chunks {
                    match blueprint_db.add_chunk(&Arc::new(chunk)) {
                        Ok(_store_events) => {}
                        Err(err) => {
                            re_log::warn_once!("Failed to store blueprint delta: {err}");
                        }
                    }
                }
            }
            SystemCommand::UndoBlueprint { blueprint_id } => {
                let blueprint_db = store_hub.entity_db_mut(&blueprint_id);
                self.state
                    .blueprint_undo_state
                    .entry(blueprint_id)
                    .or_default()
                    .undo(blueprint_db);
            }
            SystemCommand::RedoBlueprint { blueprint_id } => {
                self.state
                    .blueprint_undo_state
                    .entry(blueprint_id)
                    .or_default()
                    .redo();
            }

            SystemCommand::DropEntity(blueprint_id, entity_path) => {
                let blueprint_db = store_hub.entity_db_mut(&blueprint_id);
                blueprint_db.drop_entity_path_recursive(&entity_path);
            }

            #[cfg(debug_assertions)]
            SystemCommand::EnableInspectBlueprintTimeline(show) => {
                self.app_options_mut().inspect_blueprint_timeline = show;
            }

            SystemCommand::SetSelection(item) => {
                self.state.selection_state.set_selection(item);
            }

            SystemCommand::SetActiveTimeline { rec_id, timeline } => {
                if let Some(rec_cfg) = self.state.recording_config_mut(&rec_id) {
                    rec_cfg.time_ctrl.write().set_timeline(timeline);
                }
            }

            SystemCommand::SetFocus(item) => {
                self.state.focused_item = Some(item);
            }

            #[cfg(not(target_arch = "wasm32"))]
            SystemCommand::FileSaver(file_saver) => {
                if let Err(err) = self.background_tasks.spawn_file_saver(file_saver) {
                    re_log::error!("Failed to save file: {err}");
                }
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
        let mut force_store_info = false;
        let active_application_id = store_context
            .and_then(|ctx| {
                ctx.hub
                    .active_app()
                    // Don't redirect data to the welcome screen.
                    .filter(|&app_id| app_id != &StoreHub::welcome_screen_app_id())
                    .cloned()
            })
            // If we don't have any application ID to recommend (which means we are on the welcome screen),
            // then just generate a new one using a UUID.
            .or_else(|| Some(uuid::Uuid::new_v4().to_string().into()));
        let active_recording_id = store_context
            .and_then(|ctx| ctx.hub.active_recording_id().cloned())
            .or_else(|| {
                // When we're on the welcome screen, there is no recording ID to recommend.
                // But we want one, otherwise multiple things being dropped simultaneously on the
                // welcome screen would end up in different recordings!

                // We're creating a recording just-in-time, directly from the viewer.
                // We need those store infos or the data will just be silently ignored.
                force_store_info = true;

                // NOTE: We don't override blueprints' store IDs anyhow, so it is sound to assume that
                // this can only be a recording.
                Some(re_log_types::StoreId::random(StoreKind::Recording))
            });

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
                for file_path in open_file_dialog_native(self.main_thread_token) {
                    self.command_sender
                        .send_system(SystemCommand::LoadDataSource(DataSource::FilePath(
                            FileSource::FileDialog {
                                recommended_application_id: None,
                                recommended_recording_id: None,
                                force_store_info,
                            },
                            file_path,
                        )));
                }
            }
            #[cfg(target_arch = "wasm32")]
            UICommand::Open => {
                let egui_ctx = egui_ctx.clone();

                let promise = poll_promise::Promise::spawn_local(async move {
                    let file = async_open_rrd_dialog().await;
                    egui_ctx.request_repaint(); // Wake ui thread
                    file
                });

                self.open_files_promise = Some(PendingFilePromise {
                    recommended_application_id: None,
                    recommended_recording_id: None,
                    force_store_info,
                    promise,
                });
            }

            #[cfg(not(target_arch = "wasm32"))]
            UICommand::Import => {
                for file_path in open_file_dialog_native(self.main_thread_token) {
                    self.command_sender
                        .send_system(SystemCommand::LoadDataSource(DataSource::FilePath(
                            FileSource::FileDialog {
                                recommended_application_id: active_application_id.clone(),
                                recommended_recording_id: active_recording_id.clone(),
                                force_store_info,
                            },
                            file_path,
                        )));
                }
            }
            #[cfg(target_arch = "wasm32")]
            UICommand::Import => {
                let egui_ctx = egui_ctx.clone();

                let promise = poll_promise::Promise::spawn_local(async move {
                    let file = async_open_rrd_dialog().await;
                    egui_ctx.request_repaint(); // Wake ui thread
                    file
                });

                self.open_files_promise = Some(PendingFilePromise {
                    recommended_application_id: active_application_id.clone(),
                    recommended_recording_id: active_recording_id.clone(),
                    force_store_info,
                    promise,
                });
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

            UICommand::Undo => {
                if let Some(store_context) = store_context {
                    let blueprint_id = store_context.blueprint.store_id().clone();
                    self.command_sender
                        .send_system(SystemCommand::UndoBlueprint { blueprint_id });
                }
            }
            UICommand::Redo => {
                if let Some(store_context) = store_context {
                    let blueprint_id = store_context.blueprint.store_id().clone();
                    self.command_sender
                        .send_system(SystemCommand::RedoBlueprint { blueprint_id });
                }
            }

            #[cfg(not(target_arch = "wasm32"))]
            UICommand::Quit => {
                egui_ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }

            UICommand::OpenWebHelp => {
                egui_ctx.open_url(egui::output::OpenUrl {
                    url: "https://www.rerun.io/docs/getting-started/navigating-the-viewer"
                        .to_owned(),
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
            UICommand::TogglePanelStateOverrides => {
                self.panel_state_overrides_active ^= true;
            }
            UICommand::ToggleTopPanel => {
                app_blueprint.toggle_top_panel(&self.command_sender);
            }
            UICommand::ToggleBlueprintPanel => {
                app_blueprint.toggle_blueprint_panel(&self.command_sender);
            }
            UICommand::ToggleSelectionPanel => {
                app_blueprint.toggle_selection_panel(&self.command_sender);
            }
            UICommand::ToggleTimePanel => app_blueprint.toggle_time_panel(&self.command_sender),

            UICommand::ToggleChunkStoreBrowser => self.state.show_datastore_ui ^= true,

            #[cfg(debug_assertions)]
            UICommand::ToggleBlueprintInspectionPanel => {
                self.app_options_mut().inspect_blueprint_timeline ^= true;
            }

            #[cfg(debug_assertions)]
            UICommand::ToggleEguiDebugPanel => {
                self.egui_debug_panel_open ^= true;
            }

            UICommand::ToggleFullscreen => {
                self.toggle_fullscreen();
            }

            UICommand::Settings => {
                self.state.show_settings_ui = true;
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
            UICommand::PrintChunkStore => {
                if let Some(ctx) = store_context {
                    let text = format!("{}", ctx.recording.storage_engine().store());
                    egui_ctx.output_mut(|o| o.copied_text = text.clone());
                    println!("{text}");
                }
            }
            #[cfg(not(target_arch = "wasm32"))]
            UICommand::PrintBlueprintStore => {
                if let Some(ctx) = store_context {
                    let text = format!("{}", ctx.blueprint.storage_engine().store());
                    egui_ctx.output_mut(|o| o.copied_text = text.clone());
                    println!("{text}");
                }
            }
            #[cfg(not(target_arch = "wasm32"))]
            UICommand::PrintPrimaryCache => {
                if let Some(ctx) = store_context {
                    let text = format!("{:?}", ctx.recording.storage_engine().cache());
                    egui_ctx.output_mut(|o| o.copied_text = text.clone());
                    println!("{text}");
                }
            }

            #[cfg(debug_assertions)]
            UICommand::ResetEguiMemory => {
                egui_ctx.memory_mut(|mem| *mem = Default::default());

                // re-apply style, which is lost when resetting memory
                re_ui::apply_style_and_install_loaders(egui_ctx);
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
        let Some(rec_cfg) = self.state.recording_config_mut(&rec_id) else {
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
            Some(SmartChannelSource::RrdHttpStream { url, .. }) => format!("{href}?url={url}"),
            _ => href,
        };

        self.egui_ctx
            .output_mut(|o| o.copied_text = direct_link.clone());
        self.notifications
            .success(format!("Copied {direct_link:?} to clipboard"));
    }

    fn memory_panel_ui(
        &self,
        ui: &mut egui::Ui,
        gpu_resource_stats: &WgpuResourcePoolStatistics,
        store_stats: Option<&StoreHubStats>,
    ) {
        let frame = egui::Frame {
            fill: ui.visuals().panel_fill,
            ..DesignTokens::bottom_panel_frame()
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

    fn egui_debug_panel_ui(&self, ui: &mut egui::Ui) {
        let egui_ctx = ui.ctx().clone();

        egui::SidePanel::left("style_panel")
            .default_width(300.0)
            .resizable(true)
            .frame(DesignTokens::top_panel_frame())
            .show_animated_inside(ui, self.egui_debug_panel_open, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if ui
                        .button("request_discard")
                        .on_hover_text("Request a second layout pass. Just for testing.")
                        .clicked()
                    {
                        ui.ctx().request_discard("testing");
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

                crate::ui::mobile_warning_ui(ui);

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
                    if let Some(store_context) = store_context {
                        let entity_db = store_context.recording;

                        #[cfg(target_arch = "wasm32")]
                        let is_history_enabled = self.startup_options.enable_history;
                        #[cfg(not(target_arch = "wasm32"))]
                        let is_history_enabled = false;

                        render_ctx.begin_frame();
                        self.state.show(
                            app_blueprint,
                            ui,
                            render_ctx,
                            entity_db,
                            store_context,
                            &self.reflection,
                            &self.component_ui_registry,
                            &self.view_class_registry,
                            &self.rx,
                            &self.command_sender,
                            &WelcomeScreenState {
                                hide: self.startup_options.hide_welcome_screen,
                                opacity: self.welcome_screen_opacity(egui_ctx),
                            },
                            is_history_enabled,
                        );
                        render_ctx.before_submit();
                    }
                }

                self.show_text_logs_as_notifications();
            });
    }

    /// Show recent text log messages to the user as toast notifications.
    fn show_text_logs_as_notifications(&mut self) {
        re_tracing::profile_function!();

        while let Ok(message) = self.text_log_rx.try_recv() {
            self.notifications.add_log(message);
        }
    }

    fn receive_messages(&self, store_hub: &mut StoreHub, egui_ctx: &egui::Context) {
        re_tracing::profile_function!();

        let start = web_time::Instant::now();

        while let Some((channel_source, msg)) = self.rx.try_recv() {
            re_log::trace!("Received a message from {channel_source:?}"); // Used by `test_ui_wakeup` test app!

            let msg = match msg.payload {
                re_smart_channel::SmartMessagePayload::Msg(msg) => msg,

                re_smart_channel::SmartMessagePayload::Flush { on_flush_done } => {
                    on_flush_done();
                    continue;
                }

                re_smart_channel::SmartMessagePayload::Quit(err) => {
                    if let Some(err) = err {
                        let log_msg =
                            format!("Data source {} has left unexpectedly: {err}", msg.source);

                        #[cfg(not(target_arch = "wasm32"))]
                        if err
                            .downcast_ref::<re_sdk_comms::ConnectionError>()
                            .is_some_and(|e| {
                                matches!(e, re_sdk_comms::ConnectionError::UnknownClient)
                            })
                        {
                            // This can happen if a client tried to connect but didn't send the `re_sdk_comms::PROTOCOL_HEADER`.
                            // Likely an unknown client stumbled onto the wrong port - don't log as an error.
                            // (for more information see https://github.com/rerun-io/rerun/issues/5883).
                            re_log::debug!("{log_msg}");
                            continue;
                        }

                        re_log::warn!("{log_msg}");
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

            // TODO(cmc): we have to keep grabbing and releasing entity_db because everything references
            // everything and some of it is mutable and some not… it's really not pretty, but it
            // does the job for now.

            {
                let entity_db = store_hub.entity_db_mut(store_id);
                if entity_db.data_source.is_none() {
                    entity_db.data_source = Some((*channel_source).clone());
                }
            }

            match store_hub.entity_db_mut(store_id).add(&msg) {
                Ok(store_events) => {
                    if let Some(caches) = store_hub.active_caches() {
                        caches.on_store_events(&store_events);
                    }

                    self.validate_loaded_events(&store_events);
                }

                Err(err) => {
                    re_log::error_once!("Failed to add incoming msg: {err}");
                }
            }

            let entity_db = store_hub.entity_db_mut(store_id);

            match &msg {
                LogMsg::SetStoreInfo(_) => {
                    // Set the recording-id after potentially creating the store in the hub.
                    // This ordering is important because the `StoreHub` internally
                    // updates the app-id when changing the recording.
                    match store_id.kind {
                        StoreKind::Recording => {
                            re_log::trace!("Opening a new recording: '{store_id}'");
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
                            re_log::trace!(
                                "Activating blueprint that was loaded from {channel_source}"
                            );
                            let app_id = info.application_id.clone();
                            if cmd.make_default {
                                store_hub
                                    .set_default_blueprint_for_app(&app_id, store_id)
                                    .unwrap_or_else(|err| {
                                        re_log::warn!("Failed to make blueprint default: {err}");
                                    });
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

    /// After loading some data; check if the loaded data makes sense.
    fn validate_loaded_events(&self, store_events: &[re_chunk_store::ChunkStoreEvent]) {
        re_tracing::profile_function!();

        for event in store_events {
            let chunk = &event.diff.chunk;
            for component in chunk.component_names() {
                if let Some(archetype_name) = component.indicator_component_archetype() {
                    if let Some(archetype) = self
                        .reflection
                        .archetype_reflection_from_short_name(&archetype_name)
                    {
                        for &view_type in archetype.view_types {
                            if !cfg!(feature = "map_view") && view_type == "MapView" {
                                re_log::warn_once!("Found map-related archetype, but viewer was not compiled with the `map_view` feature.");
                            }
                        }
                    } else {
                        re_log::debug_once!("Unknown archetype: {archetype_name}");
                    }
                }
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

        // Reset egui:
        egui_ctx.memory_mut(|mem| *mem = Default::default());

        // Restore style:
        re_ui::apply_style_and_install_loaders(egui_ctx);

        if let Err(err) = crate::reset_viewer_persistence() {
            re_log::warn!("Failed to reset viewer: {err}");
        }
    }

    pub fn recording_db(&self) -> Option<&EntityDb> {
        self.store_hub
            .as_ref()
            .and_then(|store_hub| store_hub.active_recording())
    }

    // NOTE: Relying on `self` is dangerous, as this is called during a time where some internal
    // fields may have been temporarily `take()`n out. Keep this a static method.
    fn handle_dropping_files(
        egui_ctx: &egui::Context,
        store_ctx: Option<&StoreContext<'_>>,
        command_sender: &CommandSender,
    ) {
        preview_files_being_dropped(egui_ctx);

        let dropped_files = egui_ctx.input_mut(|i| std::mem::take(&mut i.raw.dropped_files));

        if dropped_files.is_empty() {
            return;
        }

        let mut force_store_info = false;
        let active_application_id = store_ctx
            .and_then(|ctx| {
                ctx.hub
                    .active_app()
                    // Don't redirect data to the welcome screen.
                    .filter(|&app_id| app_id != &StoreHub::welcome_screen_app_id())
                    .cloned()
            })
            // If we don't have any application ID to recommend (which means we are on the welcome screen),
            // then just generate a new one using a UUID.
            .or_else(|| Some(uuid::Uuid::new_v4().to_string().into()));
        let active_recording_id = store_ctx
            .and_then(|ctx| ctx.hub.active_recording_id().cloned())
            .or_else(|| {
                // When we're on the welcome screen, there is no recording ID to recommend.
                // But we want one, otherwise multiple things being dropped simultaneously on the
                // welcome screen would end up in different recordings!

                // We're creating a recording just-in-time, directly from the viewer.
                // We need those store infos or the data will just be silently ignored.
                force_store_info = true;

                // NOTE: We don't override blueprints' store IDs anyhow, so it is sound to assume that
                // this can only be a recording.
                Some(re_log_types::StoreId::random(StoreKind::Recording))
            });

        for file in dropped_files {
            if let Some(bytes) = file.bytes {
                // This is what we get on Web.
                command_sender.send_system(SystemCommand::LoadDataSource(
                    DataSource::FileContents(
                        FileSource::DragAndDrop {
                            recommended_application_id: active_application_id.clone(),
                            recommended_recording_id: active_recording_id.clone(),
                            force_store_info,
                        },
                        FileContents {
                            name: file.name.clone(),
                            bytes: bytes.clone(),
                        },
                    ),
                ));
                continue;
            }

            #[cfg(not(target_arch = "wasm32"))]
            if let Some(path) = file.path {
                command_sender.send_system(SystemCommand::LoadDataSource(DataSource::FilePath(
                    FileSource::DragAndDrop {
                        recommended_application_id: active_application_id.clone(),
                        recommended_recording_id: active_recording_id.clone(),
                        force_store_info,
                    },
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
                | SmartChannelSource::RerunGrpcStream { .. }
                | SmartChannelSource::Stdin
                | SmartChannelSource::RrdWebEventListener
                | SmartChannelSource::Sdk
                | SmartChannelSource::WsClient { .. }
                | SmartChannelSource::JsChannel { .. } => {
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

    #[allow(clippy::unused_self)]
    pub(crate) fn toggle_fullscreen(&self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let fullscreen = self
                .egui_ctx
                .input(|i| i.viewport().fullscreen.unwrap_or(false));
            self.egui_ctx
                .send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fullscreen));
        }

        #[cfg(target_arch = "wasm32")]
        {
            if let Some(options) = &self.startup_options.fullscreen_options {
                // Tell JS to toggle fullscreen.
                if let Err(err) = options.on_toggle.call() {
                    re_log::error!("{}", crate::web_tools::string_from_js_value(err));
                };
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn is_fullscreen_allowed(&self) -> bool {
        self.startup_options.fullscreen_options.is_some()
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn is_fullscreen_mode(&self) -> bool {
        if let Some(options) = &self.startup_options.fullscreen_options {
            // Ask JS if fullscreen is on or not.
            match options.get_state.call() {
                Ok(v) => return v.is_truthy(),
                Err(err) => re_log::error_once!("{}", crate::web_tools::string_from_js_value(err)),
            }
        }

        false
    }

    #[allow(clippy::needless_pass_by_ref_mut)] // False positive on wasm
    fn process_screenshot_result(
        &mut self,
        image: &Arc<egui::ColorImage>,
        user_data: &egui::UserData,
    ) {
        use re_viewer_context::ScreenshotInfo;

        if let Some(info) = &user_data
            .data
            .as_ref()
            .and_then(|data| data.downcast_ref::<ScreenshotInfo>())
        {
            let ScreenshotInfo {
                ui_rect,
                pixels_per_point,
                name,
                target,
            } = (*info).clone();

            let rgba = if let Some(ui_rect) = ui_rect {
                Arc::new(image.region(&ui_rect, Some(pixels_per_point)))
            } else {
                image.clone()
            };

            match target {
                #[cfg(not(target_arch = "wasm32"))] // TODO(#8264): copy-to-screenshot on web
                re_viewer_context::ScreenshotTarget::CopyToClipboard => {
                    re_viewer_context::Clipboard::with(|clipboard| {
                        clipboard.set_image(
                            [rgba.width(), rgba.height()],
                            bytemuck::cast_slice(rgba.as_raw()),
                        );
                    });
                }

                re_viewer_context::ScreenshotTarget::SaveToDisk => {
                    use image::ImageEncoder as _;
                    let mut png_bytes: Vec<u8> = Vec::new();
                    if let Err(err) = image::codecs::png::PngEncoder::new(&mut png_bytes)
                        .write_image(
                            rgba.as_raw(),
                            rgba.width() as u32,
                            rgba.height() as u32,
                            image::ExtendedColorType::Rgba8,
                        )
                    {
                        re_log::error!("Failed to encode screenshot as PNG: {err}");
                    } else {
                        let file_name = format!("{name}.png");
                        self.command_sender.save_file_dialog(
                            self.main_thread_token,
                            &file_name,
                            "Save screenshot".to_owned(),
                            png_bytes,
                        );
                    }
                }
            }
        } else {
            #[cfg(not(target_arch = "wasm32"))] // no full-app screenshotting on web
            self.screenshotter.save(image);
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn blueprint_loader() -> BlueprintPersistence {
    // TODO(#2579): implement persistence for web
    BlueprintPersistence {
        loader: None,
        saver: None,
        validator: Some(Box::new(crate::blueprint::is_valid_blueprint)),
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

        if let Some(bundle) = crate::loading::load_blueprint_file(&blueprint_path) {
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

        let messages = blueprint.to_messages(None);
        let rrd_version = blueprint
            .store_info()
            .and_then(|info| info.store_version)
            .unwrap_or(re_build_info::CrateVersion::LOCAL);

        // TODO(jleibs): Should we push this into a background thread? Blueprints should generally
        // be small & fast to save, but maybe not once we start adding big pieces of user data?
        crate::saving::encode_to_file(rrd_version, &blueprint_path, messages)?;

        re_log::debug!("Saved blueprint for {app_id} to {blueprint_path:?}");

        Ok(())
    }

    BlueprintPersistence {
        loader: Some(Box::new(load_blueprint_from_disk)),
        saver: Some(Box::new(save_blueprint_to_disk)),
        validator: Some(Box::new(crate::blueprint::is_valid_blueprint)),
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
                hub.gc_blueprints(&self.state.blueprint_undo_state);
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
        if self.startup_options.enable_history {
            // Handle pressing the back/forward mouse buttons explicitly, since eframe catches those.
            let back_pressed =
                egui_ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Extra1));
            let fwd_pressed =
                egui_ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Extra2));

            if back_pressed {
                crate::history::go_back();
            }
            if fwd_pressed {
                crate::history::go_forward();
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
        if let Some(PendingFilePromise {
            recommended_application_id,
            recommended_recording_id,
            force_store_info,
            promise,
        }) = &self.open_files_promise
        {
            if let Some(files) = promise.ready() {
                for file in files {
                    self.command_sender
                        .send_system(SystemCommand::LoadDataSource(DataSource::FileContents(
                            FileSource::FileDialog {
                                recommended_application_id: recommended_application_id.clone(),
                                recommended_recording_id: recommended_recording_id.clone(),
                                force_store_info: *force_store_info,
                            },
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

        {
            // TODO(andreas): store the re_renderer somewhere else.
            let egui_renderer = {
                let render_state = frame.wgpu_render_state().unwrap();
                &mut render_state.renderer.read()
            };
            let render_ctx = egui_renderer
                .callback_resources
                .get::<re_renderer::RenderContext>()
                .unwrap();

            // We haven't called `begin_frame` at this point, so pretend we did and add one to the active frame index.
            let renderer_active_frame_idx = render_ctx.active_frame_idx().wrapping_add(1);
            store_hub.begin_frame(renderer_active_frame_idx);
        }

        self.receive_messages(&mut store_hub, egui_ctx);

        if self.app_options().blueprint_gc {
            store_hub.gc_blueprints(&self.state.blueprint_undo_state);
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
            if let Some(app_id) = apps.first().copied() {
                store_hub.set_active_app(app_id.clone());
            } else {
                store_hub.set_active_app(StoreHub::welcome_screen_app_id());
            }
        }

        {
            let store_context = store_hub.read_context();

            let blueprint_query = store_context.as_ref().map_or(
                BlueprintUndoState::default_query(),
                |store_context| {
                    self.state
                        .blueprint_query_for_viewer(store_context.blueprint)
                },
            );

            let app_blueprint = AppBlueprint::new(
                store_context.as_ref(),
                &blueprint_query,
                egui_ctx,
                self.panel_state_overrides_active
                    .then_some(self.panel_state_overrides),
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

            if let Some(cmd) = self.cmd_palette.show(egui_ctx) {
                self.command_sender.send_ui(cmd);
            }

            Self::handle_dropping_files(egui_ctx, store_context.as_ref(), &self.command_sender);

            // Run pending commands last (so we don't have to wait for a repaint before they are run):
            self.run_pending_ui_commands(egui_ctx, &app_blueprint, store_context.as_ref());
        }
        self.run_pending_system_commands(&mut store_hub, egui_ctx);

        // Return the `StoreHub` to the Viewer so we have it on the next frame
        self.store_hub = Some(store_hub);

        // Check for returned screenshot:
        egui_ctx.input(|i| {
            for event in &i.raw.events {
                if let egui::Event::Screenshot {
                    image, user_data, ..
                } = event
                {
                    self.process_screenshot_result(image, user_data);
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

/// Add built-in views to the registry.
fn populate_view_class_registry_with_builtin(
    view_class_registry: &mut ViewClassRegistry,
) -> Result<(), ViewClassRegistryError> {
    re_tracing::profile_function!();
    view_class_registry.add_class::<re_view_bar_chart::BarChartView>()?;
    view_class_registry.add_class::<re_view_dataframe::DataframeView>()?;
    view_class_registry.add_class::<re_view_graph::GraphView>()?;
    #[cfg(feature = "map_view")]
    view_class_registry.add_class::<re_view_map::MapView>()?;
    view_class_registry.add_class::<re_view_spatial::SpatialView2D>()?;
    view_class_registry.add_class::<re_view_spatial::SpatialView3D>()?;
    view_class_registry.add_class::<re_view_tensor::TensorView>()?;
    view_class_registry.add_class::<re_view_text_document::TextDocumentView>()?;
    view_class_registry.add_class::<re_view_text_log::TextView>()?;
    view_class_registry.add_class::<re_view_time_series::TimeSeriesView>()?;

    Ok(())
}

fn paint_background_fill(ui: &egui::Ui) {
    // This is required because the streams view (time panel)
    // has rounded top corners, which leaves a gap.
    // So we fill in that gap (and other) here.
    // Of course this does some over-draw, but we have to live with that.

    ui.painter().rect_filled(
        ui.max_rect().shrink(0.5),
        re_ui::DesignTokens::native_window_rounding(),
        ui.visuals().panel_fill,
    );
}

fn paint_native_window_frame(egui_ctx: &egui::Context) {
    let painter = egui::Painter::new(
        egui_ctx.clone(),
        egui::LayerId::new(egui::Order::TOP, egui::Id::new("native_window_frame")),
        egui::Rect::EVERYTHING,
    );

    painter.rect_stroke(
        egui_ctx.screen_rect().shrink(0.5),
        re_ui::DesignTokens::native_window_rounding(),
        re_ui::design_tokens().native_frame_stroke,
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

/// [This may only be called on the main thread](https://docs.rs/rfd/latest/rfd/#macos-non-windowed-applications-async-and-threading).
#[cfg(not(target_arch = "wasm32"))]
fn open_file_dialog_native(_: crate::MainThreadToken) -> Vec<std::path::PathBuf> {
    re_tracing::profile_function!();

    let supported: Vec<_> = if re_data_loader::iter_external_loaders().len() == 0 {
        re_data_loader::supported_extensions().collect()
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
    let supported: Vec<_> = re_data_loader::supported_extensions().collect();

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
    loop_selection: Option<(re_entity_db::Timeline, re_log_types::ResolvedTimeRangeF)>,
) -> anyhow::Result<()> {
    let Some(entity_db) = store_context.as_ref().map(|view| view.recording) else {
        // NOTE: Can only happen if saving through the command palette.
        anyhow::bail!("No recording data to save");
    };

    let rrd_version = entity_db
        .store_info()
        .and_then(|info| info.store_version)
        .unwrap_or(re_build_info::CrateVersion::LOCAL);

    let file_name = "data.rrd";

    let title = if loop_selection.is_some() {
        "Save loop selection"
    } else {
        "Save recording"
    };

    save_entity_db(
        app,
        rrd_version,
        file_name.to_owned(),
        title.to_owned(),
        entity_db.to_messages(loop_selection),
    )
}

fn save_blueprint(app: &mut App, store_context: Option<&StoreContext<'_>>) -> anyhow::Result<()> {
    let Some(store_context) = store_context else {
        anyhow::bail!("No blueprint to save");
    };

    re_tracing::profile_function!();

    let rrd_version = store_context
        .blueprint
        .store_info()
        .and_then(|info| info.store_version)
        .unwrap_or(re_build_info::CrateVersion::LOCAL);

    // We change the recording id to a new random one,
    // otherwise when saving and loading a blueprint file, we can end up
    // in a situation where the store_id we're loading is the same as the currently active one,
    // which mean they will merge in a strange way.
    // This is also related to https://github.com/rerun-io/rerun/issues/5295
    let new_store_id = re_log_types::StoreId::random(StoreKind::Blueprint);
    let messages = store_context.blueprint.to_messages(None).map(|mut msg| {
        if let Ok(msg) = &mut msg {
            msg.set_store_id(new_store_id.clone());
        };
        msg
    });

    let file_name = format!(
        "{}.rbl",
        crate::saving::sanitize_app_id(&store_context.app_id)
    );
    let title = "Save blueprint";

    save_entity_db(app, rrd_version, file_name, title.to_owned(), messages)
}

// TODO(emilk): unify this with `ViewerContext::save_file_dialog`
#[allow(clippy::needless_pass_by_ref_mut)] // `app` is only used on native
#[allow(clippy::unnecessary_wraps)] // cannot return error on web
fn save_entity_db(
    #[allow(unused_variables)] app: &mut App, // only used on native
    rrd_version: CrateVersion,
    file_name: String,
    title: String,
    messages: impl Iterator<Item = re_chunk::ChunkResult<LogMsg>>,
) -> anyhow::Result<()> {
    re_tracing::profile_function!();

    // TODO(#6984): Ideally we wouldn't collect at all and just stream straight to the
    // encoder from the store.
    //
    // From a memory usage perspective this isn't too bad though: the data within is still
    // refcounted straight from the store in any case.
    //
    // It just sucks latency-wise.
    let messages = messages.collect::<Vec<_>>();

    // Web
    #[cfg(target_arch = "wasm32")]
    {
        wasm_bindgen_futures::spawn_local(async move {
            if let Err(err) =
                async_save_dialog(rrd_version, &file_name, &title, messages.into_iter()).await
            {
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
            app.background_tasks.spawn_file_saver(move || {
                crate::saving::encode_to_file(rrd_version, &path, messages.into_iter())?;
                Ok(path)
            })?;
        }
    }

    Ok(())
}

#[cfg(target_arch = "wasm32")]
async fn async_save_dialog(
    rrd_version: CrateVersion,
    file_name: &str,
    title: &str,
    messages: impl Iterator<Item = re_chunk::ChunkResult<LogMsg>>,
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
        rrd_version,
        re_log_encoding::EncodingOptions::MSGPACK_COMPRESSED,
        messages,
    )?;
    file_handle.write(&bytes).await.context("Failed to save")
}
