use ahash::HashMap;
use egui::{NumExt as _, Ui, text_edit::TextEditState, text_selection::LabelSelectionState};

use re_chunk::TimelineName;
use re_chunk_store::LatestAtQuery;
use re_entity_db::EntityDb;
use re_log_types::{LogMsg, ResolvedTimeRangeF, StoreId, TableId};
use re_redap_browser::RedapServers;
use re_smart_channel::ReceiveSet;
use re_types::blueprint::components::PanelState;
use re_ui::{ContextExt as _, UiExt as _};
use re_uri::Origin;
use re_viewer_context::{
    AppOptions, ApplicationSelectionState, AsyncRuntimeHandle, BlueprintUndoState, CommandSender,
    ComponentUiRegistry, DisplayMode, DragAndDropManager, GlobalContext, Item, PlayState,
    RecordingConfig, SelectionChange, StorageContext, StoreContext, StoreHub, SystemCommand,
    SystemCommandSender as _, TableStore, ViewClassExt as _, ViewClassRegistry, ViewStates,
    ViewerContext, blueprint_timeline,
};
use re_viewport::ViewportUi;
use re_viewport_blueprint::ViewportBlueprint;
use re_viewport_blueprint::ui::add_view_or_container_modal_ui;

use crate::{
    app_blueprint::AppBlueprint,
    event::ViewerEventDispatcher,
    navigation::Navigation,
    ui::{recordings_panel_ui, settings_screen_ui},
};

const WATERMARK: bool = false; // Nice for recording media material

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct AppState {
    /// Global options for the whole viewer.
    pub(crate) app_options: AppOptions,

    /// Configuration for the current recording (found in [`EntityDb`]).
    pub recording_configs: HashMap<StoreId, RecordingConfig>,
    pub blueprint_cfg: RecordingConfig,

    /// Maps blueprint id to the current undo state for it.
    pub blueprint_undo_state: HashMap<StoreId, BlueprintUndoState>,

    selection_panel: re_selection_panel::SelectionPanel,
    time_panel: re_time_panel::TimePanel,
    blueprint_time_panel: re_time_panel::TimePanel,
    #[serde(skip)]
    blueprint_tree: re_blueprint_tree::BlueprintTree,

    #[serde(skip)]
    welcome_screen: crate::ui::WelcomeScreen,

    #[serde(skip)]
    datastore_ui: re_chunk_store_ui::DatastoreUi,

    /// Redap server catalogs and browser UI
    pub(crate) redap_servers: RedapServers,

    /// A stack of display modes that represents tab-like navigation of the user.
    #[serde(skip)]
    pub(crate) navigation: Navigation,

    /// Storage for the state of each `View`
    ///
    /// This is stored here for simplicity. An exclusive reference for that is passed to the users,
    /// such as [`ViewportUi`] and [`re_selection_panel::SelectionPanel`].
    #[serde(skip)]
    view_states: ViewStates,

    /// Selection & hovering state.
    pub selection_state: ApplicationSelectionState,

    /// Item that got focused on the last frame if any.
    ///
    /// The focused item is cleared every frame, but views may react with side-effects
    /// that last several frames.
    #[serde(skip)]
    pub(crate) focused_item: Option<Item>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            app_options: Default::default(),
            recording_configs: Default::default(),
            blueprint_undo_state: Default::default(),
            blueprint_cfg: Default::default(),
            selection_panel: Default::default(),
            time_panel: Default::default(),
            blueprint_time_panel: re_time_panel::TimePanel::new_blueprint_panel(),
            blueprint_tree: Default::default(),
            welcome_screen: Default::default(),
            datastore_ui: Default::default(),
            redap_servers: Default::default(),
            navigation: Default::default(),
            view_states: Default::default(),
            selection_state: Default::default(),
            focused_item: Default::default(),
        }
    }
}

pub(crate) struct WelcomeScreenState {
    /// The normal welcome screen should be hidden. Show a fallback "no data ui" instead.
    pub hide: bool,

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

    pub fn add_redap_server(&self, command_sender: &CommandSender, origin: Origin) {
        if !self.redap_servers.has_server(&origin) {
            command_sender.send_system(SystemCommand::AddRedapServer(origin));
        }
    }

    pub fn select_redap_entry(&self, command_sender: &CommandSender, uri: &re_uri::EntryUri) {
        // make sure the server exists
        self.add_redap_server(command_sender, uri.origin.clone());

        command_sender.send_system(SystemCommand::SetSelection(Item::RedapEntry(uri.entry_id)));
    }

