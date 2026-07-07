use std::sync::Arc;

use egui::{FocusDirection, Key};
use re_auth::credentials::CredentialsProvider as _;
use re_build_info::CrateVersion;
use re_byte_size::{MemUsageTree, MemUsageTreeCapture};
use re_capabilities::MainThreadToken;
use re_data_source::{AuthErrorHandler, FileContents, LogDataSource};
use re_entity_db::InstancePath;
use re_entity_db::entity_db::EntityDb;
use re_log_channel::{LogReceiverSet, RecordingOpenBehavior, SaveScreenshotError};
use re_log_types::{ApplicationId, FileSource, RecordingId, StoreId};
use re_redap_client::ConnectionRegistryHandle;
use re_sdk_types::blueprint::components::PlayState;
use re_ui::{ContextExt as _, UICommand, UICommandSender as _, notifications};
use re_viewer_context::open_url::{OpenUrlOptions, ViewerOpenUrl};
use re_viewer_context::store_hub::{BlueprintPersistence, StoreHub};
use re_viewer_context::{
    AppBlueprintCtx, AppOptions, AsyncRuntimeHandle, AuthContext, CommandReceiver, CommandSender,
    ComponentUiRegistry, EditRedapServerModalCommand, FallbackProviderRegistry, Item, NeedsRepaint,
    Route, SystemCommand, SystemCommandSender as _, TimeControlCommand, ViewClass,
    ViewClassRegistry, ViewClassRegistryError, command_channel,
};

use crate::app_blueprint::{AppBlueprint, PanelStateOverrides};
use crate::background_tasks::BackgroundTasks;
use crate::event::ViewerEventDispatcher;
use crate::latency_tracker::ServerLatencyTrackers;
use crate::startup_options::StartupOptions;
use crate::{AppState, command_palette::CommandPaletteAction};

mod add_data_source;
mod command_handling;
mod logic;
mod ui;

// ----------------------------------------------------------------------------

/// Storage key used to store the last run Rerun version.
///
/// This is then used to detect if the user has recently upgraded Rerun.
const RERUN_VERSION_KEY: &str = "rerun.version";

const REDAP_TOKEN_KEY: &str = "rerun.redap_token";

