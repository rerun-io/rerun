use web_time::Instant;

use re_data_source::{DataSource, FileContents};
use re_entity_db::entity_db::EntityDb;
use re_log_types::{FileSource, LogMsg, StoreKind};
use re_renderer::WgpuResourcePoolStatistics;
use re_smart_channel::{ReceiveSet, SmartChannelSource};
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
    /// We use this to generate screenshots of our exmples.
    #[cfg(not(target_arch = "wasm32"))]
    pub screenshot_to_path_then_quit: Option<std::path::PathBuf>,

    /// Set the screen resolution in logical points.
    #[cfg(not(target_arch = "wasm32"))]
    pub resolution_in_points: Option<[f32; 2]>,

    pub skip_welcome_screen: bool,
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

            skip_welcome_screen: false,
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

    style_panel_open: bool,

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
    pub fn new(
        build_info: re_build_info::BuildInfo,
        app_env: &crate::AppEnvironment,
        startup_options: StartupOptions,
        re_ui: re_ui::ReUi,
        storage: Option<&dyn eframe::Storage>,
    ) -> Self {
        re_tracing::profile_function!();

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

        let mut analytics = ViewerAnalytics::new(&startup_options);
        analytics.on_viewer_started(&build_info, app_env);

        let mut space_view_class_registry = SpaceViewClassRegistry::default();
        if let Err(err) = populate_space_view_class_registry_with_builtin(
            &mut space_view_class_registry,
            state.app_options(),
        ) {
            re_log::error!(
                "Failed to populate Space View type registry with built-in Space Views: {}",
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

        let mut component_ui_registry = re_data_ui::create_component_ui_registry();
        re_viewport::blueprint::register_ui_components(&mut component_ui_registry);

        // TODO(emilk): `Instant::MIN` when we have our own `Instant` that supports it.;
        let long_time_ago = web_time::Instant::now()
            .checked_sub(web_time::Duration::from_secs(1_000_000_000))
            .unwrap_or(web_time::Instant::now());

        Self {
            build_info,
            startup_options,
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
            store_hub: Some(StoreHub::new()),
            toasts: toasts::Toasts::new(),
            memory_panel: Default::default(),
            memory_panel_open: false,

            style_panel_open: false,

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
    pub fn add_space_view_class<T: DynSpaceViewClass + Default + 'static>(
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
            SystemCommand::SetRecordingId(recording_id) => {
                store_hub.set_recording_id(recording_id);
            }
            SystemCommand::CloseRecordingId(recording_id) => {
                store_hub.remove_recording_id(&recording_id);
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

            SystemCommand::LoadStoreDb(entity_db) => {
                let store_id = entity_db.store_id().clone();
                store_hub.insert_recording(entity_db);
                store_hub.set_recording_id(store_id);
            }

            SystemCommand::ResetViewer => self.reset(store_hub, egui_ctx),
            SystemCommand::ResetBlueprint => {
                // By clearing the blueprint it will be re-populated with the defaults
                // at the beginning of the next frame.
                re_log::debug!("Reset blueprint");
                store_hub.clear_blueprint();
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

            SystemCommand::SetSelection(store_id, item) => {
                if let Some(rec_cfg) = self.state.recording_config_mut(&store_id) {
                    rec_cfg.selection_state.set_selection(item);
                } else {
                    re_log::debug!("Failed to select item {item:?}");
                }
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
                let cur_rec = store_context
                    .and_then(|ctx| ctx.recording)
                    .map(|rec| rec.store_id());
                if let Some(cur_rec) = cur_rec {
                    self.command_sender
                        .send_system(SystemCommand::CloseRecordingId(cur_rec.clone()));
                }
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
            UICommand::ToggleStylePanel => {
                self.style_panel_open ^= true;
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
                    .and_then(|store_context| store_context.recording)
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
                self.screenshotter.request_screenshot(egui_ctx);
            }
            #[cfg(not(target_arch = "wasm32"))]
            UICommand::PrintDatastore => {
                if let Some(ctx) = store_context {
                    if let Some(recording) = ctx.recording {
                        let table = recording.store().to_data_table();
                        match table {
                            Ok(table) => {
                                println!("{table}");
                            }
                            Err(err) => {
                                println!("{err}");
                            }
                        }
                    }
                }
            }
            #[cfg(not(target_arch = "wasm32"))]
            UICommand::ClearPrimaryCache => {
                if let Some(ctx) = store_context {
                    if let Some(recording) = ctx.recording {
                        recording.query_caches().clear();
                    }
                }
            }
            #[cfg(not(target_arch = "wasm32"))]
            UICommand::PrintPrimaryCache => {
                if let Some(ctx) = store_context {
                    if let Some(recording) = ctx.recording {
                        println!("{:?}", recording.query_caches());
                    }
                }
            }

            #[cfg(target_arch = "wasm32")]
            UICommand::CopyDirectLink => {
                self.run_copy_direct_link_command(store_context);
            }
        }
    }

    fn run_time_control_command(
        &mut self,
        store_context: Option<&StoreContext<'_>>,
        command: TimeControlCommand,
    ) {
        let Some(entity_db) = store_context.as_ref().and_then(|ctx| ctx.recording) else {
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
        let location = eframe::web::web_location();
        let mut href = location.origin;
        if location.host == "app.rerun.io" {
            // links to `app.rerun.io` can be made into permanent links:
            let path = if self.build_info.is_final() {
                // final release, use version tag
                format!("version/{}", self.build_info.version)
            } else {
                // not a final release, use commit hash
                format!("commit/{}", self.build_info.short_git_hash())
            };
            href = format!("{href}/{path}");
        }
        let direct_link = match store_context
            .and_then(|ctx| ctx.recording)
            .and_then(|rec| rec.data_source.as_ref())
        {
            Some(SmartChannelSource::RrdHttpStream { url }) => format!("{href}/?url={url}"),
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
                    self.re_ui(),
                    &self.startup_options.memory_limit,
                    gpu_resource_stats,
                    store_stats,
                );
            });
    }

    fn style_panel_ui(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        egui::SidePanel::left("style_panel")
            .default_width(300.0)
            .resizable(true)
            .frame(self.re_ui.top_panel_frame())
            .show_animated_inside(ui, self.style_panel_open, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ctx.settings_ui(ui);
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
                    frame,
                    self,
                    app_blueprint,
                    store_context,
                    gpu_resource_stats,
                    ui,
                );

                self.memory_panel_ui(ui, gpu_resource_stats, store_stats);

                self.style_panel_ui(egui_ctx, ui);

                if let Some(store_view) = store_context {
                    static EMPTY_ENTITY_DB: once_cell::sync::Lazy<EntityDb> =
                        once_cell::sync::Lazy::new(|| {
                            EntityDb::new(re_log_types::StoreId::from_string(
                                StoreKind::Recording,
                                "<EMPTY>".to_owned(),
                            ))
                        });

                    // We want the regular UI as soon as a blueprint is available (or, rather, an
                    // app ID is set). If no recording is available, we use a default, empty one.
                    // Note that EMPTY_STORE_DB is *not* part of the list of available recordings
                    // (StoreContext::alternate_recordings), which means that it's not displayed in
                    // the recordings UI.
                    let entity_db = if let Some(entity_db) = store_view.recording {
                        entity_db
                    } else {
                        &EMPTY_ENTITY_DB
                    };

                    // TODO(andreas): store the re_renderer somewhere else.
                    let egui_renderer = {
                        let render_state = frame.wgpu_render_state().unwrap();
                        &mut render_state.renderer.write()
                    };
                    if let Some(render_ctx) = egui_renderer
                        .callback_resources
                        .get_mut::<re_renderer::RenderContext>()
                    {
                        render_ctx.begin_frame();

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
                        );

                        render_ctx.before_submit();
                    }
                } else {
                    // There's nothing to show.
                    // We get here when
                    // A) there is nothing loaded
                    // B) we decided not to show the welcome screen, presumably because data is expected at any time now.
                    // The user can see the connection status in the top bar.
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
                        re_log::debug!("Data source {} has left", msg.source);
                    }
                    continue;
                }
            };

            let store_id = msg.store_id();

            let is_new_store = matches!(&msg, LogMsg::SetStoreInfo(_msg));

            let entity_db = store_hub.entity_db_mut(store_id);

            if entity_db.data_source.is_none() {
                entity_db.data_source = Some((*channel_source).clone());
            }

            if let Err(err) = entity_db.add(&msg) {
                re_log::error_once!("Failed to add incoming msg: {err}");
            };

            if is_new_store && entity_db.store_kind() == StoreKind::Recording {
                // Do analytics after ingesting the new message,
                // because thats when the `entity_db.store_info` is set,
                // which we use in the analytics call.
                self.analytics.on_open_recording(entity_db);
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
    fn reset(&mut self, store_hub: &mut StoreHub, egui_ctx: &egui::Context) {
        self.state = Default::default();
        store_hub.clear_blueprint();

        // Keep the style:
        let style = egui_ctx.style();
        egui_ctx.memory_mut(|mem| *mem = Default::default());
        egui_ctx.set_style((*style).clone());
    }

    pub fn recording_db(&self) -> Option<&EntityDb> {
        self.store_hub
            .as_ref()
            .and_then(|store_hub| store_hub.current_recording())
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

    /// This function implements a heuristic which determines when the welcome screen
    /// should show up.
    ///
    /// Why not always show it when no data is loaded?
    /// Because sometimes we expect data to arrive at any moment,
    /// and showing the wlecome screen for a few frames will just be an annoying flash
    /// in the users face.
    fn should_show_welcome_screen(&mut self, store_hub: &StoreHub) -> bool {
        // Don't show the welcome screen if we have actual data to display.
        if store_hub.current_recording().is_some() || store_hub.selected_application_id().is_some()
        {
            return false;
        }

        // Don't show the welcome screen if the `--skip-welcome-screen` flag was used (e.g. by the
        // Python SDK), until some data has been loaded and shown. This way, we *still* show the
        // welcome screen when the user closes all recordings after, e.g., running a Python example.
        if self.startup_options.skip_welcome_screen && !store_hub.was_recording_active() {
            return false;
        }

        let sources = self.rx.sources();

        if sources.is_empty() {
            return true;
        }

        // Here, we use the type of Receiver as a proxy for which kind of workflow the viewer is
        // being used in.
        for source in sources {
            match &*source {
                // No need for a welcome screen - data is coming soon!
                SmartChannelSource::File(_)
                | SmartChannelSource::RrdHttpStream { .. }
                | SmartChannelSource::Stdin => {
                    return false;
                }

                // The workflows associated with these sources typically do not require showing the
                // welcome screen until after some recording have been loaded and then closed.
                SmartChannelSource::RrdWebEventListener
                | SmartChannelSource::Sdk
                | SmartChannelSource::WsClient { .. } => {}

                // This might be the trickiest case. When running the bare executable, we want to show
                // the welcome screen (default, "new user" workflow). There are other case using Tcp
                // where it's not the case, including Python/C++ SDKs and possibly other, advanced used,
                // scenarios. In this cases, `--skip-welcome-screen` should be used.
                SmartChannelSource::TcpServer { .. } => {
                    return true;
                }
            }
        }

        false
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
            // TODO(#2579): implement web-storage for blueprints as well
            if let Some(hub) = &mut self.store_hub {
                match hub.gc_and_persist_app_blueprints(&self.state.app_options) {
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

        // TODO(andreas): store the re_renderer somewhere else.
        let gpu_resource_stats = {
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

        let store_stats = store_hub.stats(self.memory_panel.primary_cache_detailed_stats_enabled());

        // do early, before doing too many allocations
        self.memory_panel.update(&gpu_resource_stats, &store_stats);

        self.check_keyboard_shortcuts(egui_ctx);

        self.purge_memory_if_needed(&mut store_hub);

        self.state.cache.begin_frame();

        self.show_text_logs_as_notifications();
        self.receive_messages(&mut store_hub, egui_ctx);

        store_hub.gc_blueprints(self.app_options());

        store_hub.purge_empty();
        self.state.cleanup(&store_hub);

        file_saver_progress_ui(egui_ctx, &mut self.background_tasks); // toasts for background file saver

        // Heuristic to set the app_id to the welcome screen blueprint.
        // Must be called before `read_context` below.
        if self.should_show_welcome_screen(&store_hub) {
            store_hub.set_app_id(StoreHub::welcome_screen_app_id());
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

        // Frame time measurer - must be last
        self.frame_time_history.add(
            egui_ctx.input(|i| i.time),
            frame_start.elapsed().as_secs_f32(),
        );
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

#[cfg(not(target_arch = "wasm32"))]
fn save(
    app: &mut App,
    store_context: Option<&StoreContext<'_>>,
    loop_selection: Option<(re_entity_db::Timeline, re_log_types::TimeRangeF)>,
) {
    use crate::saving::save_database_to_file;

    let Some(entity_db) = store_context.as_ref().and_then(|view| view.recording) else {
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
        let f = match save_database_to_file(entity_db, path, loop_selection) {
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