    /// Currently selected section of time, if any.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub fn loop_selection(
        &self,
        store_context: Option<&StoreContext<'_>>,
    ) -> Option<(TimelineName, ResolvedTimeRangeF)> {
        let rec_id = store_context.as_ref()?.recording.store_id();
        let rec_cfg = self.recording_configs.get(&rec_id)?;

        // is there an active loop selection?
        let time_ctrl = rec_cfg.time_ctrl.read();
        time_ctrl
            .loop_selection()
            .map(|q| (*time_ctrl.timeline().name(), q))
    }

    #[expect(clippy::too_many_arguments)]
    pub fn show(
        &mut self,
        app_blueprint: &AppBlueprint<'_>,
        ui: &mut egui::Ui,
        render_ctx: &re_renderer::RenderContext,
        store_context: &StoreContext<'_>,
        storage_context: &StorageContext<'_>,
        reflection: &re_types_core::reflection::Reflection,
        component_ui_registry: &ComponentUiRegistry,
        view_class_registry: &ViewClassRegistry,
        rx_log: &ReceiveSet<LogMsg>,
        command_sender: &CommandSender,
        welcome_screen_state: &WelcomeScreenState,
        is_history_enabled: bool,
        event_dispatcher: Option<&crate::event::ViewerEventDispatcher>,
        runtime: &AsyncRuntimeHandle,
    ) {
        re_tracing::profile_function!();

        // check state early, before the UI has a chance to close these popups
        let is_any_popup_open = ui.memory(|m| m.any_popup_open());

        match self.navigation.peek() {
            DisplayMode::Settings => {
                let mut show_settings_ui = true;
                settings_screen_ui(ui, &mut self.app_options, &mut show_settings_ui);
                if !show_settings_ui {
                    self.navigation.pop();
                }
            }

            DisplayMode::ChunkStoreBrowser => {
                let should_datastore_ui_remain_active =
                    self.datastore_ui
                        .ui(store_context, ui, self.app_options.timestamp_format);
                if !should_datastore_ui_remain_active {
                    self.navigation.pop();
                }
            }

            // TODO(grtlr,ab): This needs to be further cleaned up and split into separately handled
            // display modes. See https://www.notion.so/rerunio/Major-refactor-of-re_viewer-1d8b24554b198085a02dfe441db330b4
            _ => {
                let blueprint_query = self.blueprint_query_for_viewer(store_context.blueprint);

                let Self {
                    app_options,
                    recording_configs,
                    blueprint_undo_state,
                    blueprint_cfg,
                    selection_panel,
                    time_panel,
                    blueprint_time_panel,
                    blueprint_tree,
                    welcome_screen,
                    redap_servers,
                    navigation,
                    view_states,
                    selection_state,
                    focused_item,
                    ..
                } = self;

                blueprint_undo_state
                    .entry(store_context.blueprint.store_id().clone())
                    .or_default()
                    .update(ui.ctx(), store_context.blueprint);

                let viewport_blueprint =
                    ViewportBlueprint::try_from_db(store_context.blueprint, &blueprint_query);
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
                        if let Item::StoreId(store_id) = item {
                            if store_id.is_empty_recording() {
                                return false;
                            }
                        }

                        viewport_ui.blueprint.is_item_valid(storage_context, item)
                    },
                    Some(Item::StoreId(store_context.recording.store_id().clone())),
                );

                if let SelectionChange::SelectionChanged(selection) = selection_change {
                    if let Some(event_dispatcher) = event_dispatcher {
                        event_dispatcher.on_selection_change(
                            store_context.recording,
                            selection,
                            &viewport_ui.blueprint,
                        );
                    }
                }

                // The root container cannot be dragged.
                let drag_and_drop_manager =
                    DragAndDropManager::new(Item::Container(viewport_ui.blueprint.root_container));

                let recording = store_context.recording;

                let maybe_visualizable_entities_per_visualizer = view_class_registry
                    .maybe_visualizable_entities_for_visualizer_systems(&recording.store_id());
                let indicated_entities_per_visualizer =
                    view_class_registry.indicated_entities_per_visualizer(&recording.store_id());

                // Execute the queries for every `View`
                let mut query_results = {
                    re_tracing::profile_scope!("query_results");
                    viewport_ui
                        .blueprint
                        .views
                        .values()
                        .map(|view| {
                            // TODO(andreas): This needs to be done in a store subscriber that exists per view (instance, not class!).
                            // Note that right now we determine *all* visualizable entities, not just the queried ones.
                            // In a store subscriber set this is fine, but on a per-frame basis it's wasteful.
                            let visualizable_entities = view
                                .class(view_class_registry)
                                .determine_visualizable_entities(
                                    &maybe_visualizable_entities_per_visualizer,
                                    recording,
                                    &view_class_registry
                                        .new_visualizer_collection(view.class_identifier()),
                                    &view.space_origin,
                                );

                            (
                                view.id,
                                view.contents.execute_query(
                                    store_context,
                                    view_class_registry,
                                    &blueprint_query,
                                    &visualizable_entities,
                                ),
                            )
                        })
                        .collect::<_>()
                };

                let rec_cfg = recording_config_entry(recording_configs, recording);
                let egui_ctx = ui.ctx().clone();
                let ctx = ViewerContext {
                    global_context: GlobalContext {
                        app_options,
                        reflection,

                        egui_ctx: &egui_ctx,
                        render_ctx,
                        command_sender,
                    },
                    component_ui_registry,
                    view_class_registry,
                    store_context,
                    storage_context,
                    maybe_visualizable_entities_per_visualizer:
                        &maybe_visualizable_entities_per_visualizer,
                    indicated_entities_per_visualizer: &indicated_entities_per_visualizer,
                    query_results: &query_results,
                    rec_cfg,
                    blueprint_cfg,
                    selection_state,
                    blueprint_query: &blueprint_query,
                    focused_item,
                    drag_and_drop_manager: &drag_and_drop_manager,
                    active_table_id: match navigation.peek() {
                        DisplayMode::LocalTable(name) => Some(name),
                        _ => None,
                    },
                    active_redap_entry: match navigation.peek() {
                        DisplayMode::RedapEntry(id) => Some(id),
                        _ => None,
                    },
                };

                // enable the heuristics if we must this frame
                if store_context.should_enable_heuristics {
                    viewport_ui.blueprint.set_auto_layout(true, &ctx);
                    viewport_ui.blueprint.set_auto_views(true, &ctx);
                    egui_ctx.request_repaint();
                }

                // We move the time at the very start of the frame,
                // so that we always show the latest data when we're in "follow" mode.
                move_time(&ctx, recording, rx_log, event_dispatcher);

                // Update the viewport. May spawn new views and handle queued requests (like screenshots).
                viewport_ui.on_frame_start(&ctx);

                for view in viewport_ui.blueprint.views.values() {
                    if let Some(query_result) = query_results.get_mut(&view.id) {
                        // TODO(andreas): This needs to be done in a store subscriber that exists per view (instance, not class!).
                        // Note that right now we determine *all* visualizable entities, not just the queried ones.
                        // In a store subscriber set this is fine, but on a per-frame basis it's wasteful.
                        let visualizable_entities = view
                            .class(view_class_registry)
                            .determine_visualizable_entities(
                                &maybe_visualizable_entities_per_visualizer,
                                recording,
                                &view_class_registry
                                    .new_visualizer_collection(view.class_identifier()),
                                &view.space_origin,
                            );

                        let resolver = re_viewport_blueprint::DataQueryPropertyResolver::new(
                            view,
                            view_class_registry,
                            &maybe_visualizable_entities_per_visualizer,
                            &visualizable_entities,
                            &indicated_entities_per_visualizer,
                        );

                        resolver.update_overrides(
                            store_context.blueprint,
                            &blueprint_query,
                            rec_cfg.time_ctrl.read().timeline(),
                            view_class_registry,
                            query_result,
                            view_states,
                        );
                    }
                }

                // We need to recreate the context to appease the borrow checker. It is a bit annoying, but
                // it's just a bunch of refs so not really that big of a deal in practice.
                let ctx = ViewerContext {
                    global_context: GlobalContext {
                        app_options,
                        reflection,

                        egui_ctx: &egui_ctx,
                        render_ctx,
                        command_sender,
                    },
                    component_ui_registry,
                    view_class_registry,
                    store_context,
                    storage_context,
                    maybe_visualizable_entities_per_visualizer:
                        &maybe_visualizable_entities_per_visualizer,
                    indicated_entities_per_visualizer: &indicated_entities_per_visualizer,
                    query_results: &query_results,
                    rec_cfg,
                    blueprint_cfg,
                    selection_state,
                    blueprint_query: &blueprint_query,
                    focused_item,
                    drag_and_drop_manager: &drag_and_drop_manager,
                    active_table_id: match self.navigation.peek() {
                        DisplayMode::LocalTable(name) => Some(name),
                        _ => None,
                    },
                    active_redap_entry: match self.navigation.peek() {
                        DisplayMode::RedapEntry(id) => Some(id),
                        _ => None,
                    },
                };

                //
                // Blueprint time panel
                //

                let display_mode = self.navigation.peek();

                if app_options.inspect_blueprint_timeline
                    && *display_mode == DisplayMode::LocalRecordings
                {
                    let blueprint_db = ctx.store_context.blueprint;

                    let undo_state = self
                        .blueprint_undo_state
                        .entry(ctx.store_context.blueprint.store_id().clone())
                        .or_default();

                    {
                        // Copy time from undo-state to the blueprint time control struct:
                        let mut time_ctrl = blueprint_cfg.time_ctrl.write();
                        if let Some(redo_time) = undo_state.redo_time() {
                            time_ctrl.set_play_state(
                                blueprint_db.times_per_timeline(),
                                PlayState::Paused,
                            );
                            time_ctrl.set_time(redo_time);
                        } else {
                            time_ctrl.set_play_state(
                                blueprint_db.times_per_timeline(),
                                PlayState::Following,
                            );
                        }
                    }

                    blueprint_time_panel.show_panel(
                        &ctx,
                        &viewport_ui.blueprint,
                        blueprint_db,
                        blueprint_cfg,
                        ui,
                        PanelState::Expanded,
                        // Give the blueprint time panel a distinct color from the normal time panel:
                        ui.design_tokens()
                            .bottom_panel_frame()
                            .fill(ui.design_tokens().blueprint_time_panel_bg_fill),
                    );

                    {
                        // Apply changes to the blueprint time to the undo-state:
                        let time_ctrl = blueprint_cfg.time_ctrl.read();
                        if time_ctrl.play_state() == PlayState::Following {
                            undo_state.redo_all();
                        } else if let Some(time) = time_ctrl.time_int() {
                            undo_state.set_redo_time(time);
                        }
                    }
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

                if *display_mode == DisplayMode::LocalRecordings {
                    time_panel.show_panel(
                        &ctx,
                        &viewport_ui.blueprint,
                        ctx.recording(),
                        ctx.rec_cfg,
                        ui,
                        app_blueprint.time_panel_state(),
                        ui.design_tokens().bottom_panel_frame(),
                    );
                }

                //
                // Selection Panel
                //

                if *display_mode == DisplayMode::LocalRecordings {
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
                        ui.ctx().screen_rect().width(),
                    ));

                left_panel.show_animated_inside(
                    ui,
                    app_blueprint.blueprint_panel_state().is_expanded(),
                    |ui: &mut egui::Ui| {
                        // ListItem don't need vertical spacing so we disable it, but restore it
                        // before drawing the blueprint panel.
                        ui.spacing_mut().item_spacing.y = 0.0;

                        match display_mode {
                            DisplayMode::LocalRecordings
                            | DisplayMode::LocalTable(..)
                            | DisplayMode::RedapEntry(..)
                            | DisplayMode::RedapServer(..) => {
                                let show_blueprints = *display_mode == DisplayMode::LocalRecordings;
                                let resizable = ctx.storage_context.bundle.recordings().count() > 3
                                    && show_blueprints;

                                if resizable {
                                    // Don't shrink either recordings panel or blueprint panel below this height
                                    let min_height_each =
                                        90.0_f32.at_most(ui.available_height() / 2.0);

                                    egui::TopBottomPanel::top("recording_panel")
                                        .frame(egui::Frame::new())
                                        .resizable(resizable)
                                        .show_separator_line(false)
                                        .min_height(min_height_each)
                                        .default_height(210.0)
                                        .max_height(ui.available_height() - min_height_each)
                                        .show_inside(ui, |ui| {
                                            recordings_panel_ui(
                                                &ctx,
                                                rx_log,
                                                ui,
                                                welcome_screen_state,
                                                redap_servers,
                                            );
                                        });
                                } else {
                                    recordings_panel_ui(
                                        &ctx,
                                        rx_log,
                                        ui,
                                        welcome_screen_state,
                                        redap_servers,
                                    );
                                }

                                ui.add_space(4.0);

                                if show_blueprints {
                                    blueprint_tree.show(&ctx, &viewport_ui.blueprint, ui);
                                }
                            }

                            DisplayMode::ChunkStoreBrowser | DisplayMode::Settings => {} // handled above
                        };
                    },
                );

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

                            DisplayMode::LocalRecordings => {
                                // If we are here and the "default" app id is selected,
                                // we should instead switch to the welcome screen.
                                if ctx.store_context.app_id == StoreHub::welcome_screen_app_id() {
                                    ctx.command_sender().send_system(
                                        SystemCommand::ChangeDisplayMode(DisplayMode::RedapServer(
                                            re_redap_browser::EXAMPLES_ORIGIN.clone(),
                                        )),
                                    );
                                }
                                viewport_ui.viewport_ui(ui, &ctx, view_states);
                            }

                            DisplayMode::RedapEntry(entry) => {
                                redap_servers.entry_ui(&ctx, ui, *entry);
                            }

                            DisplayMode::RedapServer(origin) => {
                                if origin == &*re_redap_browser::EXAMPLES_ORIGIN {
                                    welcome_screen.ui(
                                        ui,
                                        command_sender,
                                        welcome_screen_state,
                                        is_history_enabled,
                                    );
                                } else {
                                    redap_servers.server_central_panel_ui(&ctx, ui, origin);
                                }
                            }

                            DisplayMode::ChunkStoreBrowser | DisplayMode::Settings => {} // Handled above
                        }
                    });

                add_view_or_container_modal_ui(&ctx, &viewport_ui.blueprint, ui);
                drag_and_drop_manager.payload_cursor_ui(ctx.egui_ctx());

                // Process deferred layout operations and apply updates back to blueprint:
                viewport_ui.save_to_blueprint_store(&ctx);
            }
        }

        //
        // Other UI things
        //

        self.redap_servers.modals_ui(ui);

        if WATERMARK {
            ui.ctx().paint_watermark();
        }

        // This must run after any ui code, or other code that tells egui to open an url:
        check_for_clicked_hyperlinks(ui.ctx(), command_sender, &self.selection_state);

        // Deselect on ESC. Must happen after all other UI code to let them capture ESC if needed.
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) && !is_any_popup_open {
            self.selection_state.clear_selection();
        }

        // If there's no text edit or label selected, and the user triggers a copy command, copy a description of the current selection.
        if ui
            .memory(|mem| mem.focused())
            .and_then(|id| TextEditState::load(ui.ctx(), id))
            .is_none()
            && !LabelSelectionState::load(ui.ctx()).has_selection()
            && ui.input(|input| input.events.iter().any(|e| e == &egui::Event::Copy))
        {
            self.selection_state
                .selected_items()
                .copy_to_clipboard(ui.ctx());
        }

        // Reset the focused item.
        self.focused_item = None;
    }

    #[cfg(target_arch = "wasm32")] // Only used in Wasm
    pub fn recording_config(&self, rec_id: &StoreId) -> Option<&RecordingConfig> {
        self.recording_configs.get(rec_id)
    }

    pub fn recording_config_mut(&mut self, entity_db: &EntityDb) -> &mut RecordingConfig {
        recording_config_entry(&mut self.recording_configs, entity_db)
    }

    pub fn cleanup(&mut self, store_hub: &StoreHub) {
        re_tracing::profile_function!();

        self.recording_configs
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
            let time_ctrl = self.blueprint_cfg.time_ctrl.read();
            if time_ctrl.play_state() == PlayState::Following {
                // Special-case just to make sure we include stuff added in this frame
                LatestAtQuery::latest(re_viewer_context::blueprint_timeline())
            } else {
                time_ctrl.current_query().clone()
            }
        } else {
            let undo_state = self
                .blueprint_undo_state
                .entry(blueprint.store_id().clone())
                .or_default();
            undo_state.blueprint_query()
        }
    }
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