/// The egui temp-data key under which the `on_begin_pass` hook stashes the timeline
/// keyboard shortcut it consumed this frame.
///
/// The hook (which only has an [`egui::Context`]) consumes these keys early, but `App::ui`
/// pairs the stashed command with the *live* active recording and dispatches it — so it can
/// never target a stale recording.
fn pending_timeline_shortcut_key() -> egui::Id {
    egui::Id::new("rerun_pending_timeline_shortcut")
}

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
    texture_readback: crate::texture_readback::TextureReadbacks,

    /// Notifiers waiting for a file-path screenshot to finish writing.
    pending_screenshot_notifiers: std::collections::HashMap<
        camino::Utf8PathBuf,
        futures::channel::mpsc::UnboundedSender<Result<(), SaveScreenshotError>>,
    >,

    #[cfg(target_arch = "wasm32")]
    pub(crate) popstate_listener: Option<crate::web_history::PopstateListener>,

    #[cfg(not(target_arch = "wasm32"))]
    profiler: re_tracing::Profiler,

    /// Active in-memory profile capture, if any.
    #[cfg(not(target_arch = "wasm32"))]
    profile_capture: Option<re_tracing::ProfileCapture>,

    /// Listens to the local text log stream
    text_log_rx: crossbeam::channel::Receiver<re_log::LogMsg>,

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

    dev_panel: crate::dev_panel::DevPanel,
    dev_panel_open: bool,
    pub(crate) external_memory_users: crate::external_memory::ExternalMemoryUsers,

    /// Cached app overhead: total memory use minus sum of all recording chunk sizes.
    /// Updated during GC when we have a fresh memory snapshot.
    cached_app_overhead_bytes: Option<u64>,

    egui_debug_panel_open: bool,

    /// Last time the latency was deemed interesting.
    ///
    /// Note that initializing with an "old" `Instant` won't work reliably cross platform
    /// since `Instant`'s counter may start at program start.
    pub(crate) latest_latency_interest: Option<web_time::Instant>,

    /// Measures how long a frame takes to paint
    pub(crate) frame_time_history: egui::util::History<f32>,

    /// The last theme we pushed to the OS window (via [`egui::ViewportCommand::SetTheme`]).
    last_window_theme: Option<egui::SystemTheme>,

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

    pub(crate) server_latency_trackers: ServerLatencyTrackers,

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
    pub fn with_commands(
        main_thread_token: MainThreadToken,
        build_info: re_build_info::BuildInfo,
        app_env: crate::AppEnvironment,
        startup_options: StartupOptions,
        creation_context: &eframe::CreationContext<'_>,
        connection_registry: Option<ConnectionRegistryHandle>,
        tokio_runtime: AsyncRuntimeHandle,
        text_log_rx: crossbeam::channel::Receiver<re_log::LogMsg>,
        command_channel: (CommandSender, CommandReceiver),
    ) -> Self {
        re_tracing::profile_function!();

        let is_test = app_env.is_test();

        #[cfg_attr(
            not(all(feature = "internal_catalog", not(target_arch = "wasm32"))),
            expect(unused_variables)
        )]
        let connection_registry_was_provided = connection_registry.is_some();
        let connection_registry = connection_registry
            .unwrap_or_else(re_redap_client::ConnectionRegistry::new_with_stored_credentials);

        // Only subscribe to auth changes and load credentials if we're supposed to use stored credentials.
        // This prevents tests from being affected by stored credentials on the developer's machine.
        if connection_registry.should_use_stored_credentials() {
            let command_sender = command_channel.0.clone();
            re_auth::credentials::subscribe_auth_changes(move |user| {
                command_sender.send_system(SystemCommand::OnAuthChanged(user.map(|user| {
                    AuthContext {
                        email: user.email,
                        org_name: user.org_name,
                    }
                })));
            });

            // Call get_token once so the auth state is initialized.
            tokio_runtime.spawn_future(async move {
                re_auth::credentials::CliCredentialsProvider::new()
                    .get_token()
                    .await
                    .ok();
            });
        }

        if connection_registry.should_use_stored_credentials()
            && let Some(storage) = creation_context.storage
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

        if is_test {
            creation_context.egui_ctx.mark_as_test();
            state.app_options = AppOptions::test();
        }

        #[cfg(all(feature = "internal_catalog", not(target_arch = "wasm32")))]
        let connection_registry = if state.app_options.experimental.use_internal_catalog
            && !connection_registry_was_provided
            && connection_registry.internal_origin().is_none()
        {
            let catalog = crate::internal_catalog::build(std::net::SocketAddr::from((
                std::net::Ipv4Addr::LOCALHOST,
                re_uri::DEFAULT_PROXY_PORT,
            )));
            connection_registry.with_internal((catalog.origin, catalog.connection))
        } else {
            connection_registry
        };

        let reflection = re_sdk_types::reflection::generate_reflection().unwrap_or_else(|err| {
            re_log::error!(
                "Failed to create list of serialized default values for components: {err}"
            );
            Default::default()
        });

        let mut component_fallback_registry =
            re_component_fallbacks::create_component_fallback_registry();

        let view_class_registry = crate::default_views::create_view_class_registry(
            &reflection,
            &state.app_options,
            &mut component_fallback_registry,
        )
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
            // This is a workaround consuming the space and arrow keys so we can use them as timeline shortcuts.
            // Egui's built in behavior is to interact with focus, and we don't want that.
            // TODO(emilk/egui#7899): allow consuming events before egui uses them to move keyboard focus.
            // TODO(emilk/egui#7659): allow disabling certain egui shortcuts.
            creation_context.egui_ctx.on_begin_pass(
                "rerun-kb-shortcuts",
                Arc::new(move |ctx| {
                    // egui has already listened for arrow keys before this point,
                    // so in order for the arrow keys to NOT move the focus, we need to
                    // undo that focus change here:
                    let reset_focus_direction = ctx.input_mut(|i| {
                        i.key_pressed(Key::ArrowLeft) || i.key_pressed(Key::ArrowRight)
                    });

                    if reset_focus_direction {
                        ctx.memory_mut(|mem| {
                            mem.move_focus(FocusDirection::None);
                        });
                    }

                    // Consume the timeline shortcuts (space/arrows/home/end) here, before egui
                    // uses them for focus/scroll. We only stash which command was pressed; it is
                    // paired with the live active recording and dispatched later, in `App::ui`, so
                    // it can never target a stale recording.
                    if let Some(kind) = re_ui::consume_timeline_shortcut(ctx) {
                        ctx.data_mut(|data| {
                            data.insert_temp(pending_timeline_shortcut_key(), kind);
                        });
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
            texture_readback: Default::default(),
            pending_screenshot_notifiers: Default::default(),

            #[cfg(target_arch = "wasm32")]
            popstate_listener: None,

            #[cfg(not(target_arch = "wasm32"))]
            profiler: Default::default(),

            #[cfg(not(target_arch = "wasm32"))]
            profile_capture: None,

            text_log_rx,
            component_ui_registry,
            component_fallback_registry,
            rx_log: Default::default(),

            #[cfg(target_arch = "wasm32")]
            open_files_promise: Default::default(),

            state,
            background_tasks: Default::default(),
            store_hub: Some(StoreHub::new(
                if is_test {
                    noop_blueprint_loader()
                } else {
                    blueprint_loader()
                },
                &crate::app_blueprint::setup_welcome_screen_blueprint,
            )),
            notifications: notifications::NotificationUi::new(creation_context.egui_ctx.clone()),

            dev_panel: Default::default(),
            dev_panel_open: false,
            external_memory_users: crate::external_memory::ExternalMemoryUsers::default_users(),
            cached_app_overhead_bytes: None,

            egui_debug_panel_open: false,

            latest_latency_interest: None,

            frame_time_history: egui::util::History::new(1..100, 0.5),
            last_window_theme: None,

            command_sender,
            command_receiver,
            cmd_palette: Default::default(),

            view_class_registry,

            panel_state_overrides_active: true,
            panel_state_overrides,

            reflection,

            event_dispatcher,

            connection_registry,
            server_latency_trackers: ServerLatencyTrackers::default(),
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

    pub fn reflection(&self) -> &re_types_core::reflection::Reflection {
        &self.reflection
    }

    pub fn app_options_mut(&mut self) -> &mut AppOptions {
        self.state.app_options_mut()
    }

    pub fn app_env(&self) -> &crate::AppEnvironment {
        &self.app_env
    }

    /// Whether we are responsible for painting a window frame.
    ///
    /// Not enabled on Windows ever since there the OS puts some margin & frame around the window content either way.
    pub(crate) fn custom_window_frame(&self) -> bool {
        self.custom_window_decorations() && !cfg!(target_os = "windows")
    }

    /// The active recording [`StoreId`], if any, derived from the current [`Route`].
    pub fn active_recording_id(&self) -> Option<&StoreId> {
        self.state.active_recording_id()
    }

    /// Select `item` and navigate the viewer to it (if it maps to a route).
    fn select_and_navigate_to(&self, item: &Item) {
        self.command_sender
            .send_system(SystemCommand::set_selection(item.clone()));
        if let Some(route) = Route::from_item(item) {
            self.command_sender
                .send_system(SystemCommand::SetRoute(route));
        }
    }

    /// Open a content URL in the viewer.
    pub fn open_url_or_file(&self, url: &str) {
        match ViewerOpenUrl::parse_with_options(
            url,
            &re_data_source::FromUriOptions {
                accept_extensionless_http: true,
                ..Default::default()
            },
        ) {
            Ok(url) => {
                url.open(
                    &self.egui_ctx,
                    &OpenUrlOptions {
                        follow: false,
                        recording_open_behavior: RecordingOpenBehavior::OpenAndSelect,
                        show_loader: true,
                    },
                    &self.command_sender,
                );
            }
            Err(err) => {
                if err.to_string().contains(url) {
                    re_log::error!("{err}");
                } else {
                    re_log::error!(?url, "Failed to open URL: {err}");
                }
            }
        }
    }

    pub fn is_screenshotting(&self) -> bool {
        self.screenshotter.is_screenshotting()
    }

    /// Update the active [`re_viewer_context::TimeControl`]. And if the blueprint inspection
    /// panel is open, also open that time control.
    fn move_time(&mut self) {
        let stable_dt = self.egui_ctx.input(|i| i.stable_dt);

        let Some(store_hub) = &self.store_hub else {
            return;
        };

        if let Some(store_id) = self.active_recording_id()
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

            if let Some(recording) = store_hub.entity_db(store_id) {
                // Are we still connected to the data source for the current store?
                let more_data_is_streaming_in =
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
                let response = time_ctrl.update(
                    recording,
                    &re_viewer_context::TimeControlUpdateParams {
                        stable_dt,
                        more_data_is_streaming_in,
                        is_buffering: recording.is_buffering(),
                        should_diff_state: true,
                    },
                    Some(&bp_ctx),
                );

                if response.needs_repaint == NeedsRepaint::Yes {
                    self.egui_ctx.request_repaint();
                }

                command_handling::handle_time_ctrl_event(
                    recording,
                    self.event_dispatcher.as_ref(),
                    &response,
                );
            }

            if self.app_options().inspect_blueprint_timeline {
                // We ignore most things from the time control response for the blueprint but still
                // need to repaint if requested.
                let re_viewer_context::TimeControlResponse {
                    needs_repaint,
                    playing_change: _,
                    timeline_change: _,
                    time_change: _,
                } = self.state.blueprint_time_control.update(
                    blueprint,
                    &re_viewer_context::TimeControlUpdateParams {
                        stable_dt,
                        more_data_is_streaming_in: true,
                        is_buffering: false,
                        should_diff_state: false,
                    },
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

        // Tick time controls for preview recordings shown in grid or table cards.
        // Runs even when there's no active recording.
        if self
            .state
            .update_preview_time_controls(store_hub, stable_dt)
            == re_viewer_context::NeedsRepaint::Yes
        {
            self.egui_ctx.request_repaint();
        }
    }

    pub fn msg_receive_set(&self) -> &LogReceiverSet {
        &self.rx_log
    }

    /// The registry of component UIs used by the viewer.
    pub fn component_ui_registry_mut(&mut self) -> &mut ComponentUiRegistry {
        &mut self.component_ui_registry
    }

    /// Registers runtime reflection metadata for a custom archetype.
    pub fn add_archetype_reflection(
        &mut self,
        archetype_name: re_sdk_types::ArchetypeName,
        archetype_reflection: re_sdk_types::reflection::ArchetypeReflection,
    ) {
        for field in &archetype_reflection.fields {
            let descriptor = field.component_descriptor(archetype_name);
            self.reflection
                .component_identifiers
                .insert(descriptor.component, descriptor);
        }

        self.reflection
            .archetypes
            .insert(archetype_name, archetype_reflection);
    }

    /// Adds a new view class to the viewer.
    pub fn add_view_class<T: ViewClass + Default + 'static>(
        &mut self,
    ) -> Result<(), ViewClassRegistryError> {
        self.view_class_registry.add_class::<T>(
            &self.reflection,
            &self.state.app_options,
            &mut self.component_fallback_registry,
        )
    }

    /// Extends an already registered view class with additional systems (visualizers, context systems, fallbacks, etc.).
    ///
    /// **WARNING:** Many parts of the viewer assume that all views & visualizers are registered before the first frame is rendered.
    /// Doing so later in the application life cycle may cause unexpected behavior.
    pub fn extend_view_class(
        &mut self,
        view_class: re_sdk_types::ViewClassIdentifier,
        register_fn: impl FnOnce(
            &mut re_viewer_context::ViewSystemRegistrator<'_>,
        ) -> Result<(), ViewClassRegistryError>,
    ) -> Result<(), ViewClassRegistryError> {
        self.view_class_registry.extend_class(
            view_class,
            &self.reflection,
            &self.state.app_options,
            &mut self.component_fallback_registry,
            register_fn,
        )
    }

    /// If we're on web and use web history this updates the
    /// web address bar and updates history.
    ///
    /// Otherwise this updates the viewer tracked history.
    fn update_history(&mut self, store_hub: &StoreHub) {
        if self.startup_options().web_history_enabled() {
            // We don't want to spam the web history API with changes, because
            // otherwise it will start complaining about it being an insecure
            // operation.
            //
            // This is a kind of hacky way to fix that: If there are currently any
            // inputs, don't update the web address bar. This works for most cases
            // because you need to hold down pointer to aggressively scrub, need to
            // hold down key inputs to quickly step through the timeline.
            #[cfg(target_arch = "wasm32")]
            if !self.egui_ctx.egui_is_using_pointer()
                && self
                    .egui_ctx
                    .input(|input| !input.any_touches() && input.keys_down.is_empty())
            {
                self.update_web_history(store_hub);
            }
        } else {
            self.update_viewer_history(store_hub);
        }
    }

    /// Updates the viewer tracked history
    fn update_viewer_history(&mut self, store_hub: &StoreHub) {
        let route = self.state.navigation.current();
        let time_ctrl = route
            .recording_id()
            .and_then(|id| self.state.time_control(id));

        let selection = self.state.selection_state.selected_items();

        let Ok(url) = ViewerOpenUrl::from_context_expanded(store_hub, route, time_ctrl, selection)
        else {
            return;
        };

        self.state.history.update_current_url(url);
    }

    /// Updates the web address and web history.
    #[cfg(target_arch = "wasm32")]
    fn update_web_history(&self, store_hub: &StoreHub) {
        let route = self.state.navigation.current();
        let time_ctrl = route
            .recording_id()
            .and_then(|id| self.state.time_control(id));
        let selection = self.state.selection_state.selected_items();

        let Ok(url) = ViewerOpenUrl::from_context_expanded(store_hub, route, time_ctrl, selection)
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

    /// Applies a fragment.
    ///
    /// Does *not* switch the active recording.
    fn go_to_dataset_data(&self, store_id: StoreId, fragment: re_uri::Fragment) {
        let time_commands = TimeControlCommand::from_url_fragment(&fragment);

        if let Some(selection) = fragment.selection {
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
                .send_system(SystemCommand::set_selection(item));
        }

        if !time_commands.is_empty() {
            self.command_sender
                .send_system(SystemCommand::TimeControlCommands {
                    store_id,
                    time_commands,
                });
        }
    }

    pub fn recording_db(&self) -> Option<&EntityDb> {
        let store_hub = self.store_hub.as_ref()?;
        let recording_id = self.active_recording_id()?;
        store_hub.entity_db(recording_id)
    }

    /// Returns a [`re_chunk_store::LatestAtQuery`] for the active recording's current timeline
    /// position, suitable for querying frame data from [`Self::recording_db`].
    pub fn current_query(&self) -> Option<re_chunk_store::LatestAtQuery> {
        let store_id = self.active_recording_id()?;
        self.state
            .time_controls
            .get(store_id)
            .map(|tc| tc.current_query())
    }

    // NOTE: Relying on `self` is dangerous, as this is called during a time where some internal
    // fields may have been temporarily `take()`n out. Keep this a static method.
    fn handle_dropping_files(
        egui_ctx: &egui::Context,
        command_sender: &CommandSender,
        route: &Route,
    ) {
        #![allow(clippy::allow_attributes, clippy::needless_continue)] // false positive, depending on target_arch

        ui::preview_files_being_dropped(egui_ctx);

        let dropped_files = egui_ctx.input_mut(|i| std::mem::take(&mut i.raw.dropped_files));

        if dropped_files.is_empty() {
            return;
        }

        egui_ctx.request_repaint();

        let mut force_store_info = false;

        for file in dropped_files {
            let active_store_id = route
                .recording_id()
                .cloned()
                // Don't redirect data to the welcome screen.
                .filter(|store_id| store_id.application_id() != StoreHub::welcome_screen_app_id())
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
                command_sender.send_system(SystemCommand::LoadDataSource(
                    LogDataSource::FilePath {
                        file_source: FileSource::DragAndDrop {
                            recommended_store_id: Some(active_store_id.clone()),
                            force_store_info,
                        },
                        path,
                        follow: false,
                    },
                ));
            }
        }
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
                notify,
            } = (*info).clone();

            // Only used in the native `SaveToPath` branch below.
            #[cfg(target_arch = "wasm32")]
            let _ = notify;

            let rgba = if let Some(ui_rect) = ui_rect {
                Arc::new(image.region(&ui_rect, Some(pixels_per_point)))
            } else {
                image.clone()
            };

            match target {
                re_viewer_context::ScreenshotTarget::CopyToClipboard => {
                    self.egui_ctx.copy_image((*rgba).clone());
                }

                re_viewer_context::ScreenshotTarget::SaveToPathFromFileDialog => {
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

                re_viewer_context::ScreenshotTarget::SaveToPath(file_path) => {
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        let rgba = rgba.clone();
                        let notifier = self.pending_screenshot_notifiers.remove(&file_path);
                        let Some(rgba_image) = image::RgbaImage::from_vec(
                            rgba.width() as _,
                            rgba.height() as _,
                            bytemuck::pod_collect_to_vec(&rgba.pixels),
                        ) else {
                            re_log::error!("Failed to create image from screenshot data");
                            if let Some(notifier) = notifier {
                                notifier
                                    .unbounded_send(Err(SaveScreenshotError::InvalidImageData))
                                    .ok();
                            }
                            return;
                        };

                        // Convert to RGB8 so it works with JPG and other formats that don't support alpha.
                        // (There's nothing interesting in the alpha channel anyways.)
                        let rgb_image = image::DynamicImage::ImageRgba8(rgba_image).to_rgb8();

                        let result = match rgb_image.save(&file_path) {
                            Ok(()) => {
                                // Only show a user-facing toast for user-initiated screenshots.
                                if notify {
                                    re_log::info!("Saved screenshot to {file_path:?}");
                                } else {
                                    re_log::debug!("Saved screenshot to {file_path:?}");
                                }
                                Ok(())
                            }
                            Err(err) => {
                                re_log::error!(?file_path, "Failed to save screenshot: {err}");
                                // Image library has the bad habit of creating the file even when it fails e.g. due to unsupported format. Remove it again.
                                std::fs::remove_file(&file_path).ok();
                                Err(SaveScreenshotError::SaveToPathFailed {
                                    path: file_path.to_string(),
                                    reason: err.to_string(),
                                })
                            }
                        };

                        if let Some(notifier) = notifier {
                            notifier.unbounded_send(result).ok();
                        }
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        re_log::error!(
                            "Saving screenshots to a path is not supported on web. Attempted to save to: {file_path:?}"
                        );
                    }
                }
            }
        } else {
            #[cfg(not(target_arch = "wasm32"))] // no full-app screenshotting on web
            if user_data
                .data
                .as_ref()
                .is_some_and(|data| data.is::<crate::screenshotter::FullAppScreenshot>())
            {
                self.screenshotter.save(&self.egui_ctx, image);
            }
            // Ignore any other screenshot requests
        }
    }
}

impl eframe::App for App {
    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        if self.custom_window_decorations() {
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

    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.logic_impl(ctx, frame);
    }

    /// Called when application need to be repainted
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        #[cfg(all(not(target_arch = "wasm32"), feature = "perf_telemetry_tracy"))]
        re_perf_telemetry::external::tracing_tracy::client::frame_mark();

        #[cfg(not(target_arch = "wasm32"))]
        if let Some(capture) = &self.profile_capture {
            if capture.is_done() {
                if let Some(capture) = self.profile_capture.take()
                    && let Err(err) = save_profile_trace(&capture.finish())
                {
                    re_log::error!("Failed to save profile trace: {err}");
                }
            } else {
                ui.ctx().request_repaint();
            }
        }

        if let Some(seconds) = frame.info().cpu_usage {
            self.frame_time_history.add(ui.input(|i| i.time), seconds);
        }

        // NOTE: Memory stats can be very costly to compute, so only do so if the dev panel is opened.
        let mem_usage_tree = self
            .dev_panel_open
            .then(|| re_byte_size::NamedMemUsageTree::new("App", self.capture_mem_usage_tree()));

        self.external_memory_users.update();

        #[cfg(target_arch = "wasm32")]
        if self.startup_options.enable_history {
            // Handle pressing the back/forward mouse buttons explicitly, since eframe catches those.
            let back_pressed = ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Extra1));
            let fwd_pressed = ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Extra2));

            if back_pressed {
                crate::web_history::go_back();
            }
            if fwd_pressed {
                crate::web_history::go_forward();
            }
        }

        self.server_latency_trackers
            .update(&self.connection_registry);

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
            ui.send_viewport_cmd(egui::ViewportCommand::InnerSize(
                resolution_in_points.into(),
            ));
        }

        #[cfg(not(target_arch = "wasm32"))]
        if self.screenshotter.update(ui).quit {
            ui.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        if self.app_options().memory_limit.is_unlimited() {
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

        // NOTE: Store and caching stats are very costly to compute: only do so if the dev panel
        // is opened.
        let store_stats = self.dev_panel_open.then(|| store_hub.stats());

        // do early, before doing too many allocations
        let store_bundle_for_streaming = self
            .dev_panel_open
            .then(|| store_hub.store_bundle() as &re_entity_db::StoreBundle);
        self.dev_panel.update(
            &gpu_resource_stats,
            store_stats.as_ref(),
            store_bundle_for_streaming,
        );

        self.purge_memory_if_needed(&mut store_hub); // Call BEFORE `begin_frame_caches`

        // In some (rare) circumstances we run two egui passes in a single frame.
        // This happens on call to `egui::Context::request_discard`.
        let is_start_of_new_frame = ui.current_pass_index() == 0;
        if is_start_of_new_frame {
            // IMPORTANT: only call this once per FRAME even if we run multiple passes.
            // Otherwise we might incorrectly evict something that was invisible in the first (discarded) pass.
            store_hub.begin_frame_caches(self.active_recording_id()); // Call AFTER `purge_memory_if_needed`
        }

        ui::file_saver_progress_ui(ui, &mut self.background_tasks); // toasts for background file saver

        // Make sure some app is active
        // Must be called before `read_context` below.
        if let Route::Loading(source) = self.state.navigation.current() {
            if !self.msg_receive_set().contains(source) {
                // The stream finished and may have produced a recording without triggering
                // automatic navigation. So we try that before defaulting to showing the
                // Welcome screen.
                let loaded_recording = store_hub
                    .find_recording_store_by_source(source)
                    .map(|db| db.store_id().clone());

                if let Some(store_id) = loaded_recording {
                    re_log::debug!("Stream completed, navigating to loaded recording {store_id:?}");
                    store_hub.load_blueprint_and_caches(&store_id, &self.view_class_registry);
                    self.state.navigation.replace(Route::LocalRecording {
                        recording_id: store_id,
                    });
                } else if let Some(re_uri::RedapUri::DatasetData(uri)) = source.redap_uri()
                    && self.connection_registry.error_for_uri(uri).is_some()
                {
                    // Do nothing, the loading screen will show the error and a button to go back to start screen.
                } else {
                    re_log::debug!("No recording found from loading source, resetting navigation");
                    self.state.navigation.reset();
                }
            }
        } else if !matches!(
            self.state.navigation.current(),
            Route::ChunkStoreBrowser { .. }
        ) {
            // If the current route points to a stale recording, find a new valid state.
            let route_is_valid = self
                .state
                .navigation
                .current()
                .recording_id()
                .is_none_or(|recording_id| store_hub.entity_db(recording_id).is_some());

            if !route_is_valid {
                let any_other_app_id: Option<ApplicationId> = store_hub
                    .store_bundle()
                    .entity_dbs()
                    .map(|db| db.application_id())
                    .filter(|app_id| *app_id != StoreHub::welcome_screen_app_id())
                    .min()
                    .cloned();
                if let Some(app_id) = any_other_app_id {
                    store_hub.load_persisted_blueprints_for_app(&app_id);
                    if let Some(recording_id) = store_hub.earliest_recording_for_app(&app_id) {
                        store_hub
                            .load_blueprint_and_caches(&recording_id, &self.view_class_registry);
                        self.state
                            .selection_state
                            .set_selection(Item::StoreId(recording_id.clone()));
                        self.state
                            .navigation
                            .replace(Route::LocalRecording { recording_id });
                    } else {
                        self.state.navigation.reset();
                    }
                } else {
                    self.state.navigation.reset();
                }
            }
        }

        {
            let active_route = self.state.navigation.current();

            // Read-only copy of time control state (to avoid borrow checker issues with mutable state access).
            let active_time_ctrl = active_route
                .recording_id()
                .and_then(|id| self.state.time_controls.get(id).cloned())
                .unwrap_or_default();

            let (storage_context, store_context) =
                store_hub.read_context(active_route, &active_time_ctrl);

            let blueprint = store_context.as_ref().map(|ctx| ctx.blueprint);
            let blueprint_query = self.state.blueprint_query_for_viewer(blueprint);

            let app_blueprint = AppBlueprint::new(
                blueprint,
                &blueprint_query,
                ui,
                self.panel_state_overrides_active
                    .then_some(self.panel_state_overrides),
            );

            self.ui_impl(
                ui,
                frame,
                &app_blueprint,
                &gpu_resource_stats,
                store_context.as_ref(),
                &storage_context,
                mem_usage_tree,
                store_stats.as_ref(),
            );

            if self.custom_window_frame() {
                ui::paint_custom_window_frame(ui);
            }

            let selected_redap_server = if let Some(Item::RedapServer(origin)) =
                self.state.selection_state.selected_items().single_item()
            {
                Some(origin.clone())
            } else {
                None
            };

            let active_recording_id = store_context
                .as_ref()
                .map(|ctx| ctx.recording_store_id().clone());

            // The Redap entry currently being viewed (if any), so its commands (e.g. refresh)
            // are offered in the command palette.
            let current_redap_entry = match self.state.navigation.current() {
                Route::RedapEntry { origin, kind } => {
                    kind.entry_id().map(|entry_id| (origin.clone(), entry_id))
                }
                _ => None,
            };

            let cmd_env = re_ui::CommandEnvironment {
                recording: active_recording_id.clone(),
                has_editable_redap_server: selected_redap_server
                    .as_ref()
                    .is_some_and(|origin| !self.state.redap_servers.is_internal_server(origin)),
                redap_server: selected_redap_server,
                redap_entry: current_redap_entry,
            };

            // Handle keyboard shortcuts, now that we have a live `CommandEnvironment`:
            {
                use re_ui::{
                    RecordingCommandSender as _, RedapServerCommandSender as _,
                    TableCommandSender as _,
                };

                // Non-timeline shortcuts, resolved against the current environment:
                if let Some(resolved) = re_ui::listen_for_kb_shortcuts(ui.ctx(), &cmd_env) {
                    match resolved {
                        re_ui::ResolvedCommand::Ui(cmd) => self.command_sender.send_ui(cmd),
                        re_ui::ResolvedCommand::Recording(cmd) => {
                            self.command_sender.send_recording_command(cmd);
                        }
                        re_ui::ResolvedCommand::RedapServer(cmd) => {
                            self.command_sender.send_redap_server_command(cmd);
                        }
                        re_ui::ResolvedCommand::Table(cmd) => {
                            self.command_sender.send_table_command(cmd);
                        }
                    }
                }

                // Timeline shortcuts (space/arrows/home/end) were consumed early in
                // `on_begin_pass` and stashed; pair them with the live recording here:
                let pending_timeline = ui.ctx().data_mut(|data| {
                    let key = pending_timeline_shortcut_key();
                    let kind = data.get_temp::<re_ui::RecordingCommandKind>(key);
                    data.remove::<re_ui::RecordingCommandKind>(key);
                    kind
                });
                if let Some(cmd) = pending_timeline.and_then(|kind| kind.for_environment(&cmd_env))
                {
                    self.command_sender.send_recording_command(cmd);
                }
            }

            let mut cmd_palette_provider = crate::command_palette::CommandPaletteProviderImpl {
                recording: store_context.as_ref().map(|ctx| ctx.recording()),
                redap_servers: &self.state.redap_servers,
                cmd_env,
            };
            if let Some(cmd) = self.cmd_palette.show(ui.ctx(), &mut cmd_palette_provider) {
                match cmd {
                    CommandPaletteAction::UiCommand(cmd) => {
                        self.command_sender.send_ui(cmd);
                    }
                    CommandPaletteAction::RecordingCommand(cmd) => {
                        use re_ui::RecordingCommandSender as _;
                        self.command_sender.send_recording_command(cmd);
                    }
                    CommandPaletteAction::RedapServerCommand(cmd) => {
                        use re_ui::RedapServerCommandSender as _;
                        self.command_sender.send_redap_server_command(cmd);
                    }
                    CommandPaletteAction::SelectEntityPath(entity_path) => {
                        self.command_sender
                            .send_system(SystemCommand::set_selection(Item::from(
                                entity_path.clone(),
                            )));
                        self.command_sender
                            .send_system(SystemCommand::SetFocus(entity_path.into()));
                    }
                    CommandPaletteAction::SelectComponentPath(component_path) => {
                        let item = Item::from(component_path);
                        self.command_sender
                            .send_system(SystemCommand::set_selection(item.clone()));
                        self.command_sender
                            .send_system(SystemCommand::SetFocus(item.into()));
                    }
                    CommandPaletteAction::SelectRedapServer(origin) => {
                        self.select_and_navigate_to(&Item::RedapServer(origin));
                    }
                    CommandPaletteAction::SelectRedapEntry {
                        origin, entry_id, ..
                    } => {
                        self.select_and_navigate_to(&Item::RedapEntry {
                            origin,
                            kind: re_viewer_context::RedapEntryKind::Entry(entry_id),
                        });
                    }
                    CommandPaletteAction::TableCommand(cmd) => {
                        use re_ui::TableCommandSender as _;
                        self.command_sender.send_table_command(cmd);
                    }
                    CommandPaletteAction::OpenUrl(url) => {
                        match ViewerOpenUrl::parse_with_options(
                            url.as_str(),
                            &re_data_source::FromUriOptions {
                                accept_extensionless_http: true,
                                ..Default::default()
                            },
                        ) {
                            Ok(url) => {
                                url.open(
                                    ui,
                                    &OpenUrlOptions {
                                        follow: false,
                                        recording_open_behavior:
                                            RecordingOpenBehavior::OpenAndSelect,
                                        show_loader: true,
                                    },
                                    &self.command_sender,
                                );
                            }
                            Err(err) => {
                                re_log::warn!("{err}");
                            }
                        }

                        // Note that we can't use `ui.open_url(egui::OpenUrl::same_tab(uri))` here because..
                        // * the url redirect in `check_for_clicked_hyperlinks` wouldn't be hit
                        // * we don't actually want to open any URLs in the browser here ever, only ever into the current viewer
                    }
                }
            }

            let route = self.state.navigation.current().clone();
            Self::handle_dropping_files(ui, &self.command_sender, &route);

            // Run pending commands last (so we don't have to wait for a repaint before they are run):
            self.run_pending_ui_commands(
                ui,
                &app_blueprint,
                &storage_context,
                store_context.as_ref(),
                &route,
            );
            self.run_pending_recording_commands(
                ui,
                &app_blueprint,
                &storage_context,
                store_context.as_ref(),
            );
        }
        self.run_pending_system_commands(&mut store_hub, ui);

        self.update_history(&store_hub);

        // Return the `StoreHub` to the Viewer so we have it on the next frame
        self.store_hub = Some(store_hub);

        {
            // Check for returned screenshots:
            let screenshots: Vec<_> = ui.input(|i| {
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

#[cfg(not(target_arch = "wasm32"))]
fn save_profile_trace(view: &re_tracing::reexports::puffin::FrameView) -> anyhow::Result<()> {
    let Some(path) = rfd::FileDialog::new()
        .set_file_name("rerun.puffin")
        .set_title("Save profile trace")
        .add_filter("Puffin profile", &["puffin"])
        .save_file()
    else {
        re_log::info!("Profile trace capture cancelled by user.");
        return Ok(());
    };

    let file = std::fs::File::create(&path)?;
    let mut writer = std::io::BufWriter::new(file);
    view.write(&mut writer)?;

    re_log::info!("Saved profile trace to {}", path.display());
    Ok(())
}

impl MemUsageTreeCapture for App {
    fn capture_mem_usage_tree(&self) -> MemUsageTree {
        re_tracing::profile_function!();
        let mut node = re_byte_size::MemUsageNode::default();
        node.add("state", self.state.capture_mem_usage_tree());
        node.add("rx_log", self.rx_log.capture_mem_usage_tree());
        node.add("store_hub", self.store_hub.capture_mem_usage_tree());
        node.add(
            "store_subscribers",
            re_chunk_store::ChunkStore::capture_all_subscribers_mem_usage_tree(),
        );

        let mut globals = re_byte_size::MemUsageNode::new();
        globals.add(
            "forgiving_parse_cache",
            re_log_types::forgiving_parse_cache_bytes_used(),
        );
        globals.add("string_interner", re_string_interner::bytes_used() as u64);
        node.add("globals", globals.into_tree());

        node.into_tree()
    }
}

#[cfg(target_arch = "wasm32")]
fn blueprint_loader() -> BlueprintPersistence {
    // TODO(#2579): implement persistence for web
    noop_blueprint_loader()
}

/// No-op blueprint persistence used on wasm. Also used in tests so that on-disk blueprints from
/// the developer's running viewer don't leak into the test environment.
fn noop_blueprint_loader() -> BlueprintPersistence {
    BlueprintPersistence {
        loader: None,
        saver: None,
        validator: Some(Box::new(crate::blueprint::is_valid_blueprint)),
        deleter: None,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn blueprint_loader() -> BlueprintPersistence {
    use re_entity_db::{EntityDb, StoreBundle};
    use re_log_types::{ApplicationId, StoreKind};

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
        deleter: Some(Box::new(crate::saving::delete_blueprint)),
    }
}
