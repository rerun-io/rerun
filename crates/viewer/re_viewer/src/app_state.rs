use std::borrow::Cow;
use std::str::FromStr as _;

use ahash::HashMap;
use egui::Ui;
use egui::text_edit::TextEditState;
use egui::text_selection::LabelSelectionState;
use re_chunk::TimelineName;
use re_chunk_store::LatestAtQuery;
use re_entity_db::EntityDb;
use re_log_channel::LogReceiverSet;
use re_log_types::{AbsoluteTimeRangeF, StoreId, TableId};
use re_redap_browser::RedapServers;
use re_redap_client::ConnectionRegistryHandle;
use re_sdk_types::blueprint::components::{PanelState, PlayState};
use re_ui::{ContextExt as _, UiExt as _};
use re_viewer_context::open_url::{self, ViewerOpenUrl};
use re_viewer_context::{
    AppOptions, ApplicationSelectionState, AsyncRuntimeHandle, AuthContext, BlueprintContext,
    BlueprintUndoState, CommandSender, ComponentUiRegistry, DataQueryResult, DisplayMode,
    DragAndDropManager, FallbackProviderRegistry, GlobalContext, Item, PerVisualizerInViewClass,
    SelectionChange, StorageContext, StoreContext, StoreHub, SystemCommand,
    SystemCommandSender as _, TableStore, TimeControl, TimeControlCommand, ViewClassRegistry,
    ViewId, ViewStates, ViewerContext, blueprint_timeline,
};
use re_viewport::ViewportUi;
use re_viewport_blueprint::ViewportBlueprint;
use re_viewport_blueprint::ui::add_view_or_container_modal_ui;

use crate::app_blueprint::AppBlueprint;
use crate::app_blueprint_ctx::AppBlueprintCtx;
use crate::navigation::Navigation;
use crate::open_url_description::ViewerOpenUrlDescription;
use crate::ui::settings_screen_ui;
use crate::ui::{CloudState, LoginState};
use crate::{StartupOptions, history};

const WATERMARK: bool = false; // Nice for recording media material

#[cfg(feature = "testing")]
pub type TestHookFn = Box<dyn FnOnce(&ViewerContext<'_>)>;

// TODO(#11737): Remove the serde derives since almost everything is skipped.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct AppState {
    /// Global options for the whole viewer.
    pub(crate) app_options: AppOptions,

    /// Configuration for the current recording (found in [`EntityDb`]).
    #[serde(skip)]
    pub time_controls: HashMap<StoreId, TimeControl>,
    #[serde(skip)]
    pub blueprint_time_control: TimeControl,

    /// Maps blueprint id to the current undo state for it.
    #[serde(skip)]
    pub blueprint_undo_state: HashMap<StoreId, BlueprintUndoState>,

    selection_panel: re_selection_panel::SelectionPanel,
    time_panel: re_time_panel::TimePanel,
    blueprint_time_panel: re_time_panel::TimePanel,
    #[serde(skip)]
    blueprint_tree: re_blueprint_tree::BlueprintTree,
    #[serde(skip)]
    pub(crate) recording_panel: re_recording_panel::RecordingPanel,

    #[serde(skip)]
    welcome_screen: crate::ui::WelcomeScreen,

    #[serde(skip)]
    datastore_ui: re_chunk_store_ui::DatastoreUi,

    /// Redap server catalogs and browser UI
    pub(crate) redap_servers: RedapServers,

    #[serde(skip)]
    pub(crate) open_url_modal: crate::ui::OpenUrlModal,
    #[serde(skip)]
    pub(crate) share_modal: crate::ui::ShareModal,

    /// Test-only: single-shot callback to run at the end of the frame. Used in integration tests
    /// to interact with the `ViewerContext`.
    #[cfg(feature = "testing")]
    #[serde(skip)]
    pub(crate) test_hook: Option<TestHookFn>,

    /// A stack of display modes that represents tab-like navigation of the user.
    #[serde(skip)]
    pub(crate) navigation: Navigation,

    /// A history of urls the viewer has visited.
    ///
    /// This is not updated if this is a web viewer with control over
    /// web history.
    #[serde(skip)]
    pub(crate) history: history::History,

    /// Storage for the state of each `View`
    ///
    /// This is stored here for simplicity. An exclusive reference for that is passed to the users,
    /// such as [`ViewportUi`] and [`re_selection_panel::SelectionPanel`].
    #[serde(skip)]
    view_states: ViewStates,

    /// Selection & hovering state.
    ///
    /// Not serialized since on startup we have to typically discard it anyways since
    /// whatever data was selected before is no longer accessible.
    ///
    /// For dataplatform use-cases this can even be rather irritating:
    /// if previously a server was selected, then starting with a URL should no longer select it.
    #[serde(skip)]
    pub selection_state: ApplicationSelectionState,

    /// Item that got focused on the last frame if any.
    ///
    /// The focused item is cleared every frame, but views may react with side-effects
    /// that last several frames.
    #[serde(skip)]
    pub(crate) focused_item: Option<Item>,

    /// Are we logged in?
    #[serde(skip)]
    pub(crate) auth_state: Option<AuthContext>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            app_options: Default::default(),
            time_controls: Default::default(),
            blueprint_undo_state: Default::default(),
            blueprint_time_control: Default::default(),
            selection_panel: Default::default(),
            time_panel: Default::default(),
            blueprint_time_panel: re_time_panel::TimePanel::new_blueprint_panel(),
            recording_panel: Default::default(),
            blueprint_tree: Default::default(),
            welcome_screen: Default::default(),
            datastore_ui: Default::default(),
            redap_servers: Default::default(),
            open_url_modal: Default::default(),
            share_modal: Default::default(),
            navigation: Default::default(),
            history: Default::default(),
            view_states: Default::default(),
            selection_state: Default::default(),
            focused_item: Default::default(),
            auth_state: Default::default(),

            #[cfg(feature = "testing")]
            test_hook: None,
        }
    }
}

