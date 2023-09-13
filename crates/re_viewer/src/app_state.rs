use ahash::HashMap;

use re_data_store::StoreDb;
use re_log_types::{LogMsg, StoreId, TimeRangeF};
use re_smart_channel::ReceiveSet;
use re_viewer_context::{
    AppOptions, Caches, CommandSender, ComponentUiRegistry, PlayState, RecordingConfig,
    SpaceViewClassRegistry, StoreContext, ViewerContext,
};
use re_viewport::{SpaceInfoCollection, Viewport, ViewportState};

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
                        rec_cfg
                            .time_ctrl
                            .loop_selection()
                            .map(|q| (*rec_cfg.time_ctrl.timeline(), q))
                    })
            })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn show(
        &mut self,
        app_blueprint: &AppBlueprint<'_>,
        ui: &mut egui::Ui,
        render_ctx: &mut re_renderer::RenderContext,
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

        let mut viewport = Viewport::from_db(store_context.blueprint, viewport_state);

        recording_config_entry(recording_configs, store_db.store_id().clone(), store_db)
            .selection_state
            .on_frame_start(|item| viewport.is_item_valid(item));

        let rec_cfg =
            recording_config_entry(recording_configs, store_db.store_id().clone(), store_db);

        let mut ctx = ViewerContext {
            app_options,
            cache,
            space_view_class_registry,
            component_ui_registry,
            store_db,
            store_context,
            rec_cfg,
            re_ui,
            render_ctx,
            command_sender,
        };

        // First update the viewport and thus all active space views.
        // This may update their heuristics, so that all panels that are shown in this frame,
        // have the latest information.
        let spaces_info = SpaceInfoCollection::new(&ctx.store_db.entity_db);

        // If the blueprint is invalid, reset it.
        if viewport.blueprint.is_invalid() {
            re_log::warn!("Incompatible blueprint detected. Resetting to default.");
            viewport.blueprint.reset(&mut ctx, &spaces_info);
        }

        viewport.on_frame_start(&mut ctx, &spaces_info);

        time_panel.show_panel(&mut ctx, ui, app_blueprint.time_panel_expanded);
        selection_panel.show_panel(
            &mut ctx,
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

                        let recording_shown = recordings_panel_ui(&mut ctx, rx, ui);

                        if recording_shown {
                            ui.add_space(4.0);
                        }

                        blueprint_panel_ui(&mut viewport.blueprint, &mut ctx, ui, &spaces_info);
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
                            welcome_screen.ui(re_ui, ui, rx, command_sender);
                        } else {
                            viewport.viewport_ui(ui, &mut ctx);
                        }
                    });

                // If the viewport was user-edited, then disable auto space views
                if viewport.blueprint.has_been_user_edited {
                    viewport.blueprint.auto_space_views = false;
                }
            });

        viewport.sync_blueprint_changes(command_sender);

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

            let needs_repaint = ctx.rec_cfg.time_ctrl.update(
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
                | re_smart_channel::SmartChannelSource::TcpServer { .. } => PlayState::Following,
            }
        } else {
            PlayState::Following // No known source 🤷‍♂️
        };

        let mut rec_cfg = RecordingConfig::default();

        rec_cfg
            .time_ctrl
            .set_play_state(store_db.times_per_timeline(), play_state);

        rec_cfg
    }

    configs
        .entry(id)
        .or_insert_with(|| new_recording_config(store_db))
}
