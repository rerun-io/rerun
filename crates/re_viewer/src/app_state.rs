use ahash::HashMap;

use re_data_store::StoreDb;
use re_log_types::{LogMsg, StoreId, TimeRangeF};
use re_smart_channel::ReceiveSet;
use re_space_view::{DataQuery as _, PropertyResolver as _};
use re_viewer_context::{
    AppOptions, ApplicationSelectionState, Caches, CommandSender, ComponentUiRegistry, PlayState,
    RecordingConfig, SpaceViewClassRegistry, StoreContext, SystemCommandSender as _, ViewerContext,
};
use re_viewport::{
    determine_visualizable_entities, SpaceInfoCollection, Viewport, ViewportBlueprint,
    ViewportState,
};

use crate::ui::recordings_panel_ui;
use crate::{app_blueprint::AppBlueprint, store_hub::StoreHub, ui::blueprint_panel_ui};

const WATERMARK: bool = false; // Nice for recording media material

#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct AppState {
    /// Global options for the whole viewer.
    pub(crate) app_options: AppOptions,

    /// Things that need caching.
    #[serde(skip)]
    pub(crate) cache: Caches,

    /// Configuration for the current recording (found in [`StoreDb`]).
    recording_configs: HashMap<StoreId, RecordingConfig>,

    selection_panel: crate::selection_panel::SelectionPanel,
    time_panel: re_time_panel::TimePanel,

    #[serde(skip)]
    welcome_screen: crate::ui::WelcomeScreen,

    // TODO(jleibs): This is sort of a weird place to put this but makes more
    // sense than the blueprint
    #[serde(skip)]
    viewport_state: ViewportState,
}

