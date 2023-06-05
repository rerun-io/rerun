use ahash::HashMap;

use re_data_store::StoreDb;
use re_log_types::{ApplicationId, LogMsg, StoreId, TimeRangeF};
use re_smart_channel::Receiver;
use re_viewer_context::{
    AppOptions, Caches, ComponentUiRegistry, PlayState, RecordingConfig, SpaceViewClassRegistry,
    ViewerContext,
};
use re_viewport::ViewportState;

use crate::ui::Blueprint;

const WATERMARK: bool = false; // Nice for recording media material

#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct AppState {
    /// Global options for the whole viewer.
    app_options: AppOptions,

    /// Things that need caching.
    #[serde(skip)]
    pub(crate) cache: Caches,

    #[serde(skip)]
    pub(crate) selected_rec_id: Option<StoreId>,
    #[serde(skip)]
    pub(crate) selected_blueprint_by_app: HashMap<ApplicationId, StoreId>,

    /// Configuration for the current recording (found in [`StoreDb`]).
    recording_configs: HashMap<StoreId, RecordingConfig>,

    selection_panel: crate::selection_panel::SelectionPanel,
    time_panel: re_time_panel::TimePanel,

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
    pub fn loop_selection(&self) -> Option<(re_data_store::Timeline, TimeRangeF)> {
        self.selected_rec_id.as_ref().and_then(|rec_id| {
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
        blueprint: &mut Blueprint,
        ui: &mut egui::Ui,
        render_ctx: &mut re_renderer::RenderContext,
        store_db: &StoreDb,
        re_ui: &re_ui::ReUi,
        component_ui_registry: &ComponentUiRegistry,
        space_view_class_registry: &SpaceViewClassRegistry,
        rx: &Receiver<LogMsg>,
    ) {
        re_tracing::profile_function!();

        let Self {
            app_options,
            cache,
            selected_rec_id: _,
            selected_blueprint_by_app: _,
            recording_configs,
            selection_panel,
            time_panel,
            viewport_state,
        } = self;

        let rec_cfg = recording_config_entry(
            recording_configs,
            store_db.store_id().clone(),
            rx.source(),
            store_db,
        );

        let mut ctx = ViewerContext {
            app_options,
            cache,
            space_view_class_registry,
            component_ui_registry,
            store_db,
            rec_cfg,
            re_ui,
            render_ctx,
        };

        time_panel.show_panel(&mut ctx, ui, blueprint.time_panel_expanded);
        selection_panel.show_panel(viewport_state, &mut ctx, ui, blueprint);

        let central_panel_frame = egui::Frame {
            fill: ui.style().visuals.panel_fill,
            inner_margin: egui::Margin::same(0.0),
            ..Default::default()
        };

        egui::CentralPanel::default()
            .frame(central_panel_frame)
            .show_inside(ui, |ui| {
                blueprint.blueprint_panel_and_viewport(viewport_state, &mut ctx, ui);
            });

        {
            // We move the time at the very end of the frame,
            // so we have one frame to see the first data before we move the time.
            let dt = ui.ctx().input(|i| i.stable_dt);
            let more_data_is_coming = rx.is_connected();
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

    pub fn recording_config_entry(
        &mut self,
        id: StoreId,
        data_source: &'_ re_smart_channel::SmartChannelSource,
        store_db: &'_ StoreDb,
    ) -> &mut RecordingConfig {
        recording_config_entry(&mut self.recording_configs, id, data_source, store_db)
    }

    pub fn cleanup(&mut self, store_hub: &crate::StoreHub) {
        re_tracing::profile_function!();

        if !self
            .selected_rec_id
            .as_ref()
            .map_or(false, |rec_id| store_hub.contains_recording(rec_id))
        {
            // Pick any:
            self.selected_rec_id = store_hub
                .recordings()
                .next()
                .map(|log| log.store_id().clone());
        }

        self.recording_configs
            .retain(|store_id, _| store_hub.contains_recording(store_id));
    }
}

fn recording_config_entry<'cfgs>(
    configs: &'cfgs mut HashMap<StoreId, RecordingConfig>,
    id: StoreId,
    data_source: &'_ re_smart_channel::SmartChannelSource,
    store_db: &'_ StoreDb,
) -> &'cfgs mut RecordingConfig {
    fn new_recording_confg(
        data_source: &'_ re_smart_channel::SmartChannelSource,
        store_db: &'_ StoreDb,
    ) -> RecordingConfig {
        let play_state = match data_source {
            // Play files from the start by default - it feels nice and alive./
            // RrdHttpStream downloads the whole file before decoding it, so we treat it the same as a file.
            re_smart_channel::SmartChannelSource::Files { .. }
            | re_smart_channel::SmartChannelSource::RrdHttpStream { .. }
            | re_smart_channel::SmartChannelSource::RrdWebEventListener => PlayState::Playing,

            // Live data - follow it!
            re_smart_channel::SmartChannelSource::Sdk
            | re_smart_channel::SmartChannelSource::WsClient { .. }
            | re_smart_channel::SmartChannelSource::TcpServer { .. } => PlayState::Following,
        };

        let mut rec_cfg = RecordingConfig::default();

        rec_cfg
            .time_ctrl
            .set_play_state(store_db.times_per_timeline(), play_state);

        rec_cfg
    }

    configs
        .entry(id)
        .or_insert_with(|| new_recording_confg(data_source, store_db))
}