pub(crate) struct WelcomeScreenState {
    /// The normal examples screen should be hidden. Show a fallback "no data ui" instead.
    pub hide_examples: bool,

    /// The opacity of the welcome screen during fade-in.
    pub opacity: f32,
}

impl AppState {
    pub fn set_examples_manifest_url(&mut self, egui_ctx: &egui::Context, url: String) {
        self.welcome_screen.set_examples_manifest_url(egui_ctx, url);
    }

    pub fn app_options(&self) -> &AppOptions {
        &self.app_options
    }

    pub fn app_options_mut(&mut self) -> &mut AppOptions {
        &mut self.app_options
    }

    /// Currently selected section of time, if any.
    pub fn loop_selection(
        &self,
        store_context: Option<&StoreContext<'_>>,
    ) -> Option<(TimelineName, AbsoluteTimeRangeF)> {
        let rec_id = store_context.as_ref()?.recording.store_id();
        let time_ctrl = self.time_controls.get(rec_id)?;

        // is there an active loop selection?
        time_ctrl
            .time_selection()
            .map(|q| (*time_ctrl.timeline_name(), q))
    }

    #[expect(clippy::too_many_arguments)]
    pub fn show(
        &mut self,
        app_env: &crate::AppEnvironment,
        startup_options: &mut StartupOptions,
        app_blueprint: &AppBlueprint<'_>,
        ui: &mut egui::Ui,
        render_ctx: &re_renderer::RenderContext,
        store_context: &StoreContext<'_>,
        storage_context: &StorageContext<'_>,
        reflection: &re_types_core::reflection::Reflection,
        component_ui_registry: &ComponentUiRegistry,
        component_fallback_registry: &FallbackProviderRegistry,
        view_class_registry: &ViewClassRegistry,
        rx_log: &LogReceiverSet,
        command_sender: &CommandSender,
        welcome_screen_state: &WelcomeScreenState,
        event_dispatcher: Option<&crate::event::ViewerEventDispatcher>,
        connection_registry: &ConnectionRegistryHandle,
        runtime: &AsyncRuntimeHandle,
    ) {
        re_tracing::profile_function!();

        // check state early, before the UI has a chance to close these popups
        let is_any_popup_open = egui::Popup::is_any_open(ui.ctx());

        match self.navigation.current() {
            DisplayMode::Settings(prior_mode) => {
                let mut show_settings_ui = true;
                settings_screen_ui(
                    ui,
                    &mut self.app_options,
                    startup_options,
                    &mut show_settings_ui,
                );
                if !show_settings_ui {
                    self.navigation.replace((**prior_mode).clone());
                }
            }

            DisplayMode::ChunkStoreBrowser(prior_mode) => {
                let should_datastore_ui_remain_active =
                    self.datastore_ui
                        .ui(store_context, ui, self.app_options.timestamp_format);
                if !should_datastore_ui_remain_active {
                    self.navigation.replace((**prior_mode).clone());
                }
            }

            // TODO(grtlr,ab): This needs to be further cleaned up and split into separately handled
            // display modes. See https://www.notion.so/rerunio/Major-refactor-of-re_viewer-1d8b24554b198085a02dfe441db330b4
            _ => {
                let blueprint_query = self.blueprint_query_for_viewer(store_context.blueprint);

                let Self {
                    app_options,
                    time_controls,
                    blueprint_undo_state,
                    blueprint_time_control,
                    selection_panel,
                    time_panel,
                    blueprint_time_panel,
                    blueprint_tree,
                    welcome_screen,
                    redap_servers,
                    view_states,
                    selection_state,
                    focused_item,
                    auth_state,
                    ..
                } = self;

                blueprint_undo_state
                    .entry(store_context.blueprint.store_id().clone())
                    .or_default()
                    .update(ui.ctx(), store_context.blueprint);

                let viewport_blueprint =
                    ViewportBlueprint::from_db(store_context.blueprint, &blueprint_query);
                let viewport_ui = ViewportUi::new(viewport_blueprint);

                // If the blueprint is invalid, reset it.
                if viewport_ui.blueprint.is_invalid() {
                    re_log::warn!("Incompatible blueprint detected. Resetting to default.");
                    command_sender
                        .send_system(re_viewer_context::SystemCommand::ClearActiveBlueprint);

                    // The blueprint isn't valid so nothing past this is going to work properly.
                    // we might as well return and it will get fixed on the next frame.

                    // TODO(jleibs): If we move viewport loading up to a context where the EntityDb is mutable
                    // we can run the clear and re-load.
                    return;
                }

                let selection_change = selection_state.on_frame_start(
                    |item| {
                        if let Item::StoreId(store_id) = item
                            && store_id.is_empty_recording()
                        {
                            return false;
                        }

                        viewport_ui.blueprint.is_item_valid(storage_context, item)
                    },
                    Some(Item::StoreId(store_context.recording.store_id().clone())),
                );

                if let SelectionChange::SelectionChanged(selection) = selection_change
                    && let Some(event_dispatcher) = event_dispatcher
                {
                    event_dispatcher.on_selection_change(
                        store_context.recording,
                        selection,
                        &viewport_ui.blueprint,
                    );
                }

                // The root container cannot be dragged.
                let drag_and_drop_manager =
                    DragAndDropManager::new(Item::Container(viewport_ui.blueprint.root_container));

                let recording = store_context.recording;

                let visualizable_entities_per_visualizer = view_class_registry
                    .visualizable_entities_for_visualizer_systems(recording.store_id());
                let indicated_entities_per_visualizer =
                    view_class_registry.indicated_entities_per_visualizer(recording.store_id());

                // Execute the queries for every `View`
                let query_results = {
                    re_tracing::profile_scope!("query_results");

                    use rayon::iter::{IntoParallelIterator as _, ParallelIterator as _};

                    viewport_ui
                        .blueprint
                        .views
                        .values()
                        .collect::<Vec<_>>()
                        .into_par_iter()
                        .map(|view| {
                            // Same logic as in `ViewerContext::collect_visualizable_entities_for_view_class`,
                            // but we don't have access to `ViewerContext` just yet.
                            let visualizable_entities = if let Some(view_class) =
                                view_class_registry.class_entry(view.class_identifier())
                            {
                                PerVisualizerInViewClass {
                                    view_class_identifier: view.class_identifier(),
                                    per_visualizer: visualizable_entities_per_visualizer
                                        .iter()
                                        .filter_map(|(vis, ents)| {
                                            view_class
                                                .visualizer_system_ids
                                                .contains(vis)
                                                .then_some((*vis, ents.clone()))
                                        })
                                        .collect(),
                                }
                            } else {
                                PerVisualizerInViewClass::empty(view.class_identifier())
                            };

                            (
                                view.id,
                                view.contents.build_data_result_tree(
                                    store_context,
                                    view_class_registry,
                                    &blueprint_query,
                                    &visualizable_entities,
                                ),
                            )
                        })
                        .collect::<_>()
                };

                let app_blueprint_ctx = AppBlueprintCtx {
                    command_sender,
                    current_blueprint: store_context.blueprint,
                    default_blueprint: store_context.default_blueprint,
                    blueprint_query,
                };
                let time_ctrl =
                    create_time_control_for(time_controls, recording, &app_blueprint_ctx);
                let blueprint_query = app_blueprint_ctx.blueprint_query;

                let egui_ctx = ui.ctx().clone();
                let display_mode = self.navigation.current();
                let ctx = ViewerContext {
                    global_context: GlobalContext {
                        is_test: app_env.is_test(),

                        memory_limit: startup_options.memory_limit,
                        app_options,
                        reflection,

                        egui_ctx: &egui_ctx,
                        render_ctx,
                        command_sender,

                        connection_registry,
                        display_mode,
                        auth_context: auth_state.as_ref(),
                    },
                    component_ui_registry,
                    component_fallback_registry,
                    view_class_registry,
                    connected_receivers: rx_log,
                    store_context,
                    storage_context,
                    visualizable_entities_per_visualizer: &visualizable_entities_per_visualizer,
                    indicated_entities_per_visualizer: &indicated_entities_per_visualizer,
                    query_results: &query_results,
                    time_ctrl,
                    blueprint_time_ctrl: blueprint_time_control,
                    selection_state,
                    blueprint_query: &blueprint_query,
                    focused_item,
                    drag_and_drop_manager: &drag_and_drop_manager,
                };

                // enable the heuristics if we must this frame
                if store_context.should_enable_heuristics {
                    viewport_ui.blueprint.set_auto_layout(true, &ctx);
                    viewport_ui.blueprint.set_auto_views(true, &ctx);
                    egui_ctx.request_repaint();
                }

                // Update the viewport. May spawn new views and handle queued requests (like screenshots).
                viewport_ui.on_frame_start(&ctx);

                let query_results = update_overrides(&ctx, &viewport_ui.blueprint, view_states);

                // We need to recreate the context to appease the borrow checker. It is a bit annoying, but
                // it's just a bunch of refs so not really that big of a deal in practice.
                let ctx = ViewerContext {
                    global_context: GlobalContext {
                        is_test: app_env.is_test(),

                        memory_limit: startup_options.memory_limit,
                        app_options,
                        reflection,

                        egui_ctx: &egui_ctx,
                        render_ctx,
                        command_sender,

                        connection_registry,
                        display_mode,
                        auth_context: auth_state.as_ref(),
                    },
                    component_ui_registry,
                    component_fallback_registry,
                    view_class_registry,
                    connected_receivers: rx_log,
                    store_context,
                    storage_context,
                    visualizable_entities_per_visualizer: &visualizable_entities_per_visualizer,
                    indicated_entities_per_visualizer: &indicated_entities_per_visualizer,
                    query_results: &query_results,
                    time_ctrl,
                    blueprint_time_ctrl: blueprint_time_control,
                    selection_state,
                    blueprint_query: &blueprint_query,
                    focused_item,
                    drag_and_drop_manager: &drag_and_drop_manager,
                };

                //
                // Blueprint time panel
                //

                if app_options.inspect_blueprint_timeline
                    && matches!(display_mode, DisplayMode::LocalRecordings(_))
                {
                    let blueprint_db = ctx.store_context.blueprint;

                    let undo_state = self
                        .blueprint_undo_state
                        .entry(ctx.store_context.blueprint.store_id().clone())
                        .or_default();

                    {
                        // Copy time from undo-state to the blueprint time control struct:
                        if let Some(redo_time) = undo_state.redo_time() {
                            ctx.command_sender()
                                .send_system(SystemCommand::TimeControlCommands {
                                    store_id: blueprint_db.store_id().clone(),
                                    time_commands: vec![
                                        TimeControlCommand::SetPlayState(PlayState::Paused),
                                        TimeControlCommand::SetTime(redo_time.into()),
                                    ],
                                });
                        } else {
                            ctx.command_sender()
                                .send_system(SystemCommand::TimeControlCommands {
                                    store_id: blueprint_db.store_id().clone(),
                                    time_commands: vec![TimeControlCommand::SetPlayState(
                                        PlayState::Following,
                                    )],
                                });
                        }
                    }

                    blueprint_time_panel.show_panel(
                        &ctx,
                        &viewport_ui.blueprint,
                        blueprint_db,
                        blueprint_time_control,
                        ui,
                        PanelState::Expanded,
                        // Give the blueprint time panel a distinct color from the normal time panel:
                        ui.tokens()
                            .bottom_panel_frame()
                            .fill(ui.tokens().blueprint_time_panel_bg_fill),
                    );
                }

                // TODO(grtlr): We override the app blueprint, until we have proper blueprint support for tables.
                let app_blueprint = if matches!(display_mode, DisplayMode::LocalTable(..)) {
                    &AppBlueprint::new(
                        None,
                        &LatestAtQuery::latest(blueprint_timeline()),
                        &egui_ctx,
                        None,
                    )
                } else {
                    app_blueprint
                };

                //
                // Time panel
                //

                if display_mode.has_time_panel() {
                    time_panel.show_panel(
                        &ctx,
                        &viewport_ui.blueprint,
                        ctx.recording(),
                        ctx.time_ctrl,
                        ui,
                        app_blueprint.time_panel_state(),
                        ui.tokens().bottom_panel_frame(),
                    );
                }

                //
                // Selection Panel
                //

                if display_mode.has_selection_panel() {
                    selection_panel.show_panel(
                        &ctx,
                        &viewport_ui.blueprint,
                        view_states,
                        ui,
                        app_blueprint.selection_panel_state().is_expanded(),
                    );
                }

                //
                // Left panel (recordings and blueprint)
                //

                let left_panel = egui::SidePanel::left("blueprint_panel")
                    .resizable(true)
                    .frame(egui::Frame {
                        fill: ui.visuals().panel_fill,
                        ..Default::default()
                    })
                    .min_width(120.0)
                    .default_width(default_blueprint_panel_width(
                        ui.ctx().content_rect().width(),
                    ));

                let left_panel_response = left_panel.show_animated_inside(
                    ui,
                    app_blueprint.blueprint_panel_state().is_expanded(),
                    |ui: &mut egui::Ui| {
                        // ListItem don't need vertical spacing so we disable it, but restore it
                        // before drawing the blueprint panel.
                        ui.spacing_mut().item_spacing.y = 0.0;

                        match display_mode {
                            DisplayMode::LocalRecordings(..)
                            | DisplayMode::LocalTable(..)
                            | DisplayMode::RedapEntry(..)
                            | DisplayMode::RedapServer(..)
                            | DisplayMode::Loading(..) => {
                                let show_blueprints =
                                    matches!(display_mode, DisplayMode::LocalRecordings(_));
                                let resizable = show_blueprints;
                                if resizable {
                                    // Ensure Blueprint panel has at least 150px minimum height, because now it doesn't autogrow (as it does without resizing=active)
                                    let blueprint_min_height = 150.0;
                                    let recordings_min_height = 104.0; // Minimum for recordings panel = top panel + 1 opened recording + extra space before bluprint
                                    let available_height = ui.available_height();

                                    // Calculate the maximum height for recordings panel
                                    // Allow full space usage minus the blueprint minimum height, so that the blueprint panel can grow below existing content
                                    let max_recordings_height = (available_height
                                        - blueprint_min_height)
                                        .max(recordings_min_height);

                                    egui::TopBottomPanel::top("recording_panel")
                                        .frame(egui::Frame::new())
                                        .resizable(resizable)
                                        .show_separator_line(false)
                                        .min_height(recordings_min_height)
                                        .max_height(max_recordings_height)
                                        .default_height(160.0_f32.max(recordings_min_height))
                                        .show_inside(ui, |ui| {
                                            self.recording_panel.show_panel(
                                                &ctx,
                                                ui,
                                                redap_servers,
                                                welcome_screen_state.hide_examples,
                                            );
                                        });
                                } else {
                                    self.recording_panel.show_panel(
                                        &ctx,
                                        ui,
                                        redap_servers,
                                        welcome_screen_state.hide_examples,
                                    );
                                }

                                if show_blueprints {
                                    blueprint_tree.show(
                                        &ctx,
                                        &viewport_ui.blueprint,
                                        ui,
                                        view_states,
                                    );
                                }
                            }

                            DisplayMode::ChunkStoreBrowser(_) | DisplayMode::Settings(_) => {} // handled above
                        }
                    },
                );
                if let Some(left_panel_response) = left_panel_response {
                    left_panel_response.response.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::WidgetType::Panel, true, "blueprint_panel")
                    });
                }

