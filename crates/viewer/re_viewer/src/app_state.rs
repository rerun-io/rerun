use ahash::HashMap;
use egui::{text_selection::LabelSelectionState, NumExt as _, Ui};

use re_chunk::TimelineName;
use re_chunk_store::LatestAtQuery;
use re_entity_db::EntityDb;
use re_log_types::{LogMsg, ResolvedTimeRangeF, StoreId};
use re_redap_browser::RedapServers;
use re_smart_channel::ReceiveSet;
use re_types::blueprint::components::PanelState;
use re_ui::{ContextExt as _, DesignTokens};
use re_viewer_context::{
    AppOptions, ApplicationSelectionState, BlueprintUndoState, CommandSender, ComponentUiRegistry,
    DisplayMode, DragAndDropManager, GlobalContext, PlayState, RecordingConfig, SelectionChange,
    StoreContext, StoreHub, SystemCommand, SystemCommandSender as _, ViewClassExt as _,
    ViewClassRegistry, ViewStates, ViewerContext,
};
use re_viewport::ViewportUi;
use re_viewport_blueprint::ui::add_view_or_container_modal_ui;
use re_viewport_blueprint::ViewportBlueprint;

use crate::{
    app_blueprint::AppBlueprint,
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

    /// The current display mode.
    #[serde(skip)]
    pub(crate) display_mode: DisplayMode,

    /// Display the settings UI.
    ///
    /// If both `show_datastore_ui` and `show_settings_ui` are true, the settings UI takes
    /// precedence.
    #[serde(skip)]
    pub(crate) show_settings_ui: bool,

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
    pub(crate) focused_item: Option<re_viewer_context::Item>,
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
            display_mode: DisplayMode::LocalRecordings,
            show_settings_ui: false,
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
        reflection: &re_types_core::reflection::Reflection,
        component_ui_registry: &ComponentUiRegistry,
        view_class_registry: &ViewClassRegistry,
        rx: &ReceiveSet<LogMsg>,
        command_sender: &CommandSender,
        welcome_screen_state: &WelcomeScreenState,
        is_history_enabled: bool,
        callbacks: Option<&crate::callback::Callbacks>,
    ) {
        re_tracing::profile_function!();

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
            datastore_ui,
            redap_servers,
            display_mode,
            show_settings_ui,
            view_states,
            selection_state,
            focused_item,
        } = self;

        // check state early, before the UI has a chance to close these popups
        let is_any_popup_open = ui.memory(|m| m.any_popup_open());

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
            command_sender.send_system(re_viewer_context::SystemCommand::ClearActiveBlueprint);

            // The blueprint isn't valid so nothing past this is going to work properly.
            // we might as well return and it will get fixed on the next frame.

            // TODO(jleibs): If we move viewport loading up to a context where the EntityDb is mutable
            // we can run the clear and re-load.
            return;
        }

        let selection_change = selection_state.on_frame_start(
            |item| {
                if let re_viewer_context::Item::StoreId(store_id) = item {
                    if store_id.is_empty_recording() {
                        return false;
                    }
                }

                viewport_ui.blueprint.is_item_valid(store_context, item)
            },
            Some(re_viewer_context::Item::StoreId(
                store_context.recording.store_id().clone(),
            )),
        );

        if let SelectionChange::SelectionChanged(selection) = selection_change {
            if let Some(callbacks) = callbacks {
                callbacks.on_selection_change(selection, &viewport_ui.blueprint);
            }
        }

        // The root container cannot be dragged.
        let drag_and_drop_manager = DragAndDropManager::new(re_viewer_context::Item::Container(
            viewport_ui.blueprint.root_container,
        ));

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
                            &view_class_registry.new_visualizer_collection(view.class_identifier()),
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

        let rec_cfg =
            recording_config_entry(recording_configs, recording.store_id().clone(), recording);
        let egui_ctx = ui.ctx().clone();
        let ctx = ViewerContext {
            global_context: GlobalContext {
                app_options,
                reflection,
                component_ui_registry,
                view_class_registry,
                egui_ctx: &egui_ctx,
                render_ctx,
                command_sender,
            },
            store_context,
            maybe_visualizable_entities_per_visualizer: &maybe_visualizable_entities_per_visualizer,
            indicated_entities_per_visualizer: &indicated_entities_per_visualizer,
            query_results: &query_results,
            rec_cfg,
            blueprint_cfg,
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

        // We move the time at the very start of the frame,
        // so that we always show the latest data when we're in "follow" mode.
        move_time(&ctx, recording, rx, callbacks);

        // Update the viewport. May spawn new views and handle queued requests (like screenshots).
        viewport_ui.on_frame_start(&ctx);

        {
            re_tracing::profile_scope!("updated_query_results");

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
                            &view_class_registry.new_visualizer_collection(view.class_identifier()),
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
        };

        // must happen before we recreate the view context as we mutably borrow the app options
        if *show_settings_ui {
            settings_screen_ui(ui, app_options, show_settings_ui);
        }

        // We need to recreate the context to appease the borrow checker. It is a bit annoying, but
        // it's just a bunch of refs so not really that big of a deal in practice.
        let ctx = ViewerContext {
            global_context: GlobalContext {
                app_options,
                reflection,
                component_ui_registry,
                view_class_registry,
                egui_ctx: &egui_ctx,
                render_ctx,
                command_sender,
            },
            store_context,
            maybe_visualizable_entities_per_visualizer: &maybe_visualizable_entities_per_visualizer,
            indicated_entities_per_visualizer: &indicated_entities_per_visualizer,
            query_results: &query_results,
            rec_cfg,
            blueprint_cfg,
            selection_state,
            blueprint_query: &blueprint_query,
            focused_item,
            drag_and_drop_manager: &drag_and_drop_manager,
        };

        if *show_settings_ui {
            // nothing: this is already handled above
        } else if *display_mode == DisplayMode::ChunkStoreBrowser {
            let should_datastore_ui_remain_active =
                datastore_ui.ui(&ctx, ui, app_options.timestamp_format);
            if !should_datastore_ui_remain_active {
                *display_mode = DisplayMode::LocalRecordings;
            }
        } else {
            //
            // Blueprint time panel
            //

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
                        time_ctrl
                            .set_play_state(blueprint_db.times_per_timeline(), PlayState::Paused);
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
                    DesignTokens::bottom_panel_frame().fill(egui::hex_color!("#141326")),
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
                    DesignTokens::bottom_panel_frame(),
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

            //TODO(ab): this should better be handled as a specific `DisplayMode`
            let show_welcome =
                store_context.blueprint.app_id() == Some(&StoreHub::welcome_screen_app_id());

            left_panel.show_animated_inside(
                ui,
                app_blueprint.blueprint_panel_state().is_expanded(),
                |ui: &mut egui::Ui| {
                    // ListItem don't need vertical spacing so we disable it, but restore it
                    // before drawing the blueprint panel.
                    ui.spacing_mut().item_spacing.y = 0.0;

                    display_mode_toggle_ui(ui, display_mode);

                    match display_mode {
                        DisplayMode::LocalRecordings => {
                            let resizable = ctx.store_context.bundle.recordings().count() > 3;

                            if resizable {
                                // Don't shrink either recordings panel or blueprint panel below this height
                                let min_height_each = 90.0_f32.at_most(ui.available_height() / 2.0);

                                egui::TopBottomPanel::top("recording_panel")
                                    .frame(egui::Frame::new())
                                    .resizable(resizable)
                                    .show_separator_line(false)
                                    .min_height(min_height_each)
                                    .default_height(210.0)
                                    .max_height(ui.available_height() - min_height_each)
                                    .show_inside(ui, |ui| {
                                        recordings_panel_ui(&ctx, rx, ui, welcome_screen_state);
                                    });
                            } else {
                                recordings_panel_ui(&ctx, rx, ui, welcome_screen_state);
                            }

                            ui.add_space(4.0);

                            if !show_welcome {
                                blueprint_tree.show(&ctx, &viewport_ui.blueprint, ui);
                            }
                        }

                        DisplayMode::RedapBrowser => {
                            redap_servers.server_panel_ui(ui);
                        }

                        DisplayMode::ChunkStoreBrowser => {} // handled above
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
                        DisplayMode::LocalRecordings => {
                            if show_welcome {
                                welcome_screen.ui(
                                    ui,
                                    command_sender,
                                    welcome_screen_state,
                                    is_history_enabled,
                                );
                            } else {
                                viewport_ui.viewport_ui(ui, &ctx, view_states);
                            }
                        }

                        DisplayMode::RedapBrowser => {
                            redap_servers.ui(&ctx, ui);
                        }

                        DisplayMode::ChunkStoreBrowser => {} // Handled above
                    }
                });
        }

        //
        // Other UI things
        //

        add_view_or_container_modal_ui(&ctx, &viewport_ui.blueprint, ui);
        drag_and_drop_manager.payload_cursor_ui(ctx.egui_ctx());

        // Process deferred layout operations and apply updates back to blueprint:
        viewport_ui.save_to_blueprint_store(&ctx);

        if WATERMARK {
            ui.ctx().paint_watermark();
        }

        // This must run after any ui code, or other code that tells egui to open an url:
        check_for_clicked_hyperlinks(&ctx);

        // Deselect on ESC. Must happen after all other UI code to let them capture ESC if needed.
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) && !is_any_popup_open {
            selection_state.clear_selection();
        }

        // If there's no label selected, and the user triggers a copy command, copy a description of the current selection.
        if !LabelSelectionState::load(ui.ctx()).has_selection()
            && ui.input(|input| input.events.iter().any(|e| e == &egui::Event::Copy))
        {
            selection_state.selected_items().copy_to_clipboard(ui.ctx());
        }

        // Reset the focused item.
        *focused_item = None;
    }

    #[cfg(target_arch = "wasm32")] // Only used in Wasm
    pub fn recording_config(&self, rec_id: &StoreId) -> Option<&RecordingConfig> {
        self.recording_configs.get(rec_id)
    }

    pub fn recording_config_mut(&mut self, rec_id: &StoreId) -> Option<&mut RecordingConfig> {
        self.recording_configs.get_mut(rec_id)
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

fn move_time(
    ctx: &ViewerContext<'_>,
    recording: &EntityDb,
    rx: &ReceiveSet<LogMsg>,
    callbacks: Option<&crate::callback::Callbacks>,
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

    handle_time_ctrl_callbacks(callbacks, &recording_time_ctrl_response);

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

fn handle_time_ctrl_callbacks(
    callbacks: Option<&crate::callback::Callbacks>,
    response: &re_viewer_context::TimeControlResponse,
) {
    let Some(callbacks) = callbacks else {
        return;
    };

    if let Some(playing) = response.playing_change {
        if playing {
            callbacks.on_play();
        } else {
            callbacks.on_pause();
        }
    }

    if let Some((timeline, time)) = response.timeline_change {
        callbacks.on_timeline_change(timeline, time);
    }

    if let Some(time) = response.time_change {
        callbacks.on_time_update(time);
    }
}

fn display_mode_toggle_ui(ui: &mut Ui, display_mode: &mut DisplayMode) {
    ui.allocate_ui_with_layout(
        egui::vec2(
            ui.available_width(),
            re_ui::DesignTokens::title_bar_height(),
        ),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            egui::Frame::new()
                .inner_margin(re_ui::DesignTokens::panel_margin())
                .show(ui, |ui| {
                    ui.visuals_mut().widgets.hovered.expansion = 0.0;
                    ui.visuals_mut().widgets.active.expansion = 0.0;
                    ui.visuals_mut().widgets.inactive.expansion = 0.0;

                    ui.visuals_mut().selection.bg_fill = ui.visuals_mut().widgets.inactive.bg_fill;
                    ui.visuals_mut().selection.stroke = ui.visuals_mut().widgets.inactive.fg_stroke;
                    ui.visuals_mut().widgets.hovered.weak_bg_fill = egui::Color32::TRANSPARENT;

                    ui.visuals_mut().widgets.hovered.fg_stroke.color =
                        ui.visuals().widgets.inactive.fg_stroke.color;
                    ui.visuals_mut().widgets.active.fg_stroke.color =
                        ui.visuals().widgets.inactive.fg_stroke.color;
                    ui.visuals_mut().widgets.inactive.fg_stroke.color =
                        ui.visuals().widgets.noninteractive.fg_stroke.color;

                    ui.spacing_mut().button_padding = egui::vec2(6.0, 2.0);
                    ui.spacing_mut().item_spacing.x = 3.0;

                    ui.selectable_value(display_mode, DisplayMode::LocalRecordings, "Local");
                    ui.selectable_value(display_mode, DisplayMode::RedapBrowser, "Servers");
                });
        },
    );
}

pub(crate) fn recording_config_entry<'cfgs>(
    configs: &'cfgs mut HashMap<StoreId, RecordingConfig>,
    id: StoreId,
    entity_db: &'_ EntityDb,
) -> &'cfgs mut RecordingConfig {
    fn new_recording_config(entity_db: &'_ EntityDb) -> RecordingConfig {
        let play_state = if let Some(data_source) = &entity_db.data_source {
            match data_source {
                // Play files from the start by default - it feels nice and alive.
                // We assume the `RrdHttpStream` is a done recording.
                re_smart_channel::SmartChannelSource::File(_)
                | re_smart_channel::SmartChannelSource::RrdHttpStream { follow: false, .. }
                | re_smart_channel::SmartChannelSource::RedapGrpcStreamLegacy { .. }
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
        .entry(id)
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
fn check_for_clicked_hyperlinks(ctx: &ViewerContext<'_>) {
    let recording_scheme = "recording://";

    let mut recording_path = None;

    ctx.egui_ctx().output_mut(|o| {
        o.commands.retain_mut(|command| {
            if let egui::OutputCommand::OpenUrl(open_url) = command {
                let is_rerun_url = re_uri::RedapUri::try_from(open_url.url.as_ref()).is_ok();

                if is_rerun_url {
                    let data_source = re_data_source::DataSource::from_uri(
                        re_log_types::FileSource::Uri,
                        open_url.url.clone(),
                    );

                    let command_sender = ctx.command_sender().clone();
                    let on_cmd = Box::new(move |cmd| match cmd {
                        re_data_source::DataSourceCommand::SetLoopSelection {
                            recording_id,
                            timeline,
                            time_range,
                        } => command_sender.send_system(SystemCommand::SetLoopSelection {
                            rec_id: recording_id,
                            timeline,
                            time_range,
                        }),
                    });

                    match data_source.stream(on_cmd, None) {
                        Ok(re_data_source::StreamSource::LogMessages(rx)) => {
                            ctx.command_sender()
                                .send_system(SystemCommand::AddReceiver(rx));

                            if !open_url.new_tab {
                                ctx.command_sender()
                                    .send_system(SystemCommand::ChangeDisplayMode(
                                        DisplayMode::LocalRecordings,
                                    ));
                            }
                        }

                        Ok(re_data_source::StreamSource::CatalogData { endpoint }) => ctx
                            .command_sender()
                            .send_system(SystemCommand::AddRedapServer { endpoint }),
                        Err(err) => {
                            re_log::warn!("Could not handle url {:?}: {err}", open_url.url);
                        }
                    }
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
        match path.parse::<re_viewer_context::Item>() {
            Ok(item) => {
                ctx.selection_state.set_selection(item);
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
