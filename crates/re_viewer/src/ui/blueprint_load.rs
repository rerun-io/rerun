use re_arrow_store::{TimeInt, Timeline};
use re_log_types::Component;

use crate::blueprint_components::PanelState;

use super::Blueprint;

impl Blueprint {
    pub fn from_db(egui_ctx: &egui::Context, blueprint_db: &re_data_store::LogDb) -> Self {
        let mut ret = Self::new(egui_ctx);

        // TODO(jleibs): This is going to need to be a LOT more ergonomic
        let query = re_arrow_store::LatestAtQuery::new(Timeline::default(), TimeInt::MAX);

        let blueprint_state = blueprint_db.entity_db.data_store.latest_at(
            &query,
            &PanelState::BLUEPRINT_PANEL.into(),
            PanelState::name(),
            &[PanelState::name()],
        );
        ret.blueprint_panel_expanded = blueprint_state.map_or(true, |(_, data)| {
            data[0].as_ref().map_or(true, |cell| {
                cell.try_to_native::<PanelState>()
                    .unwrap()
                    .next()
                    .unwrap()
                    .expanded
            })
        });

        let selection_state = blueprint_db.entity_db.data_store.latest_at(
            &query,
            &PanelState::SELECTION_PANEL.into(),
            PanelState::name(),
            &[PanelState::name()],
        );
        ret.selection_panel_expanded = selection_state.map_or(true, |(_, data)| {
            data[0].as_ref().map_or(true, |cell| {
                cell.try_to_native::<PanelState>()
                    .unwrap()
                    .next()
                    .unwrap()
                    .expanded
            })
        });

        let timeline_state = blueprint_db.entity_db.data_store.latest_at(
            &query,
            &PanelState::TIMELINE_PANEL.into(),
            PanelState::name(),
            &[PanelState::name()],
        );
        ret.time_panel_expanded = timeline_state.map_or(true, |(_, data)| {
            data[0].as_ref().map_or(true, |cell| {
                cell.try_to_native::<PanelState>()
                    .unwrap()
                    .next()
                    .unwrap()
                    .expanded
            })
        });
        ret
    }
}