                //
                // Viewport
                //

                let viewport_frame = egui::Frame {
                    fill: ui.style().visuals.panel_fill,
                    ..Default::default()
                };

                egui::CentralPanel::default()
                    .frame(viewport_frame)
                    .show_inside(ui, |ui| {
                        match display_mode {
                            DisplayMode::LocalTable(table_id) => {
                                if let Some(store) = ctx.storage_context.tables.get(table_id) {
                                    table_ui(&ctx, runtime, ui, table_id, store);
                                } else {
                                    re_log::error_once!(
                                        "Could not find batch store for table id {}",
                                        table_id
                                    );
                                }
                            }

                            DisplayMode::LocalRecordings(_) => {
                                // If we are here and the "default" app id is selected,
                                // we should instead switch to the welcome screen.
                                if ctx.store_context.application_id()
                                    == &StoreHub::welcome_screen_app_id()
                                {
                                    ctx.command_sender().send_system(
                                        SystemCommand::ChangeDisplayMode(DisplayMode::RedapServer(
                                            re_redap_browser::EXAMPLES_ORIGIN.clone(),
                                        )),
                                    );
                                }
                                viewport_ui.viewport_ui(ui, &ctx, view_states);
                            }

                            DisplayMode::RedapEntry(entry) => {
                                redap_servers.entry_ui(&ctx, ui, entry.entry_id);
                            }

                            DisplayMode::RedapServer(origin) => {
                                if origin == &*re_redap_browser::EXAMPLES_ORIGIN {
                                    let origin = redap_servers
                                        .iter_servers()
                                        .find(|s| !s.origin().is_localhost())
                                        .map(|s| s.origin())
                                        .cloned();

                                    let email = auth_state.as_ref().map(|auth| auth.email.clone());
                                    let origin_token = origin
                                        .as_ref()
                                        .map(|o| redap_servers.is_authenticated(o))
                                        .unwrap_or(false);

                                    let login_state = if origin_token || email.is_some() {
                                        LoginState::Auth { email }
                                    } else {
                                        LoginState::NoAuth
                                    };

                                    let login_state = CloudState {
                                        login: login_state,
                                        has_server: origin,
                                    };
                                    welcome_screen.ui(
                                        ui,
                                        &ctx.global_context,
                                        welcome_screen_state,
                                        &rx_log.sources(),
                                        &login_state,
                                    );
                                } else {
                                    redap_servers.server_central_panel_ui(&ctx, ui, origin);
                                }
                            }

                            DisplayMode::Loading(source) => {
                                let source = if let Ok(url) =
                                    ViewerOpenUrl::from_data_source(source)
                                {
                                    Cow::Owned(ViewerOpenUrlDescription::from_url(&url).to_string())
                                } else {
                                    // In practice this shouldn't happen.
                                    Cow::Borrowed("<unknown>")
                                };
                                ui.loading_screen("Loading data source:", &*source);
                            }

                            DisplayMode::ChunkStoreBrowser(_) | DisplayMode::Settings(_) => {} // Handled above
                        }
                    });

