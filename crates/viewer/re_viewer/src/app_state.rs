use ahash::HashMap;
use egui::NumExt as _;

use re_chunk_store::LatestAtQuery;
use re_entity_db::EntityDb;
use re_log_types::{LogMsg, ResolvedTimeRangeF, StoreId};
use re_smart_channel::ReceiveSet;
use re_types::blueprint::components::PanelState;
use re_ui::ContextExt as _;
use re_viewer_context::{
    blueprint_timeline, AppOptions, ApplicationSelectionState, CommandSender, ComponentUiRegistry,
    PlayState, RecordingConfig, SpaceViewClassExt as _, SpaceViewClassRegistry, StoreContext,
    StoreHub, SystemCommandSender as _, ViewStates, ViewerContext,
};
use re_viewport::Viewport;
use re_viewport_blueprint::ui::add_space_view_or_container_modal_ui;
use re_viewport_blueprint::ViewportBlueprint;

use crate::app_blueprint::AppBlueprint;
use crate::ui::recordings_panel_ui;

const WATERMARK: bool = false; // Nice for recording media material

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct AppState {
    /// Global options for the whole viewer.
    pub(crate) app_options: AppOptions,

    /// Configuration for the current recording (found in [`EntityDb`]).
    recording_configs: HashMap<StoreId, RecordingConfig>,
    blueprint_cfg: RecordingConfig,

    selection_panel: re_selection_panel::SelectionPanel,
    time_panel: re_time_panel::TimePanel,
    blueprint_panel: re_time_panel::TimePanel,
    #[serde(skip)]
    blueprint_tree: re_blueprint_tree::BlueprintTree,

    #[serde(skip)]
    welcome_screen: crate::ui::WelcomeScreen,

    #[serde(skip)]
    datastore_ui: re_chunk_store_ui::DatastoreUi,

    #[serde(skip)]
    pub(crate) show_datastore_ui: bool,

    /// Storage for the state of each `SpaceView`
    ///
    /// This is stored here for simplicity. An exclusive reference for that is passed to the users,
    /// such as [`Viewport`] and [`re_selection_panel::SelectionPanel`].
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
            blueprint_cfg: Default::default(),
            selection_panel: Default::default(),
            time_panel: Default::default(),
            blueprint_panel: re_time_panel::TimePanel::new_blueprint_panel(),
            blueprint_tree: Default::default(),
            welcome_screen: Default::default(),
            datastore_ui: Default::default(),
            show_datastore_ui: false,
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
    ) -> Option<(re_entity_db::Timeline, ResolvedTimeRangeF)> {
        let rec_id = store_context.as_ref()?.recording.store_id();
        let rec_cfg = self.recording_configs.get(&rec_id)?;

        // is there an active loop selection?
        let time_ctrl = rec_cfg.time_ctrl.read();
        time_ctrl
            .loop_selection()
            .map(|q| (*time_ctrl.timeline(), q))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn show(
        &mut self,
        app_blueprint: &AppBlueprint<'_>,
        ui: &mut egui::Ui,
        render_ctx: &re_renderer::RenderContext,
        recording: &EntityDb,
        store_context: &StoreContext<'_>,
        reflection: &re_types_core::reflection::Reflection,
        component_ui_registry: &ComponentUiRegistry,
        space_view_class_registry: &SpaceViewClassRegistry,
        rx: &ReceiveSet<LogMsg>,
        command_sender: &CommandSender,
        welcome_screen_state: &WelcomeScreenState,
        is_history_enabled: bool,
    ) {
        re_tracing::profile_function!();

        let blueprint_query = self.blueprint_query_for_viewer();

        let Self {
            app_options,
            recording_configs,
            blueprint_cfg,
            selection_panel,
            time_panel,
            blueprint_panel,
            blueprint_tree,
            welcome_screen,
            datastore_ui,
            show_datastore_ui,
            view_states,
            selection_state,
            focused_item,
        } = self;

        // check state early, before the UI has a chance to close these popups
        let is_any_popup_open = ui.memory(|m| m.any_popup_open());

        // Some of the mutations APIs of `ViewportBlueprints` are recorded as `Viewport::TreeAction`
        // and must be applied by `Viewport` at the end of the frame. We use a temporary channel for
        // this, which gives us interior mutability (only a shared reference of `ViewportBlueprint`
        // is available to the UI code) and, if needed in the future, concurrency.
        let (sender, receiver) = std::sync::mpsc::channel();
        let viewport_blueprint = ViewportBlueprint::try_from_db(
            store_context.blueprint,
            &blueprint_query,
            sender.clone(),
        );
        let mut viewport = Viewport::new(
            &viewport_blueprint,
            space_view_class_registry,
            receiver,
            sender,
        );

        // If the blueprint is invalid, reset it.
        if viewport.blueprint.is_invalid() {
            re_log::warn!("Incompatible blueprint detected. Resetting to default.");
            command_sender.send_system(re_viewer_context::SystemCommand::ClearActiveBlueprint);

            // The blueprint isn't valid so nothing past this is going to work properly.
            // we might as well return and it will get fixed on the next frame.

            // TODO(jleibs): If we move viewport loading up to a context where the EntityDb is mutable
            // we can run the clear and re-load.
            return;
        }

        selection_state.on_frame_start(
            |item| {
                if let re_viewer_context::Item::StoreId(store_id) = item {
                    if store_id.is_empty_recording() {
                        return false;
                    }
                }

                viewport.is_item_valid(store_context, item)
            },
            Some(re_viewer_context::Item::StoreId(
                store_context.recording.store_id().clone(),
            )),
        );

        let applicable_entities_per_visualizer = space_view_class_registry
            .applicable_entities_for_visualizer_systems(&recording.store_id());
        let indicated_entities_per_visualizer =
            space_view_class_registry.indicated_entities_per_visualizer(&recording.store_id());

        // Execute the queries for every `SpaceView`
        let mut query_results = {
            re_tracing::profile_scope!("query_results");
            viewport
                .blueprint
                .space_views
                .values()
                .map(|space_view| {
                    // TODO(andreas): This needs to be done in a store subscriber that exists per space view (instance, not class!).
                    // Note that right now we determine *all* visualizable entities, not just the queried ones.
                    // In a store subscriber set this is fine, but on a per-frame basis it's wasteful.
                    let visualizable_entities = space_view
                        .class(space_view_class_registry)
                        .determine_visualizable_entities(
                            &applicable_entities_per_visualizer,
                            recording,
                            &space_view_class_registry
                                .new_visualizer_collection(space_view.class_identifier()),
                            &space_view.space_origin,
                        );

                    (
                        space_view.id,
                        space_view
                            .contents
                            .execute_query(store_context, &visualizable_entities),
                    )
                })
                .collect::<_>()
        };

        let rec_cfg =
            recording_config_entry(recording_configs, recording.store_id().clone(), recording);
        let egui_ctx = ui.ctx().clone();
        let ctx = ViewerContext {
            app_options,
            cache: store_context.caches,
            space_view_class_registry,
            reflection,
            component_ui_registry,
            store_context,
            applicable_entities_per_visualizer: &applicable_entities_per_visualizer,
            indicated_entities_per_visualizer: &indicated_entities_per_visualizer,
            query_results: &query_results,
            rec_cfg,
            blueprint_cfg,
            selection_state,
            blueprint_query: &blueprint_query,
            egui_ctx: &egui_ctx,
            render_ctx: Some(render_ctx),
            command_sender,
            focused_item,
        };

        // We move the time at the very start of the frame,
        // so that we always show the latest data when we're in "follow" mode.
        move_time(&ctx, recording, rx);

        // Update the viewport. May spawn new views and handle queued requests (like screenshots).
        viewport.on_frame_start(&ctx);

        {
            re_tracing::profile_scope!("updated_query_results");

            for space_view in viewport.blueprint.space_views.values() {
                if let Some(query_result) = query_results.get_mut(&space_view.id) {
                    // TODO(andreas): This needs to be done in a store subscriber that exists per space view (instance, not class!).
                    // Note that right now we determine *all* visualizable entities, not just the queried ones.
                    // In a store subscriber set this is fine, but on a per-frame basis it's wasteful.
                    let visualizable_entities = space_view
                        .class(space_view_class_registry)
                        .determine_visualizable_entities(
                            &applicable_entities_per_visualizer,
                            recording,
                            &space_view_class_registry
                                .new_visualizer_collection(space_view.class_identifier()),
                            &space_view.space_origin,
                        );

                    let resolver = space_view.contents.build_resolver(
                        space_view_class_registry,
                        space_view,
                        &applicable_entities_per_visualizer,
                        &visualizable_entities,
                        &indicated_entities_per_visualizer,
                    );

                    resolver.update_overrides(
                        store_context.blueprint,
                        &blueprint_query,
                        rec_cfg.time_ctrl.read().timeline(),
                        space_view_class_registry,
                        query_result,
                        view_states,
                    );
                }
            }
        };

        // TODO(jleibs): The need to rebuild this after updating the queries is kind of annoying,
        // but it's just a bunch of refs so not really that big of a deal in practice.
        let ctx = ViewerContext {
            app_options,
            cache: store_context.caches,
            space_view_class_registry,
            reflection,
            component_ui_registry,
            store_context,
            applicable_entities_per_visualizer: &applicable_entities_per_visualizer,
            indicated_entities_per_visualizer: &indicated_entities_per_visualizer,
            query_results: &query_results,
            rec_cfg,
            blueprint_cfg,
            selection_state,
            blueprint_query: &blueprint_query,
            egui_ctx: &egui_ctx,
            render_ctx: Some(render_ctx),
            command_sender,
            focused_item,
        };

        if *show_datastore_ui {
            datastore_ui.ui(&ctx, ui, show_datastore_ui, app_options.time_zone);
        } else {
            //
            // Blueprint time panel
            //

            if app_options.inspect_blueprint_timeline {
                blueprint_panel.show_panel(
                    &ctx,
                    &viewport_blueprint,
                    ctx.store_context.blueprint,
                    blueprint_cfg,
                    ui,
                    PanelState::Expanded,
                );
            }

            //
            // Time panel
            //

            time_panel.show_panel(
                &ctx,
                &viewport_blueprint,
                ctx.recording(),
                ctx.rec_cfg,
                ui,
                app_blueprint.time_panel_state(),
            );

            //
            // Selection Panel
            //

            selection_panel.show_panel(
                &ctx,
                &viewport_blueprint,
                view_states,
                ui,
                app_blueprint.selection_panel_state().is_expanded(),
            );

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

            let show_welcome =
                store_context.blueprint.app_id() == Some(&StoreHub::welcome_screen_app_id());

            left_panel.show_animated_inside(
                ui,
                app_blueprint.blueprint_panel_state().is_expanded(),
                |ui: &mut egui::Ui| {
                    // ListItem don't need vertical spacing so we disable it, but restore it
                    // before drawing the blueprint panel.
                    ui.spacing_mut().item_spacing.y = 0.0;

                    let resizable = ctx.store_context.bundle.recordings().count() > 3;

                    if resizable {
                        // Don't shrink either recordings panel or blueprint panel below this height
                        let min_height_each = 90.0_f32.at_most(ui.available_height() / 2.0);

                        egui::TopBottomPanel::top("recording_panel")
                            .frame(egui::Frame::none())
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
                        blueprint_tree.show(&ctx, &viewport_blueprint, ui);
                    }
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
                    if show_welcome {
                        welcome_screen.ui(
                            ui,
                            command_sender,
                            welcome_screen_state,
                            is_history_enabled,
                        );
                    } else {
                        viewport.viewport_ui(ui, &ctx, view_states);
                    }
                });
        }

        //
        // Other UI things
        //

        add_space_view_or_container_modal_ui(&ctx, &viewport_blueprint, ui);

        // Process deferred layout operations and apply updates back to blueprint
        viewport.update_and_sync_tile_tree_to_blueprint(&ctx);

        if WATERMARK {
            ui.ctx().paint_watermark();
        }

        // This must run after any ui code, or other code that tells egui to open an url:
        check_for_clicked_hyperlinks(&egui_ctx, ctx.selection_state);

        // Deselect on ESC. Must happen after all other UI code to let them capture ESC if needed.
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) && !is_any_popup_open {
            selection_state.clear_selection();
        }

        // Reset the focused item.
        *focused_item = None;
    }

    pub fn recording_config_mut(&mut self, rec_id: &StoreId) -> Option<&mut RecordingConfig> {
        self.recording_configs.get_mut(rec_id)
    }

    pub fn cleanup(&mut self, store_hub: &StoreHub) {
        re_tracing::profile_function!();

        self.recording_configs
            .retain(|store_id, _| store_hub.store_bundle().contains(store_id));
    }

    /// Returns the blueprint query that should be used for generating the current
    /// layout of the viewer.
    ///
    /// If `inspect_blueprint_timeline` is enabled, we use the time selection from the
    /// blueprint `time_ctrl`. Otherwise, we use a latest query from the blueprint timeline.
    pub fn blueprint_query_for_viewer(&self) -> LatestAtQuery {
        if self.app_options.inspect_blueprint_timeline {
            self.blueprint_cfg.time_ctrl.read().current_query().clone()
        } else {
            LatestAtQuery::latest(blueprint_timeline())
        }
    }
}

