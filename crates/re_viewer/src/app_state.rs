use ahash::HashMap;

use re_data_store::LatestAtQuery;
use re_entity_db::EntityDb;
use re_log_types::{LogMsg, ResolvedTimeRangeF, StoreId};
use re_smart_channel::ReceiveSet;
use re_space_view::{determine_visualizable_entities, DataQuery as _, PropertyResolver as _};
use re_viewer_context::{
    blueprint_timeline, AppOptions, ApplicationSelectionState, Caches, CommandSender,
    ComponentUiRegistry, PlayState, RecordingConfig, SpaceViewClassRegistry, StoreContext,
    StoreHub, SystemCommandSender as _, ViewerContext,
};
use re_viewport::{Viewport, ViewportBlueprint, ViewportState};

use crate::ui::recordings_panel_ui;
use crate::{app_blueprint::AppBlueprint, ui::blueprint_panel_ui};

const WATERMARK: bool = false; // Nice for recording media material

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct AppState {
    /// Global options for the whole viewer.
    pub(crate) app_options: AppOptions,

    /// Things that need caching.
    #[serde(skip)]
    pub(crate) cache: Caches,

    /// Configuration for the current recording (found in [`EntityDb`]).
    recording_configs: HashMap<StoreId, RecordingConfig>,
    blueprint_cfg: RecordingConfig,

    selection_panel: crate::selection_panel::SelectionPanel,
    time_panel: re_time_panel::TimePanel,
    blueprint_panel: re_time_panel::TimePanel,

    #[serde(skip)]
    welcome_screen: crate::ui::WelcomeScreen,

    // TODO(jleibs): This is sort of a weird place to put this but makes more
    // sense than the blueprint
    #[serde(skip)]
    viewport_state: ViewportState,

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
            cache: Default::default(),
            recording_configs: Default::default(),
            blueprint_cfg: Default::default(),
            selection_panel: Default::default(),
            time_panel: Default::default(),
            blueprint_panel: re_time_panel::TimePanel::new_blueprint_panel(),
            welcome_screen: Default::default(),
            viewport_state: Default::default(),
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
        let rec_cfg = self.recording_configs.get(rec_id)?;

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
        re_ui: &re_ui::ReUi,
        component_ui_registry: &ComponentUiRegistry,
        space_view_class_registry: &SpaceViewClassRegistry,
        rx: &ReceiveSet<LogMsg>,
        command_sender: &CommandSender,
        welcome_screen_state: &WelcomeScreenState,
    ) {
        re_tracing::profile_function!();

        let blueprint_query = self.blueprint_query_for_viewer();

        let Self {
            app_options,
            cache,
            recording_configs,
            blueprint_cfg,
            selection_panel,
            time_panel,
            blueprint_panel,
            welcome_screen,
            viewport_state,
            selection_state,
            focused_item,
        } = self;

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
            viewport_state,
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
            re_viewer_context::Item::StoreId(store_context.recording.store_id().clone()),
        );

        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            selection_state.clear_selection();
        }

        let applicable_entities_per_visualizer = space_view_class_registry
            .applicable_entities_for_visualizer_systems(recording.store_id());
        let indicated_entities_per_visualizer =
            space_view_class_registry.indicated_entities_per_visualizer(recording.store_id());

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
                    let visualizable_entities = determine_visualizable_entities(
                        &applicable_entities_per_visualizer,
                        recording,
                        &space_view_class_registry
                            .new_visualizer_collection(*space_view.class_identifier()),
                        space_view.class(space_view_class_registry),
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

        let ctx = ViewerContext {
            app_options,
            cache,
            space_view_class_registry,
            component_ui_registry,
            store_context,
            applicable_entities_per_visualizer: &applicable_entities_per_visualizer,
            indicated_entities_per_visualizer: &indicated_entities_per_visualizer,
            query_results: &query_results,
            rec_cfg,
            blueprint_cfg,
            selection_state,
            blueprint_query: &blueprint_query,
            re_ui,
            render_ctx,
            command_sender,
            focused_item,
        };

        // First update the viewport and thus all active space views.
        // This may update their heuristics, so that all panels that are shown in this frame,
        // have the latest information.
        viewport.on_frame_start(&ctx);

        {
            re_tracing::profile_scope!("updated_query_results");

            for space_view in viewport.blueprint.space_views.values() {
                if let Some(query_result) = query_results.get_mut(&space_view.id) {
                    // TODO(andreas): This needs to be done in a store subscriber that exists per space view (instance, not class!).
                    // Note that right now we determine *all* visualizable entities, not just the queried ones.
                    // In a store subscriber set this is fine, but on a per-frame basis it's wasteful.
                    let visualizable_entities = determine_visualizable_entities(
                        &applicable_entities_per_visualizer,
                        recording,
                        &space_view_class_registry
                            .new_visualizer_collection(*space_view.class_identifier()),
                        space_view.class(space_view_class_registry),
                        &space_view.space_origin,
                    );

                    let resolver = space_view.contents.build_resolver(
                        space_view_class_registry,
                        space_view,
                        &visualizable_entities,
                        &indicated_entities_per_visualizer,
                    );

                    resolver.update_overrides(
                        store_context.blueprint,
                        &blueprint_query,
                        rec_cfg.time_ctrl.read().timeline(),
                        space_view_class_registry,
                        viewport.state.legacy_auto_properties(space_view.id),
                        query_result,
                    );
                }
            }
        };

        // TODO(jleibs): The need to rebuild this after updating the queries is kind of annoying,
        // but it's just a bunch of refs so not really that big of a deal in practice.
        let ctx = ViewerContext {
            app_options,
            cache,
            space_view_class_registry,
            component_ui_registry,
            store_context,
            applicable_entities_per_visualizer: &applicable_entities_per_visualizer,
            indicated_entities_per_visualizer: &indicated_entities_per_visualizer,
            query_results: &query_results,
            rec_cfg,
            blueprint_cfg,
            selection_state,
            blueprint_query: &blueprint_query,
            re_ui,
            render_ctx,
            command_sender,
            focused_item,
        };

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
                true,
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
            app_blueprint.time_panel_expanded,
        );

        //
        // Selection Panel
        //

        selection_panel.show_panel(
            &ctx,
            ui,
            &mut viewport,
            app_blueprint.selection_panel_expanded,
        );

        //
        // Left panel (recordings and blueprint)
        //

        let mut left_panel = egui::SidePanel::left("blueprint_panel")
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

        //TODO(#6256): workaround for https://github.com/emilk/egui/issues/4475
        left_panel = left_panel
            .frame(egui::Frame::default())
            .show_separator_line(false);

        left_panel.show_animated_inside(
            ui,
            app_blueprint.blueprint_panel_expanded,
            |ui: &mut egui::Ui| {
                //TODO(#6256): workaround for https://github.com/emilk/egui/issues/4475
                let max_rect = ui.max_rect();
                ui.painter()
                    .rect_filled(max_rect, 0.0, ui.visuals().panel_fill);
                ui.painter().vline(
                    max_rect.right(),
                    max_rect.y_range(),
                    ui.visuals().widgets.noninteractive.bg_stroke,
                );
                ui.set_clip_rect(max_rect);

                re_ui::full_span::full_span_scope(ui, ui.max_rect().x_range(), |ui| {
                    // ListItem don't need vertical spacing so we disable it, but restore it
                    // before drawing the blueprint panel.
                    ui.spacing_mut().item_spacing.y = 0.0;

                    let pre_cursor = ui.cursor();
                    recordings_panel_ui(&ctx, rx, ui, welcome_screen_state);
                    let any_recording_shows = pre_cursor == ui.cursor();

                    if any_recording_shows {
                        ui.add_space(4.0);
                    }

                    if !show_welcome {
                        blueprint_panel_ui(&mut viewport, &ctx, ui);
                    }
                });
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
                    welcome_screen.ui(ui, re_ui, command_sender, welcome_screen_state);
                } else {
                    viewport.viewport_ui(ui, &ctx);
                }
            });

        // Process deferred layout operations and apply updates back to blueprint
        viewport.update_and_sync_tile_tree_to_blueprint(&ctx);

        {
            // We move the time at the very end of the frame,
            // so we have one frame to see the first data before we move the time.
            let dt = ui.ctx().input(|i| i.stable_dt);

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
                ui.ctx().request_repaint();
            }
        }

        if WATERMARK {
            re_ui.paint_watermark();
        }

        // This must run after any ui code, or other code that tells egui to open an url:
        check_for_clicked_hyperlinks(&re_ui.egui_ctx, ctx.selection_state);

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
                | re_smart_channel::SmartChannelSource::RrdHttpStream { .. }
                | re_smart_channel::SmartChannelSource::RrdWebEventListener => PlayState::Playing,

                // Live data - follow it!
                re_smart_channel::SmartChannelSource::Sdk
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

pub fn default_selection_panel_width(screen_width: f32) -> f32 {
    (0.45 * screen_width).min(300.0).round()
}