fn move_time(
    ctx: &ViewerContext<'_>,
    recording: &EntityDb,
    rx: &ReceiveSet<LogMsg>,
    events: Option<&ViewerEventDispatcher>,
) {
    let dt = ctx.egui_ctx().input(|i| i.stable_dt);

    // Are we still connected to the data source for the current store?
    let more_data_is_coming = if let Some(store_source) = &recording.data_source {
        rx.sources().iter().any(|s| s.as_ref() == store_source)
    } else {
        false
    };

    let should_diff_time_ctrl = ctx.has_active_recording();
    let recording_time_ctrl_response = ctx.rec_cfg.time_ctrl.write().update(
        recording.times_per_timeline(),
        dt,
        more_data_is_coming,
        // The state diffs are used to trigger callbacks if they are configured.
        // Unless we have a real recording open, we should not actually trigger any callbacks.
        should_diff_time_ctrl,
    );

    handle_time_ctrl_event(recording, events, &recording_time_ctrl_response);

    let recording_needs_repaint = recording_time_ctrl_response.needs_repaint;

    let blueprint_needs_repaint = if ctx.app_options().inspect_blueprint_timeline {
        let should_diff_time_ctrl = false; /* we don't need state diffing here */
        ctx.blueprint_cfg
            .time_ctrl
            .write()
            .update(
                ctx.store_context.blueprint.times_per_timeline(),
                dt,
                more_data_is_coming,
                should_diff_time_ctrl,
            )
            .needs_repaint
    } else {
        re_viewer_context::NeedsRepaint::No
    };

    if recording_needs_repaint == re_viewer_context::NeedsRepaint::Yes
        || blueprint_needs_repaint == re_viewer_context::NeedsRepaint::Yes
    {
        ctx.egui_ctx().request_repaint();
    }
}

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