                add_view_or_container_modal_ui(&ctx, &viewport_ui.blueprint, ui);
                drag_and_drop_manager.payload_cursor_ui(ctx.egui_ctx());

                // Process deferred layout operations and apply updates back to blueprint:
                viewport_ui.save_to_blueprint_store(&ctx);

                self.redap_servers.modals_ui(&ctx.global_context, ui);
                self.open_url_modal.ui(ui);
                self.share_modal
                    .ui(&ctx, ui, startup_options.web_viewer_base_url().as_ref());

                // Only in integration tests: call the test hook if any.
                #[cfg(feature = "testing")]
                if let Some(test_hook) = self.test_hook.take() {
                    test_hook(&ctx);
                }
            }
        }

        //
        // Other UI things
        //

        if WATERMARK {
            ui.ctx().paint_watermark();
        }

        // This must run after any ui code, or other code that tells egui to open an url:
        check_for_clicked_hyperlinks(ui.ctx(), command_sender);

        // Deselect on ESC. Must happen after all other UI code to let them capture ESC if needed.
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) && !is_any_popup_open {
            command_sender.send_system(SystemCommand::clear_selection());
        }

        // If there's no text edit or label selected, and the user triggers a copy command, copy a description of the current selection.
        if ui
            .memory(|mem| mem.focused())
            .and_then(|id| TextEditState::load(ui.ctx(), id))
            .is_none()
            && ui
                .ctx()
                .plugin::<LabelSelectionState>()
                .lock()
                .has_selection()
            && ui.input(|input| input.events.iter().any(|e| e == &egui::Event::Copy))
        {
            self.selection_state
                .selected_items()
                .copy_to_clipboard(ui.ctx());
        }

        self.selection_state.on_frame_end();

        // Reset the focused item.
        self.focused_item = None;
    }

    pub fn time_control(&self, rec_id: &StoreId) -> Option<&TimeControl> {
        self.time_controls.get(rec_id)
    }

    pub fn time_control_mut<'a>(
        &'a mut self,
        entity_db: &'a EntityDb,
        blueprint_ctx: &impl BlueprintContext,
    ) -> &'a mut TimeControl {
        create_time_control_for(&mut self.time_controls, entity_db, blueprint_ctx)
    }

    pub fn cleanup(&mut self, store_hub: &StoreHub) {
        re_tracing::profile_function!();

        self.time_controls
            .retain(|store_id, _| store_hub.store_bundle().contains(store_id));

        self.blueprint_undo_state
            .retain(|store_id, _| store_hub.store_bundle().contains(store_id));
    }

    /// Returns the blueprint query that should be used for generating the current
    /// layout of the viewer.
    ///
    /// If `inspect_blueprint_timeline` is enabled, we use the time selection from the
    /// blueprint `time_ctrl`. Otherwise, we use a latest query from the blueprint timeline.
    pub fn blueprint_query_for_viewer(&mut self, blueprint: &EntityDb) -> LatestAtQuery {
        if self.app_options.inspect_blueprint_timeline {
            if self.blueprint_time_control.play_state() == PlayState::Following {
                // Special-case just to make sure we include stuff added in this frame
                LatestAtQuery::latest(re_viewer_context::blueprint_timeline())
            } else {
                self.blueprint_time_control.current_query().clone()
            }
        } else {
            let undo_state = self
                .blueprint_undo_state
                .entry(blueprint.store_id().clone())
                .or_default();
            undo_state.blueprint_query()
        }
    }

    /// Returns the blueprint query that should be used for generating the current
    /// layout of the viewer.
    ///
    /// If `inspect_blueprint_timeline` is enabled, we use the time selection from the
    /// blueprint `time_ctrl`. Otherwise, we use a latest query from the blueprint timeline.
    pub fn get_blueprint_query_for_viewer(&self, blueprint: &EntityDb) -> Option<LatestAtQuery> {
        if self.app_options.inspect_blueprint_timeline {
            if self.blueprint_time_control.play_state() == PlayState::Following {
                // Special-case just to make sure we include stuff added in this frame
                Some(LatestAtQuery::latest(
                    re_viewer_context::blueprint_timeline(),
                ))
            } else {
                Some(self.blueprint_time_control.current_query().clone())
            }
        } else {
            self.blueprint_undo_state
                .get(blueprint.store_id())
                .map(|undo_state| undo_state.blueprint_query())
        }
    }
}