impl AppState {
    pub fn set_examples_manifest_url(&mut self, url: String) {
        self.welcome_screen.set_examples_manifest_url(url);
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
    ) -> Option<(re_data_store::Timeline, TimeRangeF)> {
        store_context
            .as_ref()
            .and_then(|ctx| ctx.recording)
            .map(|rec| rec.store_id())
            .and_then(|rec_id| {
                self.recording_configs
                    .get(rec_id)
                    // is there an active loop selection?
                    .and_then(|rec_cfg| {
                        let time_ctrl = rec_cfg.time_ctrl.read();
                        time_ctrl
                            .loop_selection()
                            .map(|q| (*time_ctrl.timeline(), q))
                    })
            })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn show(
        &mut self,
        app_blueprint: &AppBlueprint<'_>,
        ui: &mut egui::Ui,
        render_ctx: &re_renderer::RenderContext,
        store_db: &StoreDb,
        store_context: &StoreContext<'_>,
        re_ui: &re_ui::ReUi,
        component_ui_registry: &ComponentUiRegistry,
        space_view_class_registry: &SpaceViewClassRegistry,
        rx: &ReceiveSet<LogMsg>,
        command_sender: &CommandSender,
    ) {
        re_tracing::profile_function!();

        let Self {
            app_options,
            cache,
            recording_configs,
            selection_panel,
            time_panel,
            welcome_screen,
            viewport_state,
        } = self;

        // Some of the mutations APIs of `ViewportBlueprints` are recorded as `Viewport::TreeAction`
        // and must be applied by `Viewport` at the end of the frame. We use a temporary channel for
        // this, which gives us interior mutability (only a shared reference of `ViewportBlueprint`
        // is available to the UI code) and, if needed in the future, concurrency.
        let (sender, receiver) = std::sync::mpsc::channel();
        let viewport_blueprint =
            ViewportBlueprint::try_from_db(store_context.blueprint, app_options, sender);
        let mut viewport = Viewport::new(
            &viewport_blueprint,
            viewport_state,
            space_view_class_registry,
            receiver,
        );

        // If the blueprint is invalid, reset it.
        if viewport.blueprint.is_invalid() {
            re_log::warn!("Incompatible blueprint detected. Resetting to default.");
            command_sender.send_system(re_viewer_context::SystemCommand::ResetBlueprint);

            // The blueprint isn't valid so nothing past this is going to work properly.
            // we might as well return and it will get fixed on the next frame.

            // TODO(jleibs): If we move viewport loading up to a context where the StoreDb is mutable
            // we can run the clear and re-load.
            return;
        }

        recording_config_entry(recording_configs, store_db.store_id().clone(), store_db)
            .selection_state
            .on_frame_start(|item| viewport.is_item_valid(item));

        let rec_cfg =
            recording_config_entry(recording_configs, store_db.store_id().clone(), store_db);

        let applicable_entities_per_visualizer = space_view_class_registry
            .applicable_entities_for_visualizer_systems(store_db.store_id());
        let indicator_matching_entities_per_visualizer = space_view_class_registry
            .indicator_matching_entities_per_visualizer(store_db.store_id());

        // Execute the queries for every `SpaceView`
        let mut query_results = {
            re_tracing::profile_scope!("query_results");
            viewport
                .blueprint
                .space_views
                .values()
                .flat_map(|space_view| {
                    // TODO(andreas): This needs to be done in a store subscriber that exists per space view (instance, not class!).
                    // Note that right now we determine *all* visualizable entities, not just the queried ones.
                    // In a store subscriber set this is fine, but on a per-frame basis it's wasteful.
                    let visualizable_entities = determine_visualizable_entities(
                        &applicable_entities_per_visualizer,
                        store_db,
                        &space_view_class_registry
                            .new_visualizer_collection(*space_view.class_identifier()),
                        space_view.class(space_view_class_registry),
                        &space_view.space_origin,
                    );

                    space_view
                        .queries
                        .iter()
                        .map(|query| {
                            (
                                query.id,
                                query.execute_query(
                                    store_context,
                                    &visualizable_entities,
                                    &indicator_matching_entities_per_visualizer,
                                ),
                            )
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<_>()
        };

        let ctx = ViewerContext {
            app_options,
            cache,
            space_view_class_registry,
            component_ui_registry,
            store_db,
            store_context,
            applicable_entities_per_visualizer: &applicable_entities_per_visualizer,
            indicator_matching_entities_per_visualizer: &indicator_matching_entities_per_visualizer,
            query_results: &query_results,
            rec_cfg,
            re_ui,
            render_ctx,
            command_sender,
        };

        // First update the viewport and thus all active space views.
        // This may update their heuristics, so that all panels that are shown in this frame,
        // have the latest information.
        let spaces_info = SpaceInfoCollection::new(ctx.store_db);

        viewport.on_frame_start(&ctx, &spaces_info);

        {
            re_tracing::profile_scope!("updated_query_results");

            for space_view in viewport.blueprint.space_views.values() {
                for query in &space_view.queries {
                    if let Some(query_result) = query_results.get_mut(&query.id) {
                        let props = viewport.state.space_view_props(space_view.id);
                        let resolver = query.build_resolver(space_view.id, props);
                        resolver.update_overrides(store_context, query_result);
                    }
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
            store_db,
            store_context,
            applicable_entities_per_visualizer: &applicable_entities_per_visualizer,
            indicator_matching_entities_per_visualizer: &indicator_matching_entities_per_visualizer,
            query_results: &query_results,
            rec_cfg,
            re_ui,
            render_ctx,
            command_sender,
        };

        time_panel.show_panel(&ctx, ui, app_blueprint.time_panel_expanded);
        selection_panel.show_panel(
            &ctx,
            ui,
            &mut viewport,
            app_blueprint.selection_panel_expanded,
        );

        let central_panel_frame = egui::Frame {
            fill: ui.style().visuals.panel_fill,
            inner_margin: egui::Margin::same(0.0),
            ..Default::default()
        };

        egui::CentralPanel::default()
            .frame(central_panel_frame)
            .show_inside(ui, |ui| {
                let left_panel = egui::SidePanel::left("blueprint_panel")
                    .resizable(true)
                    .frame(egui::Frame {
                        fill: ui.visuals().panel_fill,
                        ..Default::default()
                    })
                    .min_width(120.0)
                    .default_width((0.35 * ui.ctx().screen_rect().width()).min(200.0).round());

                left_panel.show_animated_inside(
                    ui,
                    app_blueprint.blueprint_panel_expanded,
                    |ui: &mut egui::Ui| {
                        // Set the clip rectangle to the panel for the benefit of nested, "full span" widgets like
                        // large collapsing headers. Here, no need to extend `ui.max_rect()` as the enclosing frame
                        // doesn't have inner margins.
                        ui.set_clip_rect(ui.max_rect());

                        // ListItem don't need vertical spacing so we disable it, but restore it
                        // before drawing the blueprint panel.
                        ui.spacing_mut().item_spacing.y = 0.0;

                        let recording_shown = recordings_panel_ui(&ctx, rx, ui);

                        if recording_shown {
                            ui.add_space(4.0);
                        }

                        blueprint_panel_ui(&mut viewport, &ctx, ui, &spaces_info);
                    },
                );

                let viewport_frame = egui::Frame {
                    fill: ui.style().visuals.panel_fill,
                    ..Default::default()
                };

                let show_welcome =
                    store_context.blueprint.app_id() == Some(&StoreHub::welcome_screen_app_id());

                egui::CentralPanel::default()
                    .frame(viewport_frame)
                    .show_inside(ui, |ui| {
                        if show_welcome {
                            welcome_screen.ui(ui, re_ui, rx, command_sender);
                        } else {
                            viewport.viewport_ui(ui, &ctx);
                        }
                    });
            });

        // Process deferred layout operations and apply updates back to blueprint
        viewport.update_and_sync_tile_tree_to_blueprint(&ctx);

        {
            // We move the time at the very end of the frame,
            // so we have one frame to see the first data before we move the time.
            let dt = ui.ctx().input(|i| i.stable_dt);

            // Are we still connected to the data source for the current store?
            let more_data_is_coming = if let Some(store_source) = &store_db.data_source {
                rx.sources().iter().any(|s| s.as_ref() == store_source)
            } else {
                false
            };

            let needs_repaint = ctx.rec_cfg.time_ctrl.write().update(
                store_db.times_per_timeline(),
                dt,
                more_data_is_coming,
            );
            if needs_repaint == re_viewer_context::NeedsRepaint::Yes {
                ui.ctx().request_repaint();
            }
        }

        if WATERMARK {
            re_ui.paint_watermark();
        }

        // This must run after any ui code, or other code that tells egui to open an url:
        check_for_clicked_hyperlinks(&re_ui.egui_ctx, &rec_cfg.selection_state);
    }

    pub fn recording_config_mut(&mut self, rec_id: &StoreId) -> Option<&mut RecordingConfig> {
        self.recording_configs.get_mut(rec_id)
    }

    pub fn cleanup(&mut self, store_hub: &StoreHub) {
        re_tracing::profile_function!();

        self.recording_configs
            .retain(|store_id, _| store_hub.contains_recording(store_id));
    }
}

fn recording_config_entry<'cfgs>(
    configs: &'cfgs mut HashMap<StoreId, RecordingConfig>,
    id: StoreId,
    store_db: &'_ StoreDb,
) -> &'cfgs mut RecordingConfig {
    fn new_recording_config(store_db: &'_ StoreDb) -> RecordingConfig {
        let play_state = if let Some(data_source) = &store_db.data_source {
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
                | re_smart_channel::SmartChannelSource::Stdin => PlayState::Following,
            }
        } else {
            PlayState::Following // No known source 🤷‍♂️
        };

        let mut rec_cfg = RecordingConfig::default();

        rec_cfg
            .time_ctrl
            .get_mut()
            .set_play_state(store_db.times_per_timeline(), play_state);

        rec_cfg
    }

    configs
        .entry(id)
        .or_insert_with(|| new_recording_config(store_db))
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
