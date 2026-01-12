use std::str::FromStr as _;
use std::sync::Arc;

use egui::{FocusDirection, Key};
use itertools::Itertools as _;
use re_build_info::CrateVersion;
use re_capabilities::MainThreadToken;
use re_chunk::TimelineName;
use re_data_source::{AuthErrorHandler, FileContents, LogDataSource};
use re_entity_db::InstancePath;
use re_entity_db::entity_db::EntityDb;
use re_log_channel::{
    DataSourceMessage, DataSourceUiCommand, LogReceiver, LogReceiverSet, LogSource,
};
use re_log_types::{ApplicationId, FileSource, LogMsg, RecordingId, StoreId, StoreKind, TableMsg};
use re_redap_client::ConnectionRegistryHandle;
use re_renderer::WgpuResourcePoolStatistics;
use re_sdk_types::blueprint::components::{LoopMode, PlayState};
use re_ui::egui_ext::context_ext::ContextExt as _;
use re_ui::{ContextExt as _, UICommand, UICommandSender as _, UiExt as _, notifications};
use re_viewer_context::open_url::{OpenUrlOptions, ViewerOpenUrl, combine_with_base_url};
use re_viewer_context::store_hub::{BlueprintPersistence, StoreHub, StoreHubStats};
use re_viewer_context::{
    AppOptions, AsyncRuntimeHandle, AuthContext, BlueprintUndoState, CommandReceiver,
    CommandSender, ComponentUiRegistry, DisplayMode, EditRedapServerModalCommand,
    FallbackProviderRegistry, Item, NeedsRepaint, RecordingOrTable, StorageContext, StoreContext,
    SystemCommand, SystemCommandSender as _, TableStore, TimeControlCommand, ViewClass,
    ViewClassRegistry, ViewClassRegistryError, command_channel, sanitize_file_name,
};

use crate::AppState;
use crate::app_blueprint::{AppBlueprint, PanelStateOverrides};
use crate::app_blueprint_ctx::AppBlueprintCtx;
use crate::app_state::WelcomeScreenState;
use crate::background_tasks::BackgroundTasks;
use crate::event::ViewerEventDispatcher;
use crate::startup_options::StartupOptions;

// ----------------------------------------------------------------------------

/// Storage key used to store the last run Rerun version.
///
/// This is then used to detect if the user has recently upgraded Rerun.
const RERUN_VERSION_KEY: &str = "rerun.version";

const REDAP_TOKEN_KEY: &str = "rerun.redap_token";

#[cfg(not(target_arch = "wasm32"))]
const MIN_ZOOM_FACTOR: f32 = 0.2;
#[cfg(not(target_arch = "wasm32"))]
const MAX_ZOOM_FACTOR: f32 = 5.0;

#[cfg(target_arch = "wasm32")]
struct PendingFilePromise {
    recommended_store_id: Option<StoreId>,
    force_store_info: bool,
    promise: poll_promise::Promise<Vec<re_data_source::FileContents>>,
}

/// The Rerun Viewer as an [`eframe`] application.
pub struct App {
    #[allow(clippy::allow_attributes, dead_code)] // Unused on wasm32
    main_thread_token: MainThreadToken,
    build_info: re_build_info::BuildInfo,

    app_env: crate::AppEnvironment,

    startup_options: StartupOptions,
    start_time: web_time::Instant,
    ram_limit_warner: re_memory::RamLimitWarner,
    pub(crate) egui_ctx: egui::Context,
    screenshotter: crate::screenshotter::Screenshotter,

    #[cfg(target_arch = "wasm32")]
    pub(crate) popstate_listener: Option<crate::web_history::PopstateListener>,

    #[cfg(not(target_arch = "wasm32"))]
    profiler: re_tracing::Profiler,

    /// Listens to the local text log stream
    text_log_rx: std::sync::mpsc::Receiver<re_log::LogMsg>,

    component_ui_registry: ComponentUiRegistry,
    component_fallback_registry: FallbackProviderRegistry,

    rx_log: LogReceiverSet,

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

    /// Last time the latency was deemed interesting.
    ///
    /// Note that initializing with an "old" `Instant` won't work reliably cross platform
    /// since `Instant`'s counter may start at program start.
    pub(crate) latest_latency_interest: Option<web_time::Instant>,

    /// Measures how long a frame takes to paint
    pub(crate) frame_time_history: egui::util::History<f32>,

    /// Commands to run at the end of the frame.
    pub command_sender: CommandSender,
    command_receiver: CommandReceiver,
    cmd_palette: re_ui::CommandPalette,

    /// All known view types.
    view_class_registry: ViewClassRegistry,

    pub(crate) panel_state_overrides_active: bool,
    pub(crate) panel_state_overrides: PanelStateOverrides,

    reflection: re_types_core::reflection::Reflection,

    /// External interactions with the Viewer host (JS, custom egui app, notebook, etc.).
    pub event_dispatcher: Option<ViewerEventDispatcher>,

    connection_registry: ConnectionRegistryHandle,

    /// The async runtime that should be used for all asynchronous operations.
    ///
    /// Using the global tokio runtime should be avoided since:
    /// * we don't have a tokio runtime on web
    /// * we want the user to have full control over the runtime,
    ///   and not expect that a global runtime exists.
    async_runtime: AsyncRuntimeHandle,
}

impl App {
    pub fn new(
        main_thread_token: MainThreadToken,
        build_info: re_build_info::BuildInfo,
        app_env: crate::AppEnvironment,
        startup_options: StartupOptions,
        creation_context: &eframe::CreationContext<'_>,
        connection_registry: Option<ConnectionRegistryHandle>,
        tokio_runtime: AsyncRuntimeHandle,
    ) -> Self {
        Self::with_commands(
            main_thread_token,
            build_info,
            app_env,
            startup_options,
            creation_context,
            connection_registry,
            tokio_runtime,
            crate::register_text_log_receiver(),
            command_channel(),
        )
    }