pub(crate) fn recording_config_entry<'cfgs>(
    configs: &'cfgs mut HashMap<StoreId, RecordingConfig>,
    entity_db: &'_ EntityDb,
) -> &'cfgs mut RecordingConfig {
    fn new_recording_config(entity_db: &'_ EntityDb) -> RecordingConfig {
        let play_state = if let Some(data_source) = &entity_db.data_source {
            match data_source {
                // Play files from the start by default - it feels nice and alive.
                // We assume the `RrdHttpStream` is a done recording.
                re_smart_channel::SmartChannelSource::File(_)
                | re_smart_channel::SmartChannelSource::RrdHttpStream { follow: false, .. }
                | re_smart_channel::SmartChannelSource::RedapGrpcStream { .. }
                | re_smart_channel::SmartChannelSource::RrdWebEventListener => PlayState::Playing,

                // Live data - follow it!
                re_smart_channel::SmartChannelSource::RrdHttpStream { follow: true, .. }
                | re_smart_channel::SmartChannelSource::Sdk
                | re_smart_channel::SmartChannelSource::MessageProxy { .. }
                | re_smart_channel::SmartChannelSource::Stdin
                | re_smart_channel::SmartChannelSource::JsChannel { .. } => PlayState::Following,
            }
        } else {
            PlayState::Following // No known source 🤷‍♂️
        };

        let mut rec_cfg = RecordingConfig::default();

        rec_cfg
            .time_ctrl
            .get_mut()
            .set_play_state(entity_db.times_per_timeline(), play_state);

        rec_cfg
    }

    configs
        .entry(entity_db.store_id().clone())
        .or_insert_with(|| new_recording_config(entity_db))
}

