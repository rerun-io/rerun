use ahash::HashMap;
use re_arrow_store::{TimeInt, Timeline};
use re_data_store::EntityPath;
use re_log_types::Component;
use re_viewer_context::SpaceViewId;

use crate::blueprint_components::{PanelState, SpaceViewComponent, ViewportComponent};

use super::{Blueprint, SpaceView, Viewport};

impl Blueprint {
    pub fn from_db(egui_ctx: &egui::Context, blueprint_db: &re_data_store::LogDb) -> Self {
        let mut ret = Self::new(egui_ctx);

        let space_views: HashMap<SpaceViewId, SpaceView> = if let Some(space_views) = blueprint_db
            .entity_db
            .tree
            .children
            .get(&re_data_store::EntityPathPart::Name(
                SpaceViewComponent::SPACEVIEW_PREFIX.into(),
            )) {
            space_views
                .children
                .values()
                .filter_map(|view_tree| load_space_view(&view_tree.path, blueprint_db))
                .map(|sv| (sv.id, sv))
                .collect()
        } else {
            Default::default()
        };

        ret.viewport = load_viewport(blueprint_db, space_views);

        // TODO(jleibs): maybe just combine these into a single component
        // TODO(jleibs): Also, don't use them if they aren't set instead of defaulting to true
        //               so that we get the right default state on new()
        ret.blueprint_panel_expanded =
            load_selection_state(&PanelState::BLUEPRINT_PANEL.into(), blueprint_db);
        ret.selection_panel_expanded =
            load_selection_state(&PanelState::SELECTION_PANEL.into(), blueprint_db);
        ret.time_panel_expanded =
            load_selection_state(&PanelState::TIMELINE_PANEL.into(), blueprint_db);

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

fn load_viewport(
    blueprint_db: &re_data_store::LogDb,
    space_views: HashMap<SpaceViewId, SpaceView>,
) -> Viewport {
    // TODO(jleibs): This is going to need to be a LOT more ergonomic
    let query = re_arrow_store::LatestAtQuery::new(Timeline::default(), TimeInt::MAX);

    let blueprint_state = blueprint_db.entity_db.data_store.latest_at(
        &query,
        &ViewportComponent::VIEWPORT.into(),
        ViewportComponent::name(),
        &[ViewportComponent::name()],
    );

    let viewport_component = blueprint_state
        .and_then(|(_, data)| {
            data[0]
                .as_ref()
                .and_then(|cell| cell.try_to_native::<ViewportComponent>().unwrap().next())
        })
        .unwrap_or_default();

    let mut viewport = Viewport {
        // TODO(jleibs): avoid this clone
        space_views: space_views.clone(),
        visible: viewport_component.visible,
        trees: viewport_component.trees,
        maximized: viewport_component.maximized,
        has_been_user_edited: viewport_component.has_been_user_edited,
    };

    for (id, view) in space_views {
        if !viewport_component.space_view_keys.contains(&id) {
            viewport.add_space_view(view);
        }
    }

    viewport
}