fn move_time(ctx: &ViewerContext<'_>, recording: &EntityDb, rx: &ReceiveSet<LogMsg>) {
    let dt = ctx.egui_ctx.input(|i| i.stable_dt);

    // Are we still connected to the data source for the current store?
    let more_data_is_coming = if let Some(store_source) = &recording.data_source {
        rx.sources().iter().any(|s| s.as_ref() == store_source)
    } else {
        false
    };

    let recording_needs_repaint = ctx.rec_cfg.time_ctrl.write().update(
        recording.times_per_timeline(),
        dt,
        more_data_is_coming,
    );

    let blueprint_needs_repaint = if ctx.app_options.inspect_blueprint_timeline {
        ctx.blueprint_cfg.time_ctrl.write().update(
            ctx.store_context.blueprint.times_per_timeline(),
            dt,
            more_data_is_coming,
        )
    } else {
        re_viewer_context::NeedsRepaint::No
    };

    if recording_needs_repaint == re_viewer_context::NeedsRepaint::Yes
        || blueprint_needs_repaint == re_viewer_context::NeedsRepaint::Yes
    {
        ctx.egui_ctx.request_repaint();
    }
}

fn recording_config_entry<'cfgs>(
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
                | re_smart_channel::SmartChannelSource::RrdpStream { .. }
                | re_smart_channel::SmartChannelSource::RrdWebEventListener => PlayState::Playing,

                // Live data - follow it!
                re_smart_channel::SmartChannelSource::RrdHttpStream { follow: true, .. }
                | re_smart_channel::SmartChannelSource::Sdk
                | re_smart_channel::SmartChannelSource::WsClient { .. }
                | re_smart_channel::SmartChannelSource::TcpServer { .. }
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
/// e.g. in embedded markdown.
///
/// Detect and handle that here.
///
/// Must run after any ui code, or other code that tells egui to open an url.
fn check_for_clicked_hyperlinks(
    egui_ctx: &egui::Context,
    selection_state: &ApplicationSelectionState,
) {
    let recording_scheme = "recording://";

    let mut path = None;

    egui_ctx.output_mut(|o| {
        if let Some(open_url) = &o.open_url {
            if let Some(path_str) = open_url.url.strip_prefix(recording_scheme) {
                path = Some(path_str.to_owned());
                o.open_url = None;
            }
        }
    });

    if let Some(path) = path {
        match path.parse::<re_viewer_context::Item>() {
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
