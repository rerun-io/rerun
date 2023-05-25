use ahash::HashMap;

use re_data_store::{query_timeless_single, EntityPath};
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

        if let Some(expanded) =
            load_panel_state(&PanelState::BLUEPRINT_VIEW_PATH.into(), blueprint_db)
        {
            ret.blueprint_panel_expanded = expanded;
        }
        if let Some(expanded) =
            load_panel_state(&PanelState::SELECTION_VIEW_PATH.into(), blueprint_db)
        {
            ret.selection_panel_expanded = expanded;
        }
        if let Some(expanded) =
            load_panel_state(&PanelState::TIMELINE_VIEW_PATH.into(), blueprint_db)
        {
            ret.time_panel_expanded = expanded;
        }

        ret
    }
}

fn load_panel_state(path: &EntityPath, blueprint_db: &re_data_store::LogDb) -> Option<bool> {
    query_timeless_single::<PanelState>(&blueprint_db.entity_db.data_store, path)
        .map(|p| p.expanded)
}

fn load_space_view(path: &EntityPath, blueprint_db: &re_data_store::LogDb) -> Option<SpaceView> {
    query_timeless_single::<SpaceViewComponent>(&blueprint_db.entity_db.data_store, path)
        .map(|c| c.space_view)
}

fn load_viewport(
    blueprint_db: &re_data_store::LogDb,
    space_views: HashMap<SpaceViewId, SpaceView>,
) -> Viewport {
    let auto_space_views = query_timeless_single::<AutoSpaceViews>(
        &blueprint_db.entity_db.data_store,
        &VIEWPORT_PATH.into(),
    )
    .unwrap_or_else(|| {
        // Only enable auto-space-views if this is the app-default blueprint
        AutoSpaceViews(
            blueprint_db
                .recording_info()
                .map_or(false, |ri| ri.is_app_default_blueprint()),
        )
    });

    let space_view_visibility = query_timeless_single::<SpaceViewVisibility>(
        &blueprint_db.entity_db.data_store,
        &VIEWPORT_PATH.into(),
    )
    .unwrap_or_default();

    let space_view_maximized = query_timeless_single::<SpaceViewMaximized>(
        &blueprint_db.entity_db.data_store,
        &VIEWPORT_PATH.into(),
    )
    .unwrap_or_default();

    let viewport_layout: ViewportLayout = query_timeless_single::<ViewportLayout>(
        &blueprint_db.entity_db.data_store,
        &VIEWPORT_PATH.into(),
    )
    .unwrap_or_default();

    let unknown_space_views: HashMap<_, _> = space_views
        .iter()
        .filter(|(k, _)| !viewport_layout.space_view_keys.contains(k))
        .map(|(k, v)| (*k, v.clone()))
        .collect();

    let known_space_views: HashMap<_, _> = space_views
        .into_iter()
        .filter(|(k, _)| viewport_layout.space_view_keys.contains(k))
        .collect();

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
    // into the snapshot. For example, even if there's no visibility in the
    // store, this will end up with default-visibility, which then *won't* be saved back.
    for (_, view) in unknown_space_views {
        viewport.add_space_view(view);
    }

    viewport
}
