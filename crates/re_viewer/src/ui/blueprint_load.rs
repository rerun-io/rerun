use ahash::HashMap;
use re_arrow_store::{TimeInt, Timeline};
use re_data_store::{query_latest_single, EntityPath};
use re_viewer_context::SpaceViewId;

use crate::blueprint_components::{
    panel::PanelState,
    space_view::SpaceViewComponent,
    viewport::{
        AutoSpaceViews, SpaceViewMaximized, SpaceViewVisibility, ViewportLayout, VIEWPORT_PATH,
    },
};

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

        if let Some(expanded) = load_panel_state(&PanelState::BLUEPRINT_PANEL.into(), blueprint_db)
        {
            ret.blueprint_panel_expanded = expanded;
        }
        if let Some(expanded) = load_panel_state(&PanelState::SELECTION_PANEL.into(), blueprint_db)
        {
            ret.selection_panel_expanded = expanded;
        }
        if let Some(expanded) = load_panel_state(&PanelState::TIMELINE_PANEL.into(), blueprint_db) {
            ret.time_panel_expanded = expanded;
        }

        ret
    }
}

fn load_panel_state(path: &EntityPath, blueprint_db: &re_data_store::LogDb) -> Option<bool> {
    let query = re_arrow_store::LatestAtQuery::new(Timeline::default(), TimeInt::MAX);

    query_latest_single::<PanelState>(&blueprint_db.entity_db, path, &query).map(|p| p.expanded)
}

fn load_space_view(path: &EntityPath, blueprint_db: &re_data_store::LogDb) -> Option<SpaceView> {
    let query = re_arrow_store::LatestAtQuery::new(Timeline::default(), TimeInt::MAX);

    query_latest_single::<SpaceViewComponent>(&blueprint_db.entity_db, path, &query)
        .map(|c| c.space_view)
}

fn load_viewport(
    blueprint_db: &re_data_store::LogDb,
    space_views: HashMap<SpaceViewId, SpaceView>,
) -> Viewport {
    let query = re_arrow_store::LatestAtQuery::new(Timeline::default(), TimeInt::MAX);

    let auto_space_views = query_latest_single::<AutoSpaceViews>(
        &blueprint_db.entity_db,
        &VIEWPORT_PATH.into(),
        &query,
    )
    .unwrap_or_default();

    let space_view_visibility = query_latest_single::<SpaceViewVisibility>(
        &blueprint_db.entity_db,
        &VIEWPORT_PATH.into(),
        &query,
    )
    .unwrap_or_default();

    let space_view_maximized = query_latest_single::<SpaceViewMaximized>(
        &blueprint_db.entity_db,
        &VIEWPORT_PATH.into(),
        &query,
    )
    .unwrap_or_default();

    let viewport_layout: ViewportLayout = query_latest_single::<ViewportLayout>(
        &blueprint_db.entity_db,
        &VIEWPORT_PATH.into(),
        &query,
    )
    .unwrap_or_default();

    // TODO(jleibs): Can this be done as a partition operation without the clone?
    let mut known_space_views = space_views.clone();
    known_space_views.retain(|k, _| viewport_layout.space_view_keys.contains(k));
    let mut unknown_space_views = space_views;
    unknown_space_views.retain(|k, _| !viewport_layout.space_view_keys.contains(k));

    let mut viewport = Viewport {
        space_views: known_space_views,
        visible: space_view_visibility.0,
        trees: viewport_layout.trees,
        maximized: space_view_maximized.0,
        has_been_user_edited: viewport_layout.has_been_user_edited,
        auto_space_views: auto_space_views.0,
    };
    // TODO(jleibs): It seems we shouldn't call this until later, after we've created
    // the snapshot. Doing this here means we are mutating the state before it goes
    // into the snapshot. For example, for example, even there's no visibility in the
    // store, this will end up with default-visibility, which then *won't* be saved back.
    for (_, view) in unknown_space_views {
        viewport.add_space_view(view);
    }

    viewport
}