/// Updates the query results for the given viewport UI.
///
/// Returns query results derived from the previous one.
fn update_overrides(
    ctx: &ViewerContext<'_>,
    viewport_blueprint: &ViewportBlueprint,
    view_states: &mut ViewStates,
) -> HashMap<ViewId, DataQueryResult> {
    use rayon::iter::{IntoParallelIterator as _, ParallelIterator as _};

    struct OverridesUpdateTask<'a> {
        view: &'a re_viewport_blueprint::ViewBlueprint,
        view_state: &'a dyn re_viewer_context::ViewState,
        query_result: DataQueryResult,
    }

    for view in viewport_blueprint.views.values() {
        view_states.ensure_state_exists(view.id, view.class(ctx.view_class_registry));
    }

    let mut query_results = ctx.query_results.clone();

    let work_items = viewport_blueprint
        .views
        .values()
        .filter_map(|view| {
            query_results.remove(&view.id).map(|query_result| {
                let view_state = view_states
                    .get(view.id)
                    .expect("View state should exist, we just called ensure_state_exists on it.");
                OverridesUpdateTask {
                    view,
                    view_state,
                    query_result,
                }
            })
        })
        .collect::<Vec<_>>();

    work_items
        .into_par_iter()
        .map(
            |OverridesUpdateTask {
                 view,
                 view_state,
                 mut query_result,
             }| {
                let visualizable_entities =
                    ctx.collect_visualizable_entities_for_view_class(view.class_identifier());

                let query_range = view.query_range(
                    ctx.blueprint_db(),
                    ctx.blueprint_query(),
                    ctx.time_ctrl.timeline(),
                    ctx.view_class_registry,
                    view_state,
                );

                let resolver = re_viewport_blueprint::DataQueryPropertyResolver::new(
                    &query_range,
                    view.class(ctx.view_class_registry),
                    &visualizable_entities,
                    ctx.indicated_entities_per_visualizer,
                );

                resolver.update_overrides(
                    ctx.store_context.blueprint,
                    ctx.blueprint_query,
                    ctx.time_ctrl.timeline(),
                    &mut query_result,
                );

                (view.id, query_result)
            },
        )
        .collect()
}

