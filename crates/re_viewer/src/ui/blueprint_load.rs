use std::collections::BTreeMap;

use ahash::HashMap;

use re_data_store::{query_timeless_single, EntityPath};
use re_viewer_context::SpaceViewId;
use re_viewport::{
    blueprint_components::{
        AutoSpaceViews, SpaceViewComponent, SpaceViewMaximized, SpaceViewVisibility,
        ViewportLayout, VIEWPORT_PATH,
    },
    SpaceViewBlueprint, Viewport,
};

use super::Blueprint;

impl<'a> Blueprint<'a> {
    pub fn from_db(blueprint_db: Option<&'a re_data_store::StoreDb>) -> Self {
        let mut ret = Self::new(blueprint_db);

        if let Some(blueprint_db) = blueprint_db {
            let space_views: HashMap<SpaceViewId, SpaceViewBlueprint> = if let Some(space_views) =
                blueprint_db
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
        }
        ret
    }
}

fn load_space_view(
    path: &EntityPath,
    blueprint_db: &re_data_store::StoreDb,
) -> Option<SpaceViewBlueprint> {
    query_timeless_single::<SpaceViewComponent>(&blueprint_db.entity_db.data_store, path)
        .map(|c| c.space_view)
}

fn load_viewport(
    blueprint_db: &re_data_store::StoreDb,
    space_views: HashMap<SpaceViewId, SpaceViewBlueprint>,
) -> Viewport {
    let auto_space_views = query_timeless_single::<AutoSpaceViews>(
        &blueprint_db.entity_db.data_store,
        &VIEWPORT_PATH.into(),
    )
    .unwrap_or_else(|| {
        // Only enable auto-space-views if this is the app-default blueprint
        AutoSpaceViews(
            blueprint_db
                .store_info()
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

    let known_space_views: BTreeMap<_, _> = space_views
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