/// We allow linking to entities and components via hyperlinks,
/// e.g. in embedded markdown. We also have a custom `rerun://` scheme to be handled by the viewer.
///
/// Detect and handle that here.
///
/// Must run after any ui code, or other code that tells egui to open an url.
///
/// See [`re_ui::UiExt::re_hyperlink`] for displaying hyperlinks in the UI.
fn check_for_clicked_hyperlinks(
    egui_ctx: &egui::Context,
    command_sender: &CommandSender,
    selection_state: &ApplicationSelectionState,
) {
    let recording_scheme = "recording://";

    let mut recording_path = None;

    egui_ctx.output_mut(|o| {
        o.commands.retain_mut(|command| {
            if let egui::OutputCommand::OpenUrl(open_url) = command {
                if let Ok(uri) = open_url.url.parse::<re_uri::RedapUri>() {
                    command_sender.send_system(SystemCommand::LoadDataSource(
                        re_data_source::DataSource::RerunGrpcStream {
                            uri,
                            select_when_loaded: !open_url.new_tab,
                        },
                    ));

                    // NOTE: we do NOT change the display mode here.
                    // Instead we rely on `select_when_loaded` to trigger the selection… once the data is loaded.

                    return false;
                } else if let Some(path_str) = open_url.url.strip_prefix(recording_scheme) {
                    recording_path = Some(path_str.to_owned());
                    return false;
                } else {
                    // Open all links in a new tab (https://github.com/rerun-io/rerun/issues/4105)
                    open_url.new_tab = true;
                }
            }
            true
        });
    });

    if let Some(path) = recording_path {
        match path.parse::<Item>() {
            Ok(item) => {
                selection_state.set_selection(item);
            }
            Err(err) => {
                re_log::warn!("Failed to parse entity path {path:?}: {err}");
            }
        }
    }
}

pub fn default_blueprint_panel_width(screen_width: f32) -> f32 {
    (0.35 * screen_width).min(200.0).round()
}