fn table_ui(
    ctx: &ViewerContext<'_>,
    runtime: &AsyncRuntimeHandle,
    ui: &mut Ui,
    table_id: &TableId,
    store: &TableStore,
) {
    re_dataframe_ui::DataFusionTableWidget::new(store.session_context(), TableStore::TABLE_NAME)
        .title(table_id.as_str())
        .show(ctx, runtime, ui);
}

pub(crate) fn create_time_control_for<'cfgs>(
    configs: &'cfgs mut HashMap<StoreId, TimeControl>,
    entity_db: &'_ EntityDb,
    blueprint_ctx: &'_ impl BlueprintContext,
) -> &'cfgs mut TimeControl {
    fn new_time_control(
        entity_db: &'_ EntityDb,
        blueprint_ctx: &'_ impl BlueprintContext,
    ) -> TimeControl {
        let play_state = if let Some(data_source) = &entity_db.data_source {
            match data_source {
                // Play files from the start by default - it feels nice and alive.
                // We assume the `RrdHttpStream` is a done recording.
                re_log_channel::LogSource::File(_)
                | re_log_channel::LogSource::RrdHttpStream { follow: false, .. }
                | re_log_channel::LogSource::RedapGrpcStream { .. }
                | re_log_channel::LogSource::RrdWebEvent => PlayState::Playing,

                // Live data - follow it!
                re_log_channel::LogSource::RrdHttpStream { follow: true, .. }
                | re_log_channel::LogSource::Sdk
                | re_log_channel::LogSource::MessageProxy { .. }
                | re_log_channel::LogSource::Stdin
                | re_log_channel::LogSource::JsChannel { .. } => PlayState::Following,
            }
        } else {
            PlayState::Following // No known source 🤷‍♂️
        };

        let mut time_ctrl = TimeControl::from_blueprint(blueprint_ctx);

        time_ctrl.set_play_state(
            Some(entity_db.timeline_histograms()),
            play_state,
            Some(blueprint_ctx),
        );

        time_ctrl
    }

    configs
        .entry(entity_db.store_id().clone())
        .or_insert_with(|| new_time_control(entity_db, blueprint_ctx))
}

/// Handles all kind of links that can be opened within the viewer.
///
/// Must run after any ui code, or other code that tells egui to open an url.
///
/// See [`re_ui::UiExt::re_hyperlink`] for displaying hyperlinks in the UI.
fn check_for_clicked_hyperlinks(egui_ctx: &egui::Context, command_sender: &CommandSender) {
    egui_ctx.output_mut(|o| {
        o.commands.retain_mut(|command| {
            if let egui::OutputCommand::OpenUrl(open_url) = command {
                if let Ok(url) = open_url::ViewerOpenUrl::from_str(&open_url.url) {
                    url.open(
                        egui_ctx,
                        &open_url::OpenUrlOptions {
                            follow_if_http: false,
                            select_redap_source_when_loaded: !open_url.new_tab,
                            show_loader: !open_url.new_tab,
                        },
                        command_sender,
                    );

                    // We handled the URL, therefore egui shouldn't do anything anymore with it.
                    return false;
                } else {
                    // Open all links in a new tab (https://github.com/rerun-io/rerun/issues/4105)
                    open_url.new_tab = true;
                }
            }
            true
        });
    });
}

pub fn default_blueprint_panel_width(screen_width: f32) -> f32 {
    (0.35 * screen_width).min(200.0).round()
}