    /// Create a viewer that receives new log messages over time
    #[expect(clippy::too_many_arguments)]
    pub fn with_commands(
        main_thread_token: MainThreadToken,
        build_info: re_build_info::BuildInfo,
        app_env: crate::AppEnvironment,
        startup_options: StartupOptions,
        creation_context: &eframe::CreationContext<'_>,
        connection_registry: Option<ConnectionRegistryHandle>,
        tokio_runtime: AsyncRuntimeHandle,
        text_log_rx: std::sync::mpsc::Receiver<re_log::LogMsg>,
        command_channel: (CommandSender, CommandReceiver),
    ) -> Self {
        re_tracing::profile_function!();

        {
            let command_sender = command_channel.0.clone();
            re_auth::credentials::subscribe_auth_changes(move |user| {
                command_sender.send_system(SystemCommand::OnAuthChanged(
                    user.map(|user| AuthContext { email: user.email }),
                ));
            });
        }

        let connection_registry = connection_registry
            .unwrap_or_else(re_redap_client::ConnectionRegistry::new_with_stored_credentials);

        if let Some(storage) = creation_context.storage
            && let Some(tokens) = eframe::get_value(storage, REDAP_TOKEN_KEY)
        {
            connection_registry.load_tokens(tokens);
        }

        let mut state: AppState = if startup_options.persist_state {
            creation_context.storage
                .and_then(|storage| {
                    // This re-implements: `eframe::get_value` so we can customize the warning message.
                    // TODO(#2849): More thorough error-handling.
                    let value = storage.get_string(eframe::APP_KEY)?;
                    match ron::from_str(&value) {
                        Ok(value) => Some(value),
                        Err(err) => {
                            re_log::warn!("Failed to restore application state. This is expected if you have just upgraded Rerun versions.");
                            re_log::debug!("Failed to decode RON for app state: {err}");
                            None
                        }
                    }
                })
                .unwrap_or_default()
        } else {
            AppState::default()
        };

        if startup_options.persist_state {
            // Check if the user has recently upgraded Rerun.
            if let Some(storage) = creation_context.storage {
                let current_version = build_info.version;
                let previous_version: Option<CrateVersion> =
                    storage.get_string(RERUN_VERSION_KEY).and_then(|version| {
                        // `CrateVersion::try_parse` is `const` (for good reasons), and needs a `&'static str`.
                        // In order to accomplish this, we need to leak the string here.
                        let version = Box::leak(version.into_boxed_str());
                        CrateVersion::try_parse(version).ok()
                    });

                if previous_version
                    .is_none_or(|previous_version| previous_version < CrateVersion::new(0, 24, 0))
                {
                    re_log::debug!(
                        "Upgrading from {} to {}.",
                        previous_version.map_or_else(|| "<unknown>".to_owned(), |v| v.to_string()),
                        current_version
                    );
                    // We used to have Dark as the hard-coded theme preference. Let's change that!
                    creation_context
                        .egui_ctx
                        .options_mut(|o| o.theme_preference = egui::ThemePreference::System);
                }
            }
        }

        if let Some(video_decoder_hw_acceleration) = startup_options.video_decoder_hw_acceleration {
            state.app_options.video.hw_acceleration = video_decoder_hw_acceleration;
        }

        if app_env.is_test() {
            // Disable certain labels/warnings/etc that would be flaky or not CI-runner-agnostic in snapshot tests.
            state.app_options.show_metrics = false;
        }

        let mut component_fallback_registry =
            re_component_fallbacks::create_component_fallback_registry();

        let view_class_registry =
            crate::default_views::create_view_class_registry(&mut component_fallback_registry)
                .unwrap_or_else(|err| {
                    re_log::error!("Failed to create view class registry: {err}");
                    Default::default()
                });

        #[allow(clippy::allow_attributes, unused_mut, clippy::needless_update)]
        // false positive on web
        let mut screenshotter = crate::screenshotter::Screenshotter::default();

        #[cfg(not(target_arch = "wasm32"))]
        if let Some(screenshot_path) = startup_options.screenshot_to_path_then_quit.clone() {
            screenshotter.screenshot_to_path_then_quit(&creation_context.egui_ctx, screenshot_path);
        }

        let (command_sender, command_receiver) = command_channel;

        let mut component_ui_registry = re_component_ui::create_component_ui_registry();
        re_data_ui::register_component_uis(&mut component_ui_registry);

        let (_adapter_backend, _device_tier) = creation_context.wgpu_render_state.as_ref().map_or(
            (
                wgpu::Backend::Noop,
                re_renderer::device_caps::DeviceCapabilityTier::Limited,
            ),
            |render_state| {
                let egui_renderer = render_state.renderer.read();
                let render_ctx = egui_renderer
                    .callback_resources
                    .get::<re_renderer::RenderContext>();

                (
                    render_state.adapter.get_info().backend,
                    render_ctx.map_or(
                        re_renderer::device_caps::DeviceCapabilityTier::Limited,
                        |ctx| ctx.device_caps().tier,
                    ),
                )
            },
        );

        #[cfg(feature = "analytics")]
        if let Some(analytics) = re_analytics::Analytics::global_or_init() {
            use crate::viewer_analytics::event;

            analytics.record(event::identify(
                analytics.config(),
                build_info.clone(),
                &app_env,
            ));
            analytics.record(event::viewer_started(
                &app_env,
                &creation_context.egui_ctx,
                _adapter_backend,
                _device_tier,
            ));
        }

        let panel_state_overrides = startup_options.panel_state_overrides;

        let reflection = re_sdk_types::reflection::generate_reflection().unwrap_or_else(|err| {
            re_log::error!(
                "Failed to create list of serialized default values for components: {err}"
            );
            Default::default()
        });

        let event_dispatcher = startup_options
            .on_event
            .clone()
            .map(ViewerEventDispatcher::new);

        if !state.redap_servers.is_empty() {
            command_sender.send_ui(UICommand::ExpandBlueprintPanel);
        }

        creation_context.egui_ctx.on_end_pass(
            "remove copied text formatting",
            Arc::new(|ctx| {
                ctx.output_mut(|o| {
                    for command in &mut o.commands {
                        if let egui::output::OutputCommand::CopyText(text) = command {
                            *text = re_format::remove_number_formatting(text);
                        }
                    }
                });
            }),
        );

        {
            // TODO(emilk/egui#7659): This is a workaround consuming the Space/Arrow keys so we can
            // use them as timeline shortcuts. Egui's built in behavior is to interact with focus,
            // and we don't want that.
            // But of course text edits should still get it so we use this ugly hack to check if
            // a text edit is focused.
            let command_sender = command_sender.clone();
            creation_context.egui_ctx.on_begin_pass(
                "filter space key",
                Arc::new(move |ctx| {
                    if !ctx.text_edit_focused() {
                        let conflicting_commands = [
                            UICommand::PlaybackTogglePlayPause,
                            UICommand::PlaybackBeginning,
                            UICommand::PlaybackEnd,
                            UICommand::PlaybackForwardFast,
                            UICommand::PlaybackBackFast,
                            UICommand::PlaybackStepForward,
                            UICommand::PlaybackStepBack,
                            UICommand::PlaybackForward,
                            UICommand::PlaybackBack,
                        ];

                        let os = ctx.os();
                        let mut reset_focus_direction = false;
                        ctx.input_mut(|i| {
                            for command in conflicting_commands {
                                for shortcut in command.kb_shortcuts(os) {
                                    if i.consume_shortcut(&shortcut) {
                                        if shortcut.logical_key == Key::ArrowLeft
                                            || shortcut.logical_key == Key::ArrowRight
                                        {
                                            reset_focus_direction = true;
                                        }
                                        command_sender.send_ui(command);
                                    }
                                }
                            }
                        });

                        if reset_focus_direction {
                            // Additionally, we need to revert the focus direction on ArrowLeft/Right
                            // keys to prevent the focus change for timeline shortcuts
                            ctx.memory_mut(|mem| {
                                mem.move_focus(FocusDirection::None);
                            });
                        }
                    }
                }),
            );
        }

        Self {
            main_thread_token,
            build_info,
            app_env,
            startup_options,
            start_time: web_time::Instant::now(),
            ram_limit_warner: re_memory::RamLimitWarner::warn_at_fraction_of_max(0.75),
            egui_ctx: creation_context.egui_ctx.clone(),
            screenshotter,

            #[cfg(target_arch = "wasm32")]
            popstate_listener: None,

            #[cfg(not(target_arch = "wasm32"))]
            profiler: Default::default(),

            text_log_rx,
            component_ui_registry,
            component_fallback_registry,
            rx_log: Default::default(),

            #[cfg(target_arch = "wasm32")]
            open_files_promise: Default::default(),

            state,
            background_tasks: Default::default(),
            store_hub: Some(StoreHub::new(
                blueprint_loader(),
                &crate::app_blueprint::setup_welcome_screen_blueprint,
            )),
            notifications: notifications::NotificationUi::new(creation_context.egui_ctx.clone()),

            memory_panel: Default::default(),
            memory_panel_open: false,

            egui_debug_panel_open: false,

            latest_latency_interest: None,

            frame_time_history: egui::util::History::new(1..100, 0.5),

            command_sender,
            command_receiver,
            cmd_palette: Default::default(),

            view_class_registry,

            panel_state_overrides_active: true,
            panel_state_overrides,

            reflection,

            event_dispatcher,

            connection_registry,
            async_runtime: tokio_runtime,
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn set_profiler(&mut self, profiler: re_tracing::Profiler) {
        self.profiler = profiler;
    }

    pub fn connection_registry(&self) -> &ConnectionRegistryHandle {
        &self.connection_registry
    }

    pub fn set_examples_manifest_url(&mut self, url: String) {
        re_log::info!("Using manifest_url={url:?}");
        self.state.set_examples_manifest_url(&self.egui_ctx, url);
    }

    pub fn build_info(&self) -> &re_build_info::BuildInfo {
        &self.build_info
    }

    pub fn startup_options(&self) -> &StartupOptions {
        &self.startup_options
    }

    pub fn app_options(&self) -> &AppOptions {
        self.state.app_options()
    }

    pub fn app_options_mut(&mut self) -> &mut AppOptions {
        self.state.app_options_mut()
    }

    pub fn app_env(&self) -> &crate::AppEnvironment {
        &self.app_env
    }

    /// Open a content URL in the viewer.
    pub fn open_url_or_file(&self, url: &str) {
        match ViewerOpenUrl::from_str(url) {
            Ok(url) => {
                url.open(
                    &self.egui_ctx,
                    &OpenUrlOptions {
                        follow_if_http: false,
                        select_redap_source_when_loaded: true,
                        show_loader: true,
                    },
                    &self.command_sender,
                );
            }
            Err(err) => {
                if err.to_string().contains(url) {
                    re_log::error!("{err}");
                } else {
                    re_log::error!("Failed to open URL {url}: {err}");
                }
            }
        }
    }

    pub fn is_screenshotting(&self) -> bool {
        self.screenshotter.is_screenshotting()
    }

    #[expect(clippy::needless_pass_by_ref_mut)]
    pub fn add_log_receiver(&mut self, rx: LogReceiver) {
        re_log::debug!("Adding new log receiver: {}", rx.source());

        // Make sure we wake up when a new message is available:
        rx.set_waker({
            let egui_ctx = self.egui_ctx.clone();
            move || {
                // Spend a few more milliseconds decoding incoming messages,
                // then trigger a repaint (https://github.com/rerun-io/rerun/issues/963):
                egui_ctx.request_repaint_after(std::time::Duration::from_millis(10));
            }
        });

        // Add unknown redap servers.
        //
        // Otherwise we end up in a situation where we have a data from an unknown server,
        // which is unnecessary and can get us into a strange ui state.
        if let LogSource::RedapGrpcStream { uri, .. } = rx.source() {
            self.command_sender
                .send_system(SystemCommand::AddRedapServer(uri.origin.clone()));
        }

        self.rx_log.add(rx);
    }

    /// Update the active [`re_viewer_context::TimeControl`]. And if the blueprint inspection
    /// panel is open, also open that time control.
    fn move_time(&mut self) {
        if let Some(store_hub) = &self.store_hub
            && let Some(store_id) = store_hub.active_store_id()
            && let Some(blueprint) = store_hub.active_blueprint_for_app(store_id.application_id())
        {
            let default_blueprint = store_hub.default_blueprint_for_app(store_id.application_id());

            let blueprint_query = self
                .state
                .get_blueprint_query_for_viewer(blueprint)
                .unwrap_or_else(|| {
                    re_chunk::LatestAtQuery::latest(re_viewer_context::blueprint_timeline())
                });

            let bp_ctx = AppBlueprintCtx {
                command_sender: &self.command_sender,
                current_blueprint: blueprint,
                default_blueprint,
                blueprint_query,
            };

            let dt = self.egui_ctx.input(|i| i.stable_dt);
            if let Some(recording) = store_hub.active_recording() {
                // Are we still connected to the data source for the current store?
                let more_data_is_coming =
                    recording.data_source.as_ref().is_some_and(|store_source| {
                        self.rx_log
                            .sources()
                            .iter()
                            .any(|s| s.as_ref() == store_source)
                    });

                let time_ctrl = self.state.time_control_mut(recording, &bp_ctx);

                // The state diffs are used to trigger callbacks if they are configured.
                // If there's no active recording, we should not trigger any callbacks, but since there's an active recording here,
                // we want to diff state changes.
                let should_diff_state = true;
                let response = time_ctrl.update(
                    recording.timeline_histograms(),
                    dt,
                    more_data_is_coming,
                    should_diff_state,
                    Some(&bp_ctx),
                );

                if response.needs_repaint == NeedsRepaint::Yes {
                    self.egui_ctx.request_repaint();
                }

                handle_time_ctrl_event(recording, self.event_dispatcher.as_ref(), &response);
            }

            if self.app_options().inspect_blueprint_timeline {
                let more_data_is_coming = true;
                let should_diff_state = false;
                // We ignore most things from the time control response for the blueprint but still
                // need to repaint if requested.
                let re_viewer_context::TimeControlResponse {
                    needs_repaint,
                    playing_change: _,
                    timeline_change: _,
                    time_change: _,
                } = self.state.blueprint_time_control.update(
                    bp_ctx.current_blueprint.timeline_histograms(),
                    dt,
                    more_data_is_coming,
                    should_diff_state,
                    None::<&AppBlueprintCtx<'_>>,
                );

                if needs_repaint == NeedsRepaint::Yes {
                    self.egui_ctx.request_repaint();
                }

                let undo_state = self
                    .state
                    .blueprint_undo_state
                    .entry(blueprint.store_id().clone())
                    .or_default();
                // Apply changes to the blueprint time to the undo-state:
                if self.state.blueprint_time_control.play_state() == PlayState::Following {
                    undo_state.redo_all();
                } else if let Some(time) = self.state.blueprint_time_control.time_int() {
                    undo_state.set_redo_time(time);
                }
            }
        }
    }

    pub fn msg_receive_set(&self) -> &LogReceiverSet {
        &self.rx_log
    }

    /// Adds a new view class to the viewer.
    pub fn add_view_class<T: ViewClass + Default + 'static>(
        &mut self,
    ) -> Result<(), ViewClassRegistryError> {
        self.view_class_registry
            .add_class::<T>(&mut self.component_fallback_registry)
    }

    /// Accesses the view class registry which can be used to extend the Viewer.
    ///
    /// **WARNING:** Many parts or the viewer assume that all views & visualizers are registered before the first frame is rendered.
    /// Doing so later in the application life cycle may cause unexpected behavior.
    pub fn view_class_registry(&mut self) -> &mut ViewClassRegistry {
        &mut self.view_class_registry
    }

    pub fn component_fallback_registry(&mut self) -> &mut FallbackProviderRegistry {
        &mut self.component_fallback_registry
    }

    fn check_keyboard_shortcuts(&self, egui_ctx: &egui::Context) {
        if let Some(cmd) = UICommand::listen_for_kb_shortcut(egui_ctx) {
            self.command_sender.send_ui(cmd);
        }
    }

    fn run_pending_system_commands(&mut self, store_hub: &mut StoreHub, egui_ctx: &egui::Context) {
        re_tracing::profile_function!();
        while let Some((from_where, cmd)) = self.command_receiver.recv_system() {
            self.run_system_command(from_where, cmd, store_hub, egui_ctx);
        }
    }

    fn run_pending_ui_commands(
        &mut self,
        egui_ctx: &egui::Context,
        app_blueprint: &AppBlueprint<'_>,
        storage_context: &StorageContext<'_>,
        store_context: Option<&StoreContext<'_>>,
        display_mode: &DisplayMode,
    ) {
        while let Some(cmd) = self.command_receiver.recv_ui() {
            self.run_ui_command(
                egui_ctx,
                app_blueprint,
                storage_context,
                store_context,
                display_mode,
                cmd,
            );
        }
    }

    /// If we're on web and use web history this updates the
    /// web address bar and updates history.
    ///
    /// Otherwise this updates the viewer tracked history.
    fn update_history(&mut self, store_hub: &StoreHub) {
        if !self.startup_options().web_history_enabled() {
            self.update_viewer_history(store_hub);
        } else {
            // We don't want to spam the web history API with changes, because
            // otherwise it will start complaining about it being an insecure
            // operation.
            //
            // This is a kind of hacky way to fix that: If there are currently any
            // inputs, don't update the web address bar. This works for most cases
            // because you need to hold down pointer to aggressively scrub, need to
            // hold down key inputs to quickly step through the timeline.
            #[cfg(target_arch = "wasm32")]
            if !self.egui_ctx.is_using_pointer()
                && self
                    .egui_ctx
                    .input(|input| !input.any_touches() && input.keys_down.is_empty())
            {
                self.update_web_history(store_hub);
            }
        }
    }

    /// Updates the viewer tracked history
    fn update_viewer_history(&mut self, store_hub: &StoreHub) {
        let time_ctrl = store_hub
            .active_recording()
            .and_then(|db| self.state.time_control(db.store_id()));

        let display_mode = self.state.navigation.current();
        let selection = self.state.selection_state.selected_items();

        let Ok(url) =
            ViewerOpenUrl::from_context_expanded(store_hub, display_mode, time_ctrl, selection)
        else {
            return;
        };

        self.state.history.update_current_url(url);
    }

    /// Updates the web address and web history.
    #[cfg(target_arch = "wasm32")]
    fn update_web_history(&self, store_hub: &StoreHub) {
        let time_ctrl = store_hub
            .active_recording()
            .and_then(|db| self.state.time_control(db.store_id()));

        let display_mode = self.state.navigation.current();
        let selection = self.state.selection_state.selected_items();

        let Ok(url) =
            ViewerOpenUrl::from_context_expanded(store_hub, display_mode, time_ctrl, selection)
                .map(|mut url| {
                    // We don't want to update the url while playing, so we use the last paused time.
                    if let Some(fragment) = url.fragment_mut() {
                        fragment.when = time_ctrl.and_then(|time_ctrl| {
                            Some((
                                *time_ctrl.timeline_name(),
                                re_log_types::TimeCell {
                                    typ: time_ctrl.time_type()?,
                                    value: time_ctrl.last_paused_time()?.floor().into(),
                                },
                            ))
                        });
                    }

                    url
                })
                // History entries expect the url parameter, not the full url, therefore don't pass a base url.
                .and_then(|url| url.sharable_url(None))
        else {
            return;
        };

        re_log::trace!("Updating navigation bar");

        use crate::web_history::{HistoryEntry, HistoryExt as _, history};
        use crate::web_tools::JsResultExt as _;

        /// Returns the url without the fragment
        fn strip_fragment(url: &str) -> &str {
            // Split by url code for '#', which is used for fragments.
            url.rsplit_once("%23").map_or(url, |(url, _)| url)
        }

        if let Some(history) = history().ok_or_log_js_error() {
            let current_entry = history.current_entry().ok_or_log_js_error().flatten();
            let new_entry = HistoryEntry::new(url);
            if Some(&new_entry) != current_entry.as_ref() {
                // If only the fragment has changed, we replace history instead of pushing it.
                if current_entry
                    .and_then(|entry| {
                        Some((
                            entry.to_query_string().ok_or_log_js_error()?,
                            new_entry.to_query_string().ok_or_log_js_error()?,
                        ))
                    })
                    .is_some_and(|(current, new)| strip_fragment(&current) == strip_fragment(&new))
                {
                    history.replace_entry(new_entry).ok_or_log_js_error();
                } else {
                    history.push_entry(new_entry).ok_or_log_js_error();
                }
            }
        }
    }

    fn run_system_command(
        &mut self,
        sent_from: &std::panic::Location<'_>, // Who sent this command? Useful for debugging!
        cmd: SystemCommand,
        store_hub: &mut StoreHub,
        egui_ctx: &egui::Context,
    ) {
        re_tracing::profile_function!(cmd.debug_name());

        match cmd {
            SystemCommand::TimeControlCommands {
                store_id,
                time_commands,
            } => {
                match store_id.kind() {
                    StoreKind::Recording => {
                        store_hub.set_active_recording_id(store_id.clone()); // Switch to this recording
                        let (storage_ctx, store_ctx) = store_hub.read_context(); // Materialize the target blueprint on-demand
                        if let Some(store_ctx) = store_ctx {
                            let target_blueprint = store_ctx.blueprint;
                            let blueprint_query =
                                self.state.blueprint_query_for_viewer(target_blueprint);

                            let blueprint_ctx = AppBlueprintCtx {
                                command_sender: &self.command_sender,
                                current_blueprint: target_blueprint,
                                default_blueprint: storage_ctx
                                    .hub
                                    .default_blueprint_for_app(store_id.application_id()),
                                blueprint_query,
                            };

                            let time_ctrl = self
                                .state
                                .time_control_mut(store_ctx.recording, &blueprint_ctx);

                            let response = time_ctrl.handle_time_commands(
                                Some(&blueprint_ctx),
                                store_ctx.recording.timeline_histograms(),
                                &time_commands,
                            );

                            if response.needs_repaint == NeedsRepaint::Yes {
                                self.egui_ctx.request_repaint();
                            }

                            handle_time_ctrl_event(
                                store_ctx.recording,
                                self.event_dispatcher.as_ref(),
                                &response,
                            );
                        } else {
                            re_log::error!(
                                "Skipping time control command because of missing blueprint"
                            );
                        }
                    }
                    StoreKind::Blueprint => {
                        if let Some(target_store) = store_hub.store_bundle().get(&store_id) {
                            let blueprint_ctx: Option<&AppBlueprintCtx<'_>> = None;
                            let response = self.state.blueprint_time_control.handle_time_commands(
                                blueprint_ctx,
                                target_store.timeline_histograms(),
                                &time_commands,
                            );

                            if response.needs_repaint == NeedsRepaint::Yes {
                                self.egui_ctx.request_repaint();
                            }
                        }
                    }
                }
            }
            SystemCommand::SetUrlFragment { store_id, fragment } => {
                // This adds new system commands, which will be handled later in the loop.
                self.go_to_dataset_data(store_id, fragment);
            }
            SystemCommand::CopyViewerUrl(url) => {
                if cfg!(target_arch = "wasm32") {
                    match combine_with_base_url(
                        self.startup_options.web_viewer_base_url().as_ref(),
                        [url],
                    ) {
                        Ok(url) => {
                            self.copy_text(url);
                        }
                        Err(err) => {
                            re_log::error!("{err}");
                        }
                    }
                } else {
                    self.copy_text(url);
                }
            }
            SystemCommand::ActivateApp(app_id) => {
                store_hub.set_active_app(app_id);
                if let Some(recording_id) = store_hub.active_store_id() {
                    self.state
                        .navigation
                        .replace(DisplayMode::LocalRecordings(recording_id.clone()));
                } else {
                    self.state.navigation.reset();
                }
            }

            SystemCommand::CloseApp(app_id) => {
                store_hub.close_app(&app_id);
            }

            SystemCommand::ActivateRecordingOrTable(entry) => {
                match &entry {
                    RecordingOrTable::Recording { store_id } => {
                        store_hub.set_active_recording_id(store_id.clone());
                    }
                    RecordingOrTable::Table { .. } => {}
                }
                self.state.navigation.replace(entry.display_mode());
            }

            SystemCommand::CloseRecordingOrTable(entry) => {
                // TODO(#9464): Find a better successor here.

                let data_source = match &entry {
                    RecordingOrTable::Recording { store_id } => {
                        store_hub.entity_db_mut(store_id).data_source.clone()
                    }
                    RecordingOrTable::Table { .. } => None,
                };
                if let Some(data_source) = data_source {
                    // Only certain sources should be closed.
                    #[expect(clippy::match_same_arms)]
                    let should_close = match &data_source {
                        // Specific files should stop streaming when closing them.
                        LogSource::File(_) => true,

                        // Specific HTTP streams should stop streaming when closing them.
                        LogSource::RrdHttpStream { .. } => true,

                        // Specific GRPC streams should stop streaming when closing them.
                        // TODO(#10967): We still stream in some data after that.
                        LogSource::RedapGrpcStream { .. } => true,

                        // Don't close generic connections (like to an SDK) that may feed in different recordings over time.
                        LogSource::RrdWebEvent
                        | LogSource::JsChannel { .. }
                        | LogSource::Sdk
                        | LogSource::Stdin
                        | LogSource::MessageProxy(_) => false,
                    };

                    if should_close {
                        self.rx_log.retain(|r| r.source() != &data_source);
                    }
                }

                store_hub.remove(&entry);
            }

            SystemCommand::CloseAllEntries => {
                self.state.navigation.reset();
                store_hub.clear_entries();

                // Stop receiving into the old recordings.
                // This is most important when going back to the example screen by using the "Back"
                // button in the browser, and there is still a connection downloading an .rrd.
                // That's the case of `LogSource::RrdHttpStream`.
                // TODO(emilk): exactly what things get kept and what gets cleared?
                self.rx_log.retain(|r| match r.source() {
                    LogSource::File(_) | LogSource::RrdHttpStream { .. } => false,

                    LogSource::JsChannel { .. }
                    | LogSource::RrdWebEvent
                    | LogSource::Sdk
                    | LogSource::RedapGrpcStream { .. }
                    | LogSource::MessageProxy { .. }
                    | LogSource::Stdin => true,
                });
            }

            SystemCommand::AddReceiver(rx) => {
                re_log::debug!("Received AddReceiver");
                self.add_log_receiver(rx);
            }

            SystemCommand::ChangeDisplayMode(display_mode) => {
                if &display_mode == self.state.navigation.current() {
                    return;
                }

                // Suppress loading screen if we're loading a recording that's already loaded, even if only partially.
                if let DisplayMode::Loading(source) = &display_mode
                    && let Some(re_uri::RedapUri::DatasetData(dataset_uri)) = source.redap_uri()
                    && store_hub
                        .store_bundle()
                        .entity_dbs()
                        .any(|db| db.store_id() == &dataset_uri.store_id())
                {
                    return;
                }

                if matches!(display_mode, DisplayMode::Loading(_)) {
                    self.state
                        .selection_state
                        .set_selection(re_viewer_context::ItemCollection::default());
                }
                self.state.navigation.replace(display_mode);

                egui_ctx.request_repaint(); // Make sure we actually see the new mode.
            }

            SystemCommand::OpenSettings => {
                self.state
                    .navigation
                    .replace(DisplayMode::Settings(Box::new(
                        self.state.navigation.current().clone(),
                    )));

                #[cfg(feature = "analytics")]
                re_analytics::record(|| re_analytics::event::SettingsOpened {});
            }

            SystemCommand::OpenChunkStoreBrowser => match self.state.navigation.current() {
                DisplayMode::LocalRecordings(_)
                | DisplayMode::RedapEntry(_)
                | DisplayMode::RedapServer(_) => {
                    self.state
                        .navigation
                        .replace(DisplayMode::ChunkStoreBrowser(Box::new(
                            self.state.navigation.current().clone(),
                        )));
                }

                DisplayMode::ChunkStoreBrowser(_)
                | DisplayMode::Settings(_)
                | DisplayMode::Loading(_)
                | DisplayMode::LocalTable(_) => {
                    re_log::debug!(
                        "Cannot activate chunk store browser from current display mode: {:?}",
                        self.state.navigation.current()
                    );
                }
            },

            SystemCommand::ResetDisplayMode => {
                self.state.navigation.reset();

                egui_ctx.request_repaint(); // Make sure we actually see the new mode.
            }

            SystemCommand::AddRedapServer(origin) => {
                if origin == *re_redap_browser::EXAMPLES_ORIGIN {
                    return;
                }
                if self.state.redap_servers.has_server(&origin) {
                    return;
                }

                self.state.redap_servers.add_server(origin.clone());

                if self
                    .store_hub
                    .as_ref()
                    .is_none_or(|store_hub| store_hub.active_recording_or_table().is_none())
                {
                    self.state
                        .navigation
                        .replace(DisplayMode::RedapServer(origin));
                }
                self.command_sender.send_ui(UICommand::ExpandBlueprintPanel);
            }

            SystemCommand::EditRedapServerModal(command) => {
                self.state.redap_servers.open_edit_server_modal(command);
            }

            SystemCommand::LoadDataSource(data_source) => {
                self.load_data_source(store_hub, egui_ctx, &data_source);
            }

            SystemCommand::ResetViewer => self.reset_viewer(store_hub, egui_ctx),
            SystemCommand::ClearActiveBlueprintAndEnableHeuristics => {
                re_log::debug!("Clear and generate new blueprint");
                store_hub.clear_active_blueprint_and_generate();
                egui_ctx.request_repaint(); // Many changes take a frame delay to show up.
            }
            SystemCommand::ClearActiveBlueprint => {
                // By clearing the blueprint the default blueprint will be restored
                // at the beginning of the next frame.
                re_log::debug!("Reset blueprint to default");
                store_hub.clear_active_blueprint();
                egui_ctx.request_repaint(); // Many changes take a frame delay to show up.
            }

            SystemCommand::AppendToStore(store_id, chunks) => {
                re_log::trace!(
                    "{}:{} Update {} entities: {}",
                    sent_from.file(),
                    sent_from.line(),
                    store_id.kind(),
                    chunks.iter().map(|c| c.entity_path()).join(", ")
                );

                let db = store_hub.entity_db_mut(&store_id);

                // No need to clear undo buffer if we're just appending static data.
                //
                // It would be nice to be able to undo edits to a recording, but
                // we haven't implemented that yet.
                if store_id.is_blueprint() && chunks.iter().any(|c| !c.is_static()) {
                    self.state
                        .blueprint_undo_state
                        .entry(store_id.clone())
                        .or_default()
                        .clear_redo_buffer(db);

                    if self.app_options().inspect_blueprint_timeline {
                        self.command_sender
                            .send_system(SystemCommand::TimeControlCommands {
                                store_id,
                                time_commands: vec![TimeControlCommand::SetPlayState(
                                    PlayState::Following,
                                )],
                            });
                    }
                }

                for chunk in chunks {
                    match db.add_chunk(&Arc::new(chunk)) {
                        Ok(_store_events) => {}
                        Err(err) => {
                            re_log::warn_once!("Failed to append chunk: {err}");
                        }
                    }
                }
            }

            SystemCommand::UndoBlueprint { blueprint_id } => {
                let inspect_blueprint_timeline = self.app_options().inspect_blueprint_timeline;
                let blueprint_db = store_hub.entity_db_mut(&blueprint_id);
                let undo_state = self
                    .state
                    .blueprint_undo_state
                    .entry(blueprint_id.clone())
                    .or_default();

                undo_state.undo(blueprint_db);

                // Update blueprint inspector timeline.
                if inspect_blueprint_timeline {
                    if let Some(redo_time) = undo_state.redo_time() {
                        self.command_sender
                            .send_system(SystemCommand::TimeControlCommands {
                                store_id: blueprint_id,
                                time_commands: vec![
                                    TimeControlCommand::SetPlayState(PlayState::Paused),
                                    TimeControlCommand::SetTime(redo_time.into()),
                                ],
                            });
                    } else {
                        self.command_sender
                            .send_system(SystemCommand::TimeControlCommands {
                                store_id: blueprint_id,
                                time_commands: vec![TimeControlCommand::SetPlayState(
                                    PlayState::Following,
                                )],
                            });
                    }
                }
            }
            SystemCommand::RedoBlueprint { blueprint_id } => {
                let inspect_blueprint_timeline = self.app_options().inspect_blueprint_timeline;
                let undo_state = self
                    .state
                    .blueprint_undo_state
                    .entry(blueprint_id.clone())
                    .or_default();

                undo_state.redo();

                // Update blueprint inspector timeline.
                if inspect_blueprint_timeline {
                    if let Some(redo_time) = undo_state.redo_time() {
                        self.command_sender
                            .send_system(SystemCommand::TimeControlCommands {
                                store_id: blueprint_id,
                                time_commands: vec![
                                    TimeControlCommand::SetPlayState(PlayState::Paused),
                                    TimeControlCommand::SetTime(redo_time.into()),
                                ],
                            });
                    } else {
                        self.command_sender
                            .send_system(SystemCommand::TimeControlCommands {
                                store_id: blueprint_id,
                                time_commands: vec![TimeControlCommand::SetPlayState(
                                    PlayState::Following,
                                )],
                            });
                    }
                }
            }

            SystemCommand::DropEntity(blueprint_id, entity_path) => {
                let blueprint_db = store_hub.entity_db_mut(&blueprint_id);
                blueprint_db.drop_entity_path_recursive(&entity_path);
            }

            #[cfg(debug_assertions)]
            SystemCommand::EnableInspectBlueprintTimeline(show) => {
                self.app_options_mut().inspect_blueprint_timeline = show;
            }

            SystemCommand::SetSelection(set) => {
                if let Some(item) = set.selection.single_item() {
                    // If the selected item has its own page, switch to it.
                    if let Some(display_mode) = DisplayMode::from_item(item) {
                        if let DisplayMode::LocalRecordings(store_id) = &display_mode {
                            store_hub.set_active_recording_id(store_id.clone());
                        }
                        self.state.navigation.replace(display_mode);
                    }
                }

                self.state.selection_state.set_selection(set);
                egui_ctx.request_repaint(); // Make sure we actually see the new selection.
            }

            SystemCommand::SetFocus(item) => {
                self.state.focused_item = Some(item);
            }

            SystemCommand::ShowNotification(notification) => {
                self.notifications.add(notification);
            }

            #[cfg(not(target_arch = "wasm32"))]
            SystemCommand::FileSaver(file_saver) => {
                if let Err(err) = self.background_tasks.spawn_file_saver(file_saver) {
                    re_log::error!("Failed to save file: {err}");
                }
            }

            SystemCommand::OnAuthChanged(auth) => {
                self.state.auth_state = auth;
            }

            SystemCommand::SetAuthCredentials {
                access_token,
                email,
            } => {
                let credentials =
                    match re_auth::oauth::Credentials::try_new(access_token, None, email) {
                        Ok(credentials) => credentials,
                        Err(err) => {
                            re_log::error!("Failed to create credentials: {err}");
                            return;
                        }
                    };
                if let Err(err) = credentials.ensure_stored() {
                    re_log::error!("Failed to store credentials: {err}");
                }
            }
            SystemCommand::Logout => {
                if let Err(err) = re_auth::oauth::clear_credentials() {
                    re_log::error!("Failed to logout: {err}");
                }
                self.state.redap_servers.logout();
            }
        }
    }

    pub fn auth_error_handler(sender: CommandSender) -> AuthErrorHandler {
        Arc::new(move |url, _err| {
            sender.send_system(SystemCommand::EditRedapServerModal(
                EditRedapServerModalCommand {
                    origin: url.origin.clone(),
                    open_on_success: Some(url.to_string()),
                    title: Some("Authenticate to see this recording".to_owned()),
                },
            ));
        })
    }

    /// Loads a data source into the viewer.
    ///
    /// Tries to detect whether the datasource is already present (either still streaming in or already loaded),
    /// and if so, will not load the data again.
    /// Instead, it will only perform any kind of selection/mode-switching operations associated with loading the given data source.
    ///
    /// Note that we *do not* change the display mode here _unconditionally_.
    /// For instance if the datasource is a blueprint for a dataset that may be loaded later,
    /// we don't want to switch out to it while the user browses a server.
    fn load_data_source(
        &mut self,
        store_hub: &mut StoreHub,
        egui_ctx: &egui::Context,
        data_source: &LogDataSource,
    ) {
        re_tracing::profile_function!();

        // Check if we've already loaded this data source and should just switch to it.
        //
        // Go through all sources that are still loading and those that are already in the store_hub.
        // (if we look only at the one from the store_hub, we might miss those that haven't hit it yet)
        let active_sources = self.rx_log.sources();
        let store_sources = store_hub
            .store_bundle()
            .entity_dbs()
            .filter_map(|db| db.data_source.as_ref());
        let mut all_sources = store_sources.chain(active_sources.iter().map(|s| s.as_ref()));

        match data_source {
            LogDataSource::RrdHttpUrl { url, follow } => {
                let new_source = LogSource::RrdHttpStream {
                    url: url.to_string(),
                    follow: *follow,
                };

                if all_sources.any(|source| source.is_same_ignoring_uri_fragments(&new_source)) {
                    if let Some(entity_db) = store_hub.find_recording_store_by_source(&new_source) {
                        if *follow {
                            self.command_sender
                                .send_system(SystemCommand::TimeControlCommands {
                                    store_id: entity_db.store_id().clone(),
                                    time_commands: vec![TimeControlCommand::SetPlayState(
                                        PlayState::Following,
                                    )],
                                });
                        }

                        let store_id = entity_db.store_id().clone();
                        debug_assert!(store_id.is_recording()); // `find_recording_store_by_source` should have filtered for recordings rather than blueprints.
                        drop(all_sources);
                        self.make_store_active_and_highlight(store_hub, egui_ctx, &store_id);
                    }
                    return;
                }
            }

            #[cfg(not(target_arch = "wasm32"))]
            LogDataSource::FilePath(_file_source, path) => {
                let new_source = LogSource::File(path.clone());
                if all_sources.any(|source| source.is_same_ignoring_uri_fragments(&new_source)) {
                    drop(all_sources);
                    self.try_make_recording_from_source_active(egui_ctx, store_hub, &new_source);
                    return;
                }
            }

            LogDataSource::FileContents(_file_source, _file_contents) => {
                // For raw file contents we currently can't determine whether we're already receiving them.
            }

            #[cfg(not(target_arch = "wasm32"))]
            LogDataSource::Stdin => {
                let new_source = LogSource::Stdin;
                if all_sources.any(|source| source.is_same_ignoring_uri_fragments(&new_source)) {
                    drop(all_sources);
                    self.try_make_recording_from_source_active(egui_ctx, store_hub, &new_source);
                    return;
                }
            }

            LogDataSource::RedapDatasetSegment {
                uri,
                select_when_loaded,
            } => {
                let new_source = LogSource::RedapGrpcStream {
                    uri: uri.clone(),
                    select_when_loaded: *select_when_loaded,
                };
                if all_sources.any(|source| source.is_same_ignoring_uri_fragments(&new_source)) {
                    // We're already receiving from the exact same data source!
                    // But we still should select if requested according to the fragments if any.
                    if *select_when_loaded {
                        // First make the recording itself active.
                        // `go_to_dataset_data` may override the selection again, but this is important regardless,
                        // since `go_to_dataset_data` does not change the active recording.
                        drop(all_sources);
                        self.make_store_active_and_highlight(store_hub, egui_ctx, &uri.store_id());
                    }

                    // Note that applying the fragment changes the per-recording settings like the active time cursor.
                    // Therefore, we apply it even when `select_when_loaded` is false.
                    self.go_to_dataset_data(uri.store_id(), uri.fragment.clone());

                    return;
                }
            }

            LogDataSource::RedapProxy(uri) => {
                let new_source = LogSource::MessageProxy(uri.clone());
                if all_sources.any(|source| source.is_same_ignoring_uri_fragments(&new_source)) {
                    drop(all_sources);
                    self.try_make_recording_from_source_active(egui_ctx, store_hub, &new_source);
                    return;
                }
            }
        }

        let sender = self.command_sender.clone();
        let stream = data_source.clone().stream(
            Self::auth_error_handler(sender),
            &self.connection_registry,
            self.app_options().experimental.stream_mode,
        );

        #[cfg(feature = "analytics")]
        if let Some(analytics) = re_analytics::Analytics::global_or_init() {
            let data_source_analytics = data_source.analytics();
            analytics.record(re_analytics::event::LoadDataSource {
                source_type: data_source_analytics.source_type,
                file_extension: data_source_analytics.file_extension,
                file_source: data_source_analytics.file_source,
                started_successfully: stream.is_ok(),
            });
        }

        match stream {
            Ok(rx) => self.add_log_receiver(rx),
            Err(err) => {
                re_log::error!("Failed to open data source: {}", re_error::format(err));
            }
        }
    }

    /// Applies a fragment.
    ///
    /// Does *not* switch the active recording.
    fn go_to_dataset_data(&self, store_id: StoreId, fragment: re_uri::Fragment) {
        let re_uri::Fragment {
            selection,
            when,
            time_selection,
        } = fragment;

        if let Some(selection) = selection {
            let re_log_types::DataPath {
                entity_path,
                instance,
                component,
            } = selection;

            let item = if let Some(component) = component {
                Item::from(re_log_types::ComponentPath::new(entity_path, component))
            } else if let Some(instance) = instance {
                Item::from(InstancePath::instance(entity_path, instance))
            } else {
                Item::from(entity_path)
            };

            self.command_sender
                .send_system(SystemCommand::set_selection(item.clone()));
        }

        let mut time_commands = Vec::new();
        if let Some(time_selection) = time_selection {
            time_commands.push(TimeControlCommand::SetActiveTimeline(
                *time_selection.timeline.name(),
            ));
            time_commands.push(TimeControlCommand::SetTimeSelection(time_selection.range));
            time_commands.push(TimeControlCommand::SetLoopMode(LoopMode::Selection));
        }

        if let Some((timeline, timecell)) = when {
            time_commands.push(TimeControlCommand::SetActiveTimeline(timeline));
            time_commands.push(TimeControlCommand::SetPlayState(PlayState::Paused));
            time_commands.push(TimeControlCommand::SetTime(timecell.value.into()));
        }

        if !time_commands.is_empty() {
            self.command_sender
                .send_system(SystemCommand::TimeControlCommands {
                    store_id,
                    time_commands,
                });
        }
    }

    fn run_ui_command(
        &mut self,
        egui_ctx: &egui::Context,
        app_blueprint: &AppBlueprint<'_>,
        storage_context: &StorageContext<'_>,
        store_context: Option<&StoreContext<'_>>,
        display_mode: &DisplayMode,
        cmd: UICommand,
    ) {
        let mut force_store_info = false;
        let active_store_id = store_context
            .map(|ctx| ctx.recording_store_id().clone())
            // Don't redirect data to the welcome screen.
            .filter(|store_id| store_id.application_id() != &StoreHub::welcome_screen_app_id())
            .unwrap_or_else(|| {
                // If we don't have any application ID to recommend (which means we are on the welcome screen),
                // then just generate a new one using a UUID.
                let application_id = ApplicationId::random();

                // NOTE: We don't override blueprints' store IDs anyhow, so it is sound to assume that
                // this can only be a recording.
                let recording_id = RecordingId::random();

                // We're creating a recording just-in-time, directly from the viewer.
                // We need those store infos or the data will just be silently ignored.
                force_store_info = true;

                StoreId::recording(application_id, recording_id)
            });

        match cmd {
            UICommand::SaveRecording => {
                #[cfg(target_arch = "wasm32")] // Web
                {
                    if let Err(err) = save_active_recording(self, store_context, None) {
                        re_log::error!("Failed to save recording: {err}");
                    }
                }

                #[cfg(not(target_arch = "wasm32"))] // Native
                {
                    let mut selected_stores = vec![];
                    for item in self.state.selection_state.selected_items().iter_items() {
                        match item {
                            Item::AppId(selected_app_id) => {
                                for recording in storage_context.bundle.recordings() {
                                    if recording.application_id() == selected_app_id {
                                        selected_stores.push(recording.store_id().clone());
                                    }
                                }
                            }
                            Item::StoreId(store_id) => {
                                selected_stores.push(store_id.clone());
                            }
                            _ => {}
                        }
                    }

                    let selected_stores = selected_stores
                        .iter()
                        .filter_map(|store_id| storage_context.bundle.get(store_id))
                        .collect_vec();

                    if selected_stores.is_empty() {
                        if let Err(err) = save_active_recording(self, store_context, None) {
                            re_log::error!("Failed to save recording: {err}");
                        }
                    } else if selected_stores.len() == 1 {
                        // Common case: saving a single recording.
                        // In this case we want the user to be able to pick a file name (not just a folder):
                        if let Err(err) = save_recording(self, selected_stores[0], None) {
                            re_log::error!("Failed to save recording: {err}");
                        }
                    } else {
                        // Save all selected recordings to a folder:
                        if let Some(folder) = rfd::FileDialog::new()
                            .set_title("Save recordings to folder")
                            .pick_folder()
                        {
                            self.save_many_recordings(&selected_stores, &folder);
                        } else {
                            re_log::info!("No folder selected - recordings not saved.");
                        }
                    }
                }
            }
            UICommand::SaveRecordingSelection => {
                if let Err(err) = save_active_recording(
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
                        .send_system(SystemCommand::LoadDataSource(LogDataSource::FilePath(
                            FileSource::FileDialog {
                                recommended_store_id: None,
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
                    recommended_store_id: None,
                    force_store_info,
                    promise,
                });
            }

            #[cfg(not(target_arch = "wasm32"))]
            UICommand::Import => {
                for file_path in open_file_dialog_native(self.main_thread_token) {
                    self.command_sender
                        .send_system(SystemCommand::LoadDataSource(LogDataSource::FilePath(
                            FileSource::FileDialog {
                                recommended_store_id: Some(active_store_id.clone()),
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
                    recommended_store_id: Some(active_store_id.clone()),
                    force_store_info,
                    promise,
                });
            }

            UICommand::OpenUrl => {
                self.state.open_url_modal.open();
            }

            UICommand::CloseCurrentRecording => {
                let cur_rec = store_context.map(|ctx| ctx.recording.store_id());
                if let Some(cur_rec) = cur_rec {
                    self.command_sender
                        .send_system(SystemCommand::CloseRecordingOrTable(cur_rec.clone().into()));
                }
            }
            UICommand::CloseAllEntries => {
                self.command_sender
                    .send_system(SystemCommand::CloseAllEntries);
            }

            UICommand::NextRecording => {
                self.state
                    .recording_panel
                    .send_command(re_recording_panel::RecordingPanelCommand::SelectNextRecording);
            }
            UICommand::PreviousRecording => {
                self.state.recording_panel.send_command(
                    re_recording_panel::RecordingPanelCommand::SelectPreviousRecording,
                );
            }

            UICommand::NavigateBack => {
                if let Some(url) = self.state.history.go_back() {
                    url.clone().open(
                        egui_ctx,
                        &OpenUrlOptions {
                            follow_if_http: true,
                            select_redap_source_when_loaded: true,
                            show_loader: true,
                        },
                        &self.command_sender,
                    );
                }
            }
            UICommand::NavigateForward => {
                if let Some(url) = self.state.history.go_forward() {
                    url.clone().open(
                        egui_ctx,
                        &OpenUrlOptions {
                            follow_if_http: true,
                            select_redap_source_when_loaded: true,
                            show_loader: true,
                        },
                        &self.command_sender,
                    );
                }
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
            UICommand::ClearActiveBlueprint => {
                self.command_sender
                    .send_system(SystemCommand::ClearActiveBlueprint);
            }
            UICommand::ClearActiveBlueprintAndEnableHeuristics => {
                self.command_sender
                    .send_system(SystemCommand::ClearActiveBlueprintAndEnableHeuristics);
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
            UICommand::ExpandBlueprintPanel => {
                if !app_blueprint.blueprint_panel_state().is_expanded() {
                    app_blueprint.toggle_blueprint_panel(&self.command_sender);
                }
            }
            UICommand::ToggleSelectionPanel => {
                app_blueprint.toggle_selection_panel(&self.command_sender);
            }
            UICommand::ExpandSelectionPanel => {
                if !app_blueprint.selection_panel_state().is_expanded() {
                    app_blueprint.toggle_selection_panel(&self.command_sender);
                }
            }
            UICommand::ToggleTimePanel => app_blueprint.toggle_time_panel(&self.command_sender),

            UICommand::ToggleChunkStoreBrowser => match self.state.navigation.current() {
                DisplayMode::LocalRecordings(_)
                | DisplayMode::RedapEntry(_)
                | DisplayMode::RedapServer(_) => {
                    self.state
                        .navigation
                        .replace(DisplayMode::ChunkStoreBrowser(Box::new(
                            self.state.navigation.current().clone(),
                        )));
                }

                DisplayMode::ChunkStoreBrowser(mode) => {
                    self.state.navigation.replace((**mode).clone());
                }

                DisplayMode::Settings(_) | DisplayMode::Loading(_) | DisplayMode::LocalTable(_) => {
                    re_log::debug!(
                        "Cannot toggle chunk store browser from current display mode: {:?}",
                        self.state.navigation.current()
                    );
                }
            },

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
                self.command_sender.send_system(SystemCommand::OpenSettings);
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
                if let Some(store_id) = storage_context.hub.active_store_id() {
                    self.command_sender
                        .send_system(SystemCommand::TimeControlCommands {
                            store_id: store_id.clone(),
                            time_commands: vec![TimeControlCommand::TogglePlayPause],
                        });
                }
            }
            UICommand::PlaybackFollow => {
                if let Some(store_id) = storage_context.hub.active_store_id() {
                    self.command_sender
                        .send_system(SystemCommand::TimeControlCommands {
                            store_id: store_id.clone(),
                            time_commands: vec![TimeControlCommand::SetPlayState(
                                PlayState::Following,
                            )],
                        });
                }
            }
            UICommand::PlaybackStepBack => {
                if let Some(store_id) = storage_context.hub.active_store_id() {
                    self.command_sender
                        .send_system(SystemCommand::TimeControlCommands {
                            store_id: store_id.clone(),
                            time_commands: vec![TimeControlCommand::StepTimeBack],
                        });
                }
            }
            UICommand::PlaybackStepForward => {
                if let Some(store_id) = storage_context.hub.active_store_id() {
                    self.command_sender
                        .send_system(SystemCommand::TimeControlCommands {
                            store_id: store_id.clone(),
                            time_commands: vec![TimeControlCommand::StepTimeForward],
                        });
                }
            }
            UICommand::PlaybackBack => {
                if let Some(store_id) = storage_context.hub.active_store_id() {
                    self.command_sender
                        .send_system(SystemCommand::TimeControlCommands {
                            store_id: store_id.clone(),
                            time_commands: vec![TimeControlCommand::MoveBySeconds(-0.1)],
                        });
                }
            }
            UICommand::PlaybackForward => {
                if let Some(store_id) = storage_context.hub.active_store_id() {
                    self.command_sender
                        .send_system(SystemCommand::TimeControlCommands {
                            store_id: store_id.clone(),
                            time_commands: vec![TimeControlCommand::MoveBySeconds(0.1)],
                        });
                }
            }
            UICommand::PlaybackBackFast => {
                if let Some(store_id) = storage_context.hub.active_store_id() {
                    self.command_sender
                        .send_system(SystemCommand::TimeControlCommands {
                            store_id: store_id.clone(),
                            time_commands: vec![TimeControlCommand::MoveBySeconds(-1.0)],
                        });
                }
            }
            UICommand::PlaybackForwardFast => {
                if let Some(store_id) = storage_context.hub.active_store_id() {
                    self.command_sender
                        .send_system(SystemCommand::TimeControlCommands {
                            store_id: store_id.clone(),
                            time_commands: vec![TimeControlCommand::MoveBySeconds(1.0)],
                        });
                }
            }
            UICommand::PlaybackBeginning => {
                if let Some(store_id) = storage_context.hub.active_store_id() {
                    self.command_sender
                        .send_system(SystemCommand::TimeControlCommands {
                            store_id: store_id.clone(),
                            time_commands: vec![TimeControlCommand::MoveBeginning],
                        });
                }
            }
            UICommand::PlaybackEnd => {
                if let Some(store_id) = storage_context.hub.active_store_id() {
                    self.command_sender
                        .send_system(SystemCommand::TimeControlCommands {
                            store_id: store_id.clone(),
                            time_commands: vec![TimeControlCommand::MoveEnd],
                        });
                }
            }
            UICommand::PlaybackRestart => {
                if let Some(store_id) = storage_context.hub.active_store_id() {
                    self.command_sender
                        .send_system(SystemCommand::TimeControlCommands {
                            store_id: store_id.clone(),
                            time_commands: vec![TimeControlCommand::Restart],
                        });
                }
            }

            UICommand::PlaybackSpeed(speed) => {
                if let Some(store_id) = storage_context.hub.active_store_id() {
                    self.command_sender
                        .send_system(SystemCommand::TimeControlCommands {
                            store_id: store_id.clone(),
                            time_commands: vec![TimeControlCommand::SetSpeed(speed.0.0)],
                        });
                }
            }

            #[cfg(not(target_arch = "wasm32"))]
            UICommand::ScreenshotWholeApp => {
                self.screenshotter.request_screenshot(egui_ctx);
            }
            #[cfg(not(target_arch = "wasm32"))]
            UICommand::PrintChunkStore => {
                if let Some(ctx) = store_context {
                    let text = format!("{}", ctx.recording.storage_engine().store());
                    egui_ctx.copy_text(text.clone());
                    println!("{text}");
                }
            }
            #[cfg(not(target_arch = "wasm32"))]
            UICommand::PrintBlueprintStore => {
                if let Some(ctx) = store_context {
                    let text = format!("{}", ctx.blueprint.storage_engine().store());
                    egui_ctx.copy_text(text.clone());
                    println!("{text}");
                }
            }
            #[cfg(not(target_arch = "wasm32"))]
            UICommand::PrintPrimaryCache => {
                if let Some(ctx) = store_context {
                    let text = format!("{:?}", ctx.recording.storage_engine().cache());
                    egui_ctx.copy_text(text.clone());
                    println!("{text}");
                }
            }

            #[cfg(debug_assertions)]
            UICommand::ResetEguiMemory => {
                egui_ctx.memory_mut(|mem| *mem = Default::default());

                // re-apply style, which is lost when resetting memory
                re_ui::apply_style_and_install_loaders(egui_ctx);
            }

            UICommand::Share => {
                let selection = self.state.selection_state.selected_items();
                let rec_cfg = storage_context
                    .hub
                    .active_store_id()
                    .and_then(|id| self.state.time_controls.get(id));
                if let Err(err) = self.state.share_modal.open(
                    storage_context.hub,
                    display_mode,
                    rec_cfg,
                    selection,
                ) {
                    re_log::error!("Cannot share link to current screen: {err}");
                }
            }
            UICommand::CopyDirectLink => {
                match ViewerOpenUrl::from_display_mode(storage_context.hub, display_mode) {
                    Ok(url) => self.run_copy_link_command(&url),
                    Err(err) => re_log::error!("{err}"),
                }
            }

            UICommand::CopyTimeSelectionLink => {
                match ViewerOpenUrl::from_display_mode(storage_context.hub, display_mode) {
                    Ok(mut url) => {
                        if let Some(fragment) = url.fragment_mut() {
                            let time_ctrl = storage_context
                                .hub
                                .active_store_id()
                                .and_then(|id| self.state.time_control(id));

                            if let Some(time_ctrl) = &time_ctrl
                                && let Some(time_selection) = time_ctrl.time_selection()
                                && let Some(timeline) = time_ctrl.timeline()
                            {
                                fragment.time_selection = Some(re_uri::TimeSelection {
                                    timeline: *timeline,
                                    range: time_selection.to_int(),
                                });
                            } else {
                                re_log::warn!("No timeline selection to copy");
                            }
                        } else {
                            re_log::warn!(
                                "The current recording doesn't support sharing a time range"
                            );
                        }

                        self.run_copy_link_command(&url);
                    }
                    Err(err) => re_log::error!("{err}"),
                }
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

            UICommand::CopyEntityHierarchy => {
                self.copy_entity_hierarchy_to_clipboard(egui_ctx, store_context);
            }

            UICommand::AddRedapServer => {
                self.state.redap_servers.open_add_server_modal();
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn save_many_recordings(&mut self, stores: &[&EntityDb], folder: &std::path::Path) {
        use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

        use re_log::ResultExt as _;
        use tap::Pipe as _;

        re_tracing::profile_function!();

        let num_stores = stores.len();
        let any_error = Arc::new(AtomicBool::new(false));
        let num_remaining = Arc::new(AtomicUsize::new(stores.len()));

        re_log::info!("Saving {num_stores} recordings to {}…", folder.display());

        for store in stores {
            let messages = store.to_messages(None).collect_vec();

            let file_name = if let Some(rec_name) = store
                .recording_info_property::<re_sdk_types::components::Name>(
                    re_sdk_types::archetypes::RecordingInfo::descriptor_name().component,
                ) {
                rec_name.to_string()
            } else {
                format!("{}-{}", store.application_id(), store.recording_id())
            }
            .pipe(|name| sanitize_file_name(&name))
            .pipe(|stem| format!("{stem}.rrd"));

            let file_path = folder.join(file_name.clone());
            let any_error = any_error.clone();
            let num_remaining = num_remaining.clone();
            let folder = folder.display().to_string();

            self.background_tasks
                .spawn_threaded_promise(file_name, move || {
                    let res = crate::saving::encode_to_file(
                        re_build_info::CrateVersion::LOCAL,
                        &file_path,
                        messages.into_iter(),
                    );

                    if res.is_err() {
                        any_error.store(true, Ordering::Relaxed);
                    }

                    let num_remaining = num_remaining.fetch_sub(1, Ordering::Relaxed) - 1;

                    if num_remaining == 0 {
                        if any_error.load(Ordering::Relaxed) {
                            re_log::error!("Some recordings failed to save.");
                        } else {
                            re_log::info!("{num_stores} recordings successfully saved to {folder}");
                        }
                    }

                    res
                })
                .ok_or_log_error_once();
        }
    }

    fn run_copy_link_command(&mut self, content_url: &ViewerOpenUrl) {
        let base_url = self.startup_options.web_viewer_base_url();

        match content_url.sharable_url(base_url.as_ref()) {
            Ok(url) => {
                self.copy_text(url);
            }
            Err(err) => {
                re_log::error!("{err}");
            }
        }
    }

    /// Copies text to the clipboard, and gives a notification about it.
    fn copy_text(&mut self, url: String) {
        self.notifications
            .success(format!("Copied {url:?} to clipboard"));
        self.egui_ctx.copy_text(url);
    }

    fn copy_entity_hierarchy_to_clipboard(
        &mut self,
        egui_ctx: &egui::Context,
        store_context: Option<&StoreContext<'_>>,
    ) {
        let Some(entity_db) = store_context.as_ref().map(|ctx| ctx.recording) else {
            re_log::warn!("Could not copy entity hierarchy: No active recording");
            return;
        };

        let mut hierarchy_text = String::new();

        // Add application ID and recording ID header
        hierarchy_text.push_str(&format!(
            "Application ID: {}\nRecording ID: {}\n\n",
            entity_db.application_id(),
            entity_db.recording_id()
        ));

        hierarchy_text.push_str(&entity_db.format_with_components());

        if hierarchy_text.is_empty() {
            hierarchy_text = "(no entities)".to_owned();
        }

        egui_ctx.copy_text(hierarchy_text.clone());
        self.notifications
            .success("Copied entity hierarchy with schema to clipboard".to_owned());
    }

    fn memory_panel_ui(
        &self,
        ui: &mut egui::Ui,
        gpu_resource_stats: &WgpuResourcePoolStatistics,
        store_stats: Option<&StoreHubStats>,
    ) {
        let frame = egui::Frame {
            fill: ui.visuals().panel_fill,
            ..ui.tokens().bottom_panel_frame()
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
            .frame(ui.tokens().top_panel_frame())
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
    #[expect(clippy::too_many_arguments)]
    fn ui(
        &mut self,
        egui_ctx: &egui::Context,
        frame: &eframe::Frame,
        app_blueprint: &AppBlueprint<'_>,
        gpu_resource_stats: &WgpuResourcePoolStatistics,
        store_context: Option<&StoreContext<'_>>,
        storage_context: &StorageContext<'_>,
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
                    storage_context.hub,
                    gpu_resource_stats,
                    ui,
                );

                self.memory_panel_ui(ui, gpu_resource_stats, store_stats);

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
                    if let Some(store_context) = store_context {
                        render_ctx.begin_frame(); // This may actually be called multiple times per egui frame, if we have a multi-pass layout frame.

                        // In some (rare) circumstances we run two egui passes in a single frame.
                        // This happens on call to `egui::Context::request_discard`.
                        let is_start_of_new_frame = egui_ctx.current_pass_index() == 0;

                        if is_start_of_new_frame {
                            self.state.redap_servers.on_frame_start(
                                &self.connection_registry,
                                &self.async_runtime,
                                &self.egui_ctx,
                            );
                        }

                        let mut startup_options = self.startup_options.clone();

                        self.state.show(
                            &self.app_env,
                            &mut startup_options,
                            app_blueprint,
                            ui,
                            render_ctx,
                            store_context,
                            storage_context,
                            &self.reflection,
                            &self.component_ui_registry,
                            &self.component_fallback_registry,
                            &self.view_class_registry,
                            &self.rx_log,
                            &self.command_sender,
                            &WelcomeScreenState {
                                hide_examples: self.startup_options.hide_welcome_screen,
                                opacity: self.welcome_screen_opacity(egui_ctx),
                            },
                            self.event_dispatcher.as_ref(),
                            &self.connection_registry,
                            &self.async_runtime,
                        );
                        self.startup_options = startup_options;
                        render_ctx.before_submit();
                    }

                    self.show_text_logs_as_notifications();
                }
            });

        if self.app_options().show_notification_toasts {
            self.notifications.show_toasts(egui_ctx);
        }
    }

    /// Show recent text log messages to the user as toast notifications.
    fn show_text_logs_as_notifications(&mut self) {
        re_tracing::profile_function!();

        while let Ok(message) = self.text_log_rx.try_recv() {
            self.notifications.add_log(message);
        }
    }

    fn receive_messages(&mut self, store_hub: &mut StoreHub, egui_ctx: &egui::Context) {
        re_tracing::profile_function!();

        let start = web_time::Instant::now();

        while let Some((channel_source, msg)) = self.rx_log.try_recv() {
            re_log::trace!("Received a message from {channel_source:?}"); // Used by `test_ui_wakeup` test app!

            let msg = match msg.payload {
                re_log_channel::SmartMessagePayload::Msg(msg) => msg,

                re_log_channel::SmartMessagePayload::Flush { on_flush_done } => {
                    on_flush_done();
                    continue;
                }

                re_log_channel::SmartMessagePayload::Quit(err) => {
                    if let Some(err) = err {
                        re_log::warn!("Data source {} has left unexpectedly: {err}", msg.source);
                    } else {
                        re_log::debug!("Data source {} has finished", msg.source);
                    }
                    continue;
                }
            };

            match msg {
                DataSourceMessage::RrdManifest(store_id, rrd_manifest) => {
                    let entity_db = store_hub.entity_db_mut(&store_id);
                    entity_db.add_rrd_manifest_message(*rrd_manifest);
                }

                DataSourceMessage::LogMsg(msg) => {
                    self.receive_log_msg(&msg, store_hub, egui_ctx, &channel_source);
                }

                DataSourceMessage::TableMsg(table) => {
                    self.receive_table_msg(store_hub, egui_ctx, table);
                }

                DataSourceMessage::UiCommand(ui_command) => {
                    self.receive_data_source_ui_command(ui_command, &channel_source);
                }
            }

            if start.elapsed() > web_time::Duration::from_millis(10) {
                egui_ctx.request_repaint(); // make sure we keep receiving messages asap
                break; // don't block the main thread for too long
            }
        }

        // Run pending system commands in case any of the messages resulted in additional commands.
        // This avoid further frame delays on these commands.
        self.run_pending_system_commands(store_hub, egui_ctx);
    }

    fn receive_log_msg(
        &self,
        msg: &LogMsg,
        store_hub: &mut StoreHub,
        egui_ctx: &egui::Context,
        channel_source: &LogSource,
    ) {
        let store_id = msg.store_id();

        if store_hub.is_active_blueprint(store_id) {
            // TODO(#5514): handle loading of active blueprints.
            re_log::warn_once!(
                "Loading a blueprint {store_id:?} that is active. See https://github.com/rerun-io/rerun/issues/5514 for details."
            );
        }

        // Note that the `SetStoreInfo` message might be missing. It's not strictly necessary to add a new store.
        let msg_will_add_new_store = !store_hub.store_bundle().contains(store_id);

        let entity_db = store_hub.entity_db_mut(store_id);
        if entity_db.data_source.is_none() {
            entity_db.data_source = Some((*channel_source).clone());
        }

        let was_empty = entity_db.is_empty();
        let entity_db_add_result = entity_db.add_log_msg(msg);

        // Downgrade to read-only, so we can access caches.
        let entity_db = store_hub
            .entity_db(store_id)
            .expect("Just queried it mutable and that was fine.");

        match entity_db_add_result {
            Ok(store_events) => {
                if let Some(caches) = store_hub.active_caches() {
                    caches.on_store_events(&store_events, entity_db);
                }

                self.validate_loaded_events(&store_events);
            }

            Err(err) => {
                re_log::error_once!("Failed to add incoming msg: {err}");
            }
        }

        if was_empty && !entity_db.is_empty() {
            // Hack: we cannot go to a specific timeline or entity until we know about it.
            // Now we _hopefully_ do.
            if let LogSource::RedapGrpcStream { uri, .. } = channel_source {
                self.go_to_dataset_data(uri.store_id(), uri.fragment.clone());
            }
        }

        #[expect(clippy::match_same_arms)]
        match &msg {
            LogMsg::SetStoreInfo(_) => {
                // Causes a new store typically. But that's handled below via `on_new_store`.
            }

            LogMsg::ArrowMsg(_, _) => {
                // Handled by `EntityDb::add`.
            }

            LogMsg::BlueprintActivationCommand(cmd) => match store_id.kind() {
                StoreKind::Recording => {
                    re_log::debug!(
                        "Unexpected `BlueprintActivationCommand` message for {store_id:?}"
                    );
                }
                StoreKind::Blueprint => {
                    if let Some(info) = entity_db.store_info() {
                        re_log::trace!(
                            "Activating blueprint that was loaded from {channel_source}"
                        );
                        let app_id = info.application_id().clone();
                        if cmd.make_default {
                            store_hub
                                .set_default_blueprint_for_app(store_id)
                                .unwrap_or_else(|err| {
                                    re_log::warn!("Failed to make blueprint default: {err}");
                                });
                        }
                        if cmd.make_active {
                            store_hub
                                .set_cloned_blueprint_active_for_app(store_id)
                                .unwrap_or_else(|err| {
                                    re_log::warn!("Failed to make blueprint active: {err}");
                                });

                            // Switch to this app, e.g. on drag-and-drop of a blueprint file
                            store_hub.set_active_app(app_id);

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

        // Handle any action that is triggered by a new store _after_ processing the message that caused it.
        if msg_will_add_new_store {
            self.on_new_store(egui_ctx, store_id, channel_source, store_hub);
        }
    }

    fn receive_table_msg(
        &self,
        store_hub: &mut StoreHub,
        egui_ctx: &egui::Context,
        table: TableMsg,
    ) {
        let TableMsg { id, data } = table;

        // TODO(grtlr): For now we don't append anything to existing stores and always replace.
        // TODO(ab): When we actually append to existing table, we will have to clear the UI
        // cache by calling `DataFusionTableWidget::clear_state`.
        let store = TableStore::default();
        if let Err(err) = store.add_record_batch(data) {
            re_log::error!("Failed to load table {id}: {err}");
        } else {
            if store_hub.insert_table_store(id.clone(), store).is_some() {
                re_log::debug!("Overwritten table store with id: `{id}`");
            } else {
                re_log::debug!("Inserted table store with id: `{id}`");
            }
            self.command_sender
                .send_system(SystemCommand::set_selection(
                    re_viewer_context::Item::TableId(id),
                ));

            // If the viewer is in the background, tell the user that it has received something new.
            egui_ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
                egui::UserAttentionType::Informational,
            ));
        }
    }

    fn on_new_store(
        &self,
        egui_ctx: &egui::Context,
        store_id: &StoreId,
        channel_source: &LogSource,
        store_hub: &mut StoreHub,
    ) {
        if channel_source.select_when_loaded() {
            // Set the recording-id after potentially creating the store in the hub.
            // This ordering is important because the `StoreHub` internally
            // updates the app-id when changing the recording.
            match store_id.kind() {
                StoreKind::Recording => {
                    re_log::trace!("Opening a new recording: '{store_id:?}'");
                    self.make_store_active_and_highlight(store_hub, egui_ctx, store_id);
                }
                StoreKind::Blueprint => {
                    // We wait with activating blueprints until they are fully loaded,
                    // so that we don't run heuristics on half-loaded blueprints.
                    // Otherwise on a mixed connection (SDK sending both blueprint and recording)
                    // the blueprint won't be activated until the whole _recording_ has finished loading.
                }
            }
        }

        let entity_db = store_hub.entity_db_mut(store_id);
        let is_example = entity_db.store_class().is_example();

        if cfg!(target_arch = "wasm32") && !self.startup_options.is_in_notebook && !is_example {
            use std::sync::Once;
            static ONCE: Once = Once::new();
            ONCE.call_once(|| {
                // Tell the user there is a faster native viewer they can use instead of the web viewer:
                let notification = re_ui::notifications::Notification::new(
                    re_ui::notifications::NotificationLevel::Tip, "For better performance, try the native Rerun Viewer!").with_link(
                    re_ui::Link {
                        text: "Install…".into(),
                        url: "https://rerun.io/docs/getting-started/installing-viewer#installing-the-viewer".into(),
                    }
                )
                    .no_toast()
                    .permanent_dismiss_id(egui::Id::new("install_native_viewer_prompt"));
                self.command_sender
                    .send_system(SystemCommand::ShowNotification(notification));
            });
        }

        if entity_db.store_kind() == StoreKind::Recording {
            #[cfg(feature = "analytics")]
            if let Some(analytics) = re_analytics::Analytics::global_or_init()
                && let Some(event) =
                    crate::viewer_analytics::event::open_recording(&self.app_env, entity_db)
            {
                analytics.record(event);
            }

            if let Some(event_dispatcher) = self.event_dispatcher.as_ref() {
                event_dispatcher.on_recording_open(entity_db);
            }
        }
    }

    fn receive_data_source_ui_command(
        &self,
        ui_command: DataSourceUiCommand,
        channel_source: &LogSource,
    ) {
        match ui_command {
            DataSourceUiCommand::SetUrlFragment { store_id, fragment } => {
                match re_uri::Fragment::from_str(&fragment) {
                    Ok(fragment) => {
                        self.command_sender
                            .send_system(SystemCommand::SetUrlFragment { store_id, fragment });
                    }

                    Err(err) => {
                        re_log::warn!(
                            "Failed to parse fragment received from {channel_source:?}: {err}"
                        );
                    }
                }
            }
        }
    }

    /// Makes the first recording store active that is found for a given data source if any.
    fn try_make_recording_from_source_active(
        &self,
        egui_ctx: &egui::Context,
        store_hub: &mut StoreHub,
        new_source: &LogSource,
    ) {
        if let Some(entity_db) = store_hub.find_recording_store_by_source(new_source) {
            let store_id = entity_db.store_id().clone();
            debug_assert!(store_id.is_recording()); // `find_recording_store_by_source` should have filtered for recordings rather than blueprints.
            self.make_store_active_and_highlight(store_hub, egui_ctx, &store_id);
        }
    }

    /// Makes the given store active and request user attention if Rerun in the background.
    fn make_store_active_and_highlight(
        &self,
        store_hub: &mut StoreHub,
        egui_ctx: &egui::Context,
        store_id: &StoreId,
    ) {
        if store_id.is_blueprint() {
            re_log::warn!(
                "Can't make a blueprint active: {store_id:?}. This is likely a bug in Rerun."
            );
            return;
        }

        store_hub.set_active_recording_id(store_id.clone());

        // Also select the new recording:
        self.command_sender
            .send_system(SystemCommand::set_selection(
                re_viewer_context::Item::StoreId(store_id.clone()),
            ));

        // If the viewer is in the background, tell the user that it has received something new.
        egui_ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
            egui::UserAttentionType::Informational,
        ));
    }

    /// After loading some data; check if the loaded data makes sense.
    fn validate_loaded_events(&self, store_events: &[re_chunk_store::ChunkStoreEvent]) {
        re_tracing::profile_function!();

        for event in store_events {
            let chunk = &event.diff.chunk;

            // For speed, we don't care about the order of the following log statements, so we silence this warning
            for component_descr in chunk.components().component_descriptors() {
                if let Some(archetype_name) = component_descr.archetype {
                    if let Some(archetype) = self.reflection.archetypes.get(&archetype_name) {
                        for &view_type in archetype.view_types {
                            if !cfg!(feature = "map_view") && view_type == "MapView" {
                                re_log::warn_once!(
                                    "Found map-related archetype, but viewer was not compiled with the `map_view` feature."
                                );
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

        fn format_limit(limit: Option<u64>) -> String {
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
                "Reached memory limit of {}. Freeing up data…",
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

            let time_cursor_for =
                |store_id: &StoreId| -> Option<(re_log_types::Timeline, re_log_types::TimeInt)> {
                    let time_ctrl = self.state.time_controls.get(store_id)?;
                    Some((*time_ctrl.timeline()?, time_ctrl.time_int()?))
                };
            store_hub.purge_fraction_of_ram(fraction_to_purge, &time_cursor_for);

            let mem_use_after = MemoryUse::capture();

            let freed_memory = mem_use_before - mem_use_after;

            if let (Some(counted_before), Some(counted_diff)) =
                (mem_use_before.counted, freed_memory.counted)
                && 0 < counted_diff
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
        self.store_hub.as_ref()?.active_recording()
    }

    // NOTE: Relying on `self` is dangerous, as this is called during a time where some internal
    // fields may have been temporarily `take()`n out. Keep this a static method.
    fn handle_dropping_files(
        egui_ctx: &egui::Context,
        storage_ctx: &StorageContext<'_>,
        command_sender: &CommandSender,
    ) {
        #![allow(clippy::allow_attributes, clippy::needless_continue)] // false positive, depending on target_arch

        preview_files_being_dropped(egui_ctx);

        let dropped_files = egui_ctx.input_mut(|i| std::mem::take(&mut i.raw.dropped_files));

        if dropped_files.is_empty() {
            return;
        }

        let mut force_store_info = false;

        for file in dropped_files {
            let active_store_id = storage_ctx
                .hub
                .active_store_id()
                .cloned()
                // Don't redirect data to the welcome screen.
                .filter(|store_id| store_id.application_id() != &StoreHub::welcome_screen_app_id())
                .unwrap_or_else(|| {
                    // When we're on the welcome screen, there is no recording ID to recommend.
                    // But we want one, otherwise multiple things being dropped simultaneously on the
                    // welcome screen would end up in different recordings!

                    // If we don't have any application ID to recommend (which means we are on the welcome screen),
                    // then we use the file path as the application ID or the file name if there is no path (on web builds).
                    let application_id = file
                        .path
                        .clone()
                        .map(|p| ApplicationId::from(p.display().to_string()))
                        .unwrap_or_else(|| ApplicationId::from(file.name.clone()));

                    // NOTE: We don't override blueprints' store IDs anyhow, so it is sound to assume that
                    // this can only be a recording.
                    let recording_id = RecordingId::random();

                    // We're creating a recording just-in-time, directly from the viewer.
                    // We need those store infos or the data will just be silently ignored.
                    force_store_info = true;

                    StoreId::recording(application_id, recording_id)
                });

            if let Some(bytes) = file.bytes {
                // This is what we get on Web.
                command_sender.send_system(SystemCommand::LoadDataSource(
                    LogDataSource::FileContents(
                        FileSource::DragAndDrop {
                            recommended_store_id: Some(active_store_id.clone()),
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
                command_sender.send_system(SystemCommand::LoadDataSource(LogDataSource::FilePath(
                    FileSource::DragAndDrop {
                        recommended_store_id: Some(active_store_id.clone()),
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

        for source in self.rx_log.sources() {
            match &*source {
                LogSource::File(_)
                | LogSource::RrdHttpStream { .. }
                | LogSource::RedapGrpcStream { .. }
                | LogSource::Stdin
                | LogSource::RrdWebEvent
                | LogSource::Sdk
                | LogSource::JsChannel { .. } => {
                    return true; // We expect data soon, so fade-in
                }

                LogSource::MessageProxy { .. } => {
                    // We start a gRPC server by default in native rerun, i.e. when just running `rerun`,
                    // and in that case fading in the welcome screen would be slightly annoying.
                    // However, we also use the gRPC server for sending data from the logging SDKs
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
                if let Err(err) = options.on_toggle.call0() {
                    re_log::error!("{}", crate::web_tools::string_from_js_value(err));
                }
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
            match options.get_state.call0() {
                Ok(v) => return v.is_truthy(),
                Err(err) => re_log::error_once!("{}", crate::web_tools::string_from_js_value(err)),
            }
        }

        false
    }

    #[allow(clippy::allow_attributes, clippy::needless_pass_by_ref_mut)] // False positive on wasm
    fn process_screenshot_result(
        &mut self,
        image: &Arc<egui::ColorImage>,
        user_data: &egui::UserData,
    ) {
        use re_viewer_context::ScreenshotInfo;

        if let Some(info) = user_data
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
                re_viewer_context::ScreenshotTarget::CopyToClipboard => {
                    self.egui_ctx.copy_image((*rgba).clone());
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
            self.screenshotter.save(&self.egui_ctx, image);
        }
    }

    /// Get a helper struct to interact with the given recording.
    pub fn blueprint_ctx<'a>(&'a self, recording_id: &StoreId) -> Option<AppBlueprintCtx<'a>> {
        let hub = self.store_hub.as_ref()?;

        let blueprint = hub.active_blueprint_for_app(recording_id.application_id())?;

        let default_blueprint = hub.default_blueprint_for_app(recording_id.application_id());

        let blueprint_query = self
            .state
            .get_blueprint_query_for_viewer(blueprint)
            .unwrap_or_else(|| {
                re_chunk::LatestAtQuery::latest(re_viewer_context::blueprint_timeline())
            });

        Some(AppBlueprintCtx {
            command_sender: &self.command_sender,
            current_blueprint: blueprint,
            default_blueprint,
            blueprint_query,
        })
    }

    /// Prefetch chunks for the open recording (stream from server)
    fn prefetch_chunks(&self, store_hub: &mut StoreHub) {
        re_tracing::profile_function!();

        // Receive in-transit chunks (previously prefetched):
        for db in store_hub.store_bundle_mut().recordings_mut() {
            if db.rrd_manifest_index.has_manifest() {
                for chunk in db.rrd_manifest_index.resolve_pending_promises() {
                    if let Err(err) = db.add_chunk(&std::sync::Arc::new(chunk)) {
                        re_log::warn_once!("add_chunk failed: {err}");
                    }
                }

                if db.rrd_manifest_index.has_pending_promises() {
                    self.egui_ctx.request_repaint(); // check back for more
                }
            }
        }

        // Prefetch new chunks for the active recording (if any):
        if let Some(recording) = store_hub.active_recording_mut()
            && let Some(time_ctrl) = self.state.time_controls.get(recording.store_id())
        {
            crate::prefetch_chunks::prefetch_chunks_for_active_recording(
                &self.egui_ctx,
                &self.startup_options,
                recording,
                time_ctrl,
                self.connection_registry(),
            );
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
                    re_log::warn_once!(
                        "Blueprint for {app_id} at {blueprint_path:?} appears invalid - will ignore. This is expected if you have just upgraded Rerun versions."
                    );
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
    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        if re_ui::CUSTOM_WINDOW_DECORATIONS {
            [0.; 4] // transparent
        } else if visuals.dark_mode {
            [0., 0., 0., 1.]
        } else {
            [1., 1., 1., 1.]
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        if !self.startup_options.persist_state {
            return;
        }

        re_tracing::profile_function!();

        storage.set_string(RERUN_VERSION_KEY, self.build_info.version.to_string());

        // Save the app state
        eframe::set_value(storage, eframe::APP_KEY, &self.state);
        eframe::set_value(
            storage,
            REDAP_TOKEN_KEY,
            &self.connection_registry.dump_tokens(),
        );

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
        #[cfg(all(not(target_arch = "wasm32"), feature = "perf_telemetry_tracy"))]
        re_perf_telemetry::external::tracing_tracy::client::frame_mark();

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
                crate::web_history::go_back();
            }
            if fwd_pressed {
                crate::web_history::go_forward();
            }
        }

        // We move the time at the very start of the frame,
        // so that we always show the latest data when we're in "follow" mode.
        self.move_time();

        // Temporarily take the `StoreHub` out of the Viewer so it doesn't interfere with mutability
        let mut store_hub = self
            .store_hub
            .take()
            .expect("Failed to take store hub from the Viewer");

        // Update data source order so it's based on opening order.
        store_hub.update_data_source_order(&self.rx_log.sources());

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
            recommended_store_id,
            force_store_info,
            promise,
        }) = &self.open_files_promise
            && let Some(files) = promise.ready()
        {
            for file in files {
                self.command_sender
                    .send_system(SystemCommand::LoadDataSource(LogDataSource::FileContents(
                        FileSource::FileDialog {
                            recommended_store_id: recommended_store_id.clone(),
                            force_store_info: *force_store_info,
                        },
                        file.clone(),
                    )));
            }
            self.open_files_promise = None;
        }

        // NOTE: GPU resource stats are cheap to compute so we always do.
        let gpu_resource_stats = {
            re_tracing::profile_scope!("gpu_resource_stats");

            let egui_renderer = frame
                .wgpu_render_state()
                .expect("Failed to get frame render state")
                .renderer
                .read();

            let render_ctx = egui_renderer
                .callback_resources
                .get::<re_renderer::RenderContext>()
                .expect("Failed to get render context");

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

        // In some (rare) circumstances we run two egui passes in a single frame.
        // This happens on call to `egui::Context::request_discard`.
        let is_start_of_new_frame = egui_ctx.current_pass_index() == 0;
        if is_start_of_new_frame {
            // IMPORTANT: only call this once per FRAME even if we run multiple passes.
            // Otherwise we might incorrectly evict something that was invisible in the first (discarded) pass.
            store_hub.begin_frame_caches();
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
        if let DisplayMode::Loading(source) = self.state.navigation.current() {
            if !self.msg_receive_set().contains(source) {
                self.state.navigation.reset();
            }
        } else if store_hub.active_app().is_none() {
            let apps: std::collections::BTreeSet<&ApplicationId> = store_hub
                .store_bundle()
                .entity_dbs()
                .map(|db| db.application_id())
                .filter(|&app_id| app_id != &StoreHub::welcome_screen_app_id())
                .collect();
            if let Some(app_id) = apps.first().copied() {
                store_hub.set_active_app(app_id.clone());
                // set_active_app will also activate a new entry.
                // Select this entry so it's more obvious to the user which recording
                // is now active.
                match store_hub.active_recording_or_table() {
                    Some(RecordingOrTable::Recording { store_id }) => {
                        self.state
                            .selection_state
                            .set_selection(Item::StoreId(store_id.clone()));
                    }
                    Some(RecordingOrTable::Table { table_id }) => {
                        self.state
                            .selection_state
                            .set_selection(Item::TableId(table_id.clone()));
                    }
                    None => {}
                }
            } else {
                self.state.navigation.reset();
                store_hub.set_active_app(StoreHub::welcome_screen_app_id());
            }
        }

        self.prefetch_chunks(&mut store_hub);

        {
            let (storage_context, store_context) = store_hub.read_context();

            let blueprint_query = store_context.as_ref().map_or_else(
                BlueprintUndoState::default_query,
                |store_context| {
                    self.state
                        .blueprint_query_for_viewer(store_context.blueprint)
                },
            );

            let app_blueprint = AppBlueprint::new(
                store_context.as_ref().map(|ctx| ctx.blueprint),
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
                &storage_context,
                store_stats.as_ref(),
            );

            if re_ui::CUSTOM_WINDOW_DECORATIONS {
                // Paint the main window frame on top of everything else
                paint_native_window_frame(egui_ctx);
            }

            if let Some(cmd) = self.cmd_palette.show(
                egui_ctx,
                &crate::open_url_description::command_palette_parse_url,
            ) {
                match cmd {
                    re_ui::CommandPaletteAction::UiCommand(cmd) => {
                        self.command_sender.send_ui(cmd);
                    }
                    re_ui::CommandPaletteAction::OpenUrl(url_desc) => {
                        match url_desc.url.parse::<ViewerOpenUrl>() {
                            Ok(url) => {
                                url.open(
                                    egui_ctx,
                                    &OpenUrlOptions {
                                        follow_if_http: false,
                                        select_redap_source_when_loaded: true,
                                        show_loader: true,
                                    },
                                    &self.command_sender,
                                );
                            }
                            Err(err) => {
                                re_log::warn!("{err}");
                            }
                        }

                        // Note that we can't use `ui.ctx().open_url(egui::OpenUrl::same_tab(uri))` here because..
                        // * the url redirect in `check_for_clicked_hyperlinks` wouldn't be hit
                        // * we don't actually want to open any URLs in the browser here ever, only ever into the current viewer
                    }
                }
            }

            Self::handle_dropping_files(egui_ctx, &storage_context, &self.command_sender);

            // Run pending commands last (so we don't have to wait for a repaint before they are run):
            let display_mode = self.state.navigation.current().clone();
            self.run_pending_ui_commands(
                egui_ctx,
                &app_blueprint,
                &storage_context,
                store_context.as_ref(),
                &display_mode,
            );
        }
        self.run_pending_system_commands(&mut store_hub, egui_ctx);

        self.update_history(&store_hub);

        // Return the `StoreHub` to the Viewer so we have it on the next frame
        self.store_hub = Some(store_hub);

        {
            // Check for returned screenshots:
            let screenshots: Vec<_> = egui_ctx.input(|i| {
                i.raw
                    .events
                    .iter()
                    .filter_map(|event| {
                        if let egui::Event::Screenshot {
                            image, user_data, ..
                        } = event
                        {
                            Some((image.clone(), user_data.clone()))
                        } else {
                            None
                        }
                    })
                    .collect()
            });

            for (image, user_data) in screenshots {
                self.process_screenshot_result(&image, &user_data);
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(&mut *self)
    }
}

fn paint_background_fill(ui: &egui::Ui) {
    // This is required because the streams view (time panel)
    // has rounded top corners, which leaves a gap.
    // So we fill in that gap (and other) here.
    // Of course this does some over-draw, but we have to live with that.

    let tokens = ui.tokens();

    ui.painter().rect_filled(
        ui.max_rect().shrink(0.5),
        tokens.native_window_corner_radius(),
        ui.visuals().panel_fill,
    );
}

fn paint_native_window_frame(egui_ctx: &egui::Context) {
    let tokens = egui_ctx.tokens();

    let painter = egui::Painter::new(
        egui_ctx.clone(),
        egui::LayerId::new(egui::Order::TOP, egui::Id::new("native_window_frame")),
        egui::Rect::EVERYTHING,
    );

    painter.rect_stroke(
        egui_ctx.content_rect(),
        tokens.native_window_corner_radius(),
        egui_ctx.tokens().native_frame_stroke,
        egui::StrokeKind::Inside,
    );
}

fn preview_files_being_dropped(egui_ctx: &egui::Context) {
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
                .style()
                .visuals
                .extreme_bg_color
                .gamma_multiply_u8(192),
        );
        painter.text(
            screen_rect.center(),
            Align2::CENTER_CENTER,
            text,
            TextStyle::Body.resolve(&egui_ctx.style()),
            egui_ctx.style().visuals.strong_text_color(),
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

fn save_active_recording(
    app: &mut App,
    store_context: Option<&StoreContext<'_>>,
    loop_selection: Option<(TimelineName, re_log_types::AbsoluteTimeRangeF)>,
) -> anyhow::Result<()> {
    let Some(entity_db) = store_context.as_ref().map(|view| view.recording) else {
        // NOTE: Can only happen if saving through the command palette.
        anyhow::bail!("No recording data to save");
    };

    save_recording(app, entity_db, loop_selection)
}

fn save_recording(
    app: &mut App,
    entity_db: &EntityDb,
    loop_selection: Option<(TimelineName, re_log_types::AbsoluteTimeRangeF)>,
) -> anyhow::Result<()> {
    let rrd_version = entity_db
        .store_info()
        .and_then(|info| info.store_version)
        .unwrap_or(re_build_info::CrateVersion::LOCAL);

    let file_name = if let Some(recording_name) = entity_db
        .recording_info_property::<re_sdk_types::components::Name>(
            re_sdk_types::archetypes::RecordingInfo::descriptor_name().component,
        ) {
        format!("{}.rrd", sanitize_file_name(&recording_name))
    } else {
        "data.rrd".to_owned()
    };

    let title = if loop_selection.is_some() {
        "Save loop selection"
    } else {
        "Save recording"
    };

    save_entity_db(
        app,
        rrd_version,
        file_name,
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
    let new_store_id = store_context
        .blueprint
        .store_id()
        .clone()
        .with_recording_id(RecordingId::random());
    let messages = store_context.blueprint.to_messages(None).map(|mut msg| {
        if let Ok(msg) = &mut msg {
            msg.set_store_id(new_store_id.clone());
        }
        msg
    });

    let file_name = format!(
        "{}.rbl",
        crate::saving::sanitize_app_id(store_context.application_id())
    );
    let title = "Save blueprint";

    save_entity_db(app, rrd_version, file_name, title.to_owned(), messages)
}

// TODO(emilk): unify this with `ViewerContext::save_file_dialog`
#[allow(clippy::allow_attributes, clippy::needless_pass_by_ref_mut)] // `app` is only used on native
#[allow(clippy::unnecessary_wraps)] // cannot return error on web
fn save_entity_db(
    #[allow(clippy::allow_attributes, unused_variables)] app: &mut App, // only used on native
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

    let options = re_log_encoding::rrd::EncodingOptions::PROTOBUF_COMPRESSED;
    let mut bytes = Vec::new();
    re_log_encoding::Encoder::encode_into(rrd_version, options, messages, &mut bytes)?;
    file_handle.write(&bytes).await.context("Failed to save")
}

/// Propagates [`re_viewer_context::TimeControlResponse`] to [`ViewerEventDispatcher`].
fn handle_time_ctrl_event(
    recording: &EntityDb,
    events: Option<&ViewerEventDispatcher>,
    response: &re_viewer_context::TimeControlResponse,
) {
    let Some(events) = events else {
        return;
    };

    if let Some(playing) = response.playing_change {
        events.on_play_state_change(recording, playing);
    }

    if let Some((timeline, time)) = response.timeline_change {
        events.on_timeline_change(recording, timeline, time);
    }

    if let Some(time) = response.time_change {
        events.on_time_update(recording, time);
    }
}
