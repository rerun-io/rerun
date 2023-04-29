use re_arrow_store::{TimeInt, Timeline};
use re_data_store::EntityPath;
use re_log_types::Component;

use crate::blueprint_components::{PanelState, SpaceViewComponent};

use super::{Blueprint, SpaceView};

impl Blueprint {
    pub fn from_db(egui_ctx: &egui::Context, blueprint_db: &re_data_store::LogDb) -> Self {
        let mut ret = Self::new(egui_ctx);

        // TODO(jleibs): this needs to be part of the blueprint
        ret.viewport.mark_user_interaction();

        // TODO(jleibs): maybe just combine these into a single component
        ret.blueprint_panel_expanded =
            load_selection_state(&PanelState::BLUEPRINT_PANEL.into(), blueprint_db);
        ret.selection_panel_expanded =
            load_selection_state(&PanelState::SELECTION_PANEL.into(), blueprint_db);
        ret.time_panel_expanded =
            load_selection_state(&PanelState::TIMELINE_PANEL.into(), blueprint_db);

        if let Some(space_views) = blueprint_db
            .entity_db
            .tree
            .children
            .get(&re_data_store::EntityPathPart::Name("space_view".into()))
        {
            for tree in space_views.children.values() {
                if let Some(space_view) = load_space_view(&tree.path, blueprint_db) {
                    ret.viewport.add_space_view(space_view);
                }
            }
        }

        ret
    }
}

fn load_selection_state(path: &EntityPath, blueprint_db: &re_data_store::LogDb) -> bool {
    // TODO(jleibs): This is going to need to be a LOT more ergonomic
    let query = re_arrow_store::LatestAtQuery::new(Timeline::default(), TimeInt::MAX);

    let blueprint_state = blueprint_db.entity_db.data_store.latest_at(
        &query,
        path,
        PanelState::name(),
        &[PanelState::name()],
    );
    blueprint_state.map_or(true, |(_, data)| {
        data[0].as_ref().map_or(true, |cell| {
            cell.try_to_native::<PanelState>()
                .unwrap()
                .next()
                .unwrap()
                .expanded
        })
    })
}

fn load_space_view(path: &EntityPath, blueprint_db: &re_data_store::LogDb) -> Option<SpaceView> {
    // TODO(jleibs): This is going to need to be a LOT more ergonomic
    let query = re_arrow_store::LatestAtQuery::new(Timeline::default(), TimeInt::MAX);

    let blueprint_state = blueprint_db.entity_db.data_store.latest_at(
        &query,
        path,
        SpaceViewComponent::name(),
        &[SpaceViewComponent::name()],
    );
    blueprint_state.and_then(|(_, data)| {
        data[0].as_ref().and_then(|cell| {
            cell.try_to_native::<SpaceViewComponent>()
                .unwrap()
                .next()
                .map(|c| c.space_view)
        })
    })
}
