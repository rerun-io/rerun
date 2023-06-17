use std::collections::BTreeMap;

use ahash::HashMap;
use arrow2_convert::field::ArrowField;
use re_data_store::StoreDb;
use re_log_types::{
    Component, DataCell, DataRow, EntityPath, RowId, SerializableComponent, TimePoint,
};
use re_viewer_context::{CommandSender, Item, SpaceViewId, SystemCommand, SystemCommandSender};

use crate::{
    blueprint_components::{
        AutoSpaceViews, SpaceViewComponent, SpaceViewMaximized, SpaceViewVisibility,
        ViewportLayout, VIEWPORT_PATH,
    },
    SpaceViewBlueprint, Viewport,
};

// ----------------------------------------------------------------------------

/// Defines the layout of the Viewport
#[derive(Clone)]
pub struct ViewportBlueprint<'a> {
    pub blueprint_db: &'a StoreDb,

    pub viewport: Viewport,
    snapshot: Viewport,
}

impl<'a> ViewportBlueprint<'a> {
    pub fn from_db(blueprint_db: &'a re_data_store::StoreDb) -> Self {
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

        let viewport = load_viewport(blueprint_db, space_views);

        Self {
            blueprint_db,
            viewport: viewport.clone(),
            snapshot: viewport,
        }
    }

    /// If `false`, the item is referring to data that is not present in this blueprint.
    pub fn is_item_valid(&self, item: &Item) -> bool {
        match item {
            Item::ComponentPath(_) => true,
            Item::InstancePath(space_view_id, _) => space_view_id
                .map(|space_view_id| self.viewport.space_view(&space_view_id).is_some())
                .unwrap_or(true),
            Item::SpaceView(space_view_id) => self.viewport.space_view(space_view_id).is_some(),
            Item::DataBlueprintGroup(space_view_id, data_blueprint_group_handle) => {
                if let Some(space_view) = self.viewport.space_view(space_view_id) {
                    space_view
                        .data_blueprint
                        .group(*data_blueprint_group_handle)
                        .is_some()
                } else {
                    false
                }
            }
        }
    }

    pub fn sync_changes(&self, command_sender: &CommandSender) {
        let mut deltas = vec![];

        sync_viewport(&mut deltas, &self.viewport, &self.snapshot);

        // Add any new or modified space views
        for id in self.viewport.space_view_ids() {
            if let Some(space_view) = self.viewport.space_view(id) {
                sync_space_view(&mut deltas, space_view, self.snapshot.space_view(id));
            }
        }

        // Remove any deleted space views
        for space_view_id in self.snapshot.space_view_ids() {
            if self.viewport.space_view(space_view_id).is_none() {
                clear_space_view(&mut deltas, space_view_id);
            }
        }

        command_sender.send_system(SystemCommand::UpdateBlueprint(
            self.blueprint_db.store_id().clone(),
            deltas,
        ));
    }
}

// ----------------------------------------------------------------------------

// TODO(jleibs): Move this helper to a better location
fn add_delta_from_single_component<C: SerializableComponent>(
    deltas: &mut Vec<DataRow>,
    entity_path: &EntityPath,
    timepoint: &TimePoint,
    component: C,
) {
    let row = DataRow::from_cells1_sized(
        RowId::random(),
        entity_path.clone(),
        timepoint.clone(),
        1,
        [component].as_slice(),
    );

    deltas.push(row);
}

// ----------------------------------------------------------------------------

fn load_space_view(
    path: &EntityPath,
    blueprint_db: &re_data_store::StoreDb,
) -> Option<SpaceViewBlueprint> {
    blueprint_db
        .store()
        .query_timeless_component::<SpaceViewComponent>(path)
        .map(|c| c.space_view)
}

fn load_viewport(
    blueprint_db: &re_data_store::StoreDb,
    space_views: HashMap<SpaceViewId, SpaceViewBlueprint>,
) -> Viewport {
    let auto_space_views = blueprint_db
        .store()
        .query_timeless_component::<AutoSpaceViews>(&VIEWPORT_PATH.into())
        .unwrap_or_else(|| {
            // Only enable auto-space-views if this is the app-default blueprint
            AutoSpaceViews(
                blueprint_db
                    .store_info()
                    .map_or(false, |ri| ri.is_app_default_blueprint()),
            )
        });

    let space_view_visibility = blueprint_db
        .store()
        .query_timeless_component::<SpaceViewVisibility>(&VIEWPORT_PATH.into())
        .unwrap_or_default();

    let space_view_maximized = blueprint_db
        .store()
        .query_timeless_component::<SpaceViewMaximized>(&VIEWPORT_PATH.into())
        .unwrap_or_default();

    let viewport_layout: ViewportLayout = blueprint_db
        .store()
        .query_timeless_component::<ViewportLayout>(&VIEWPORT_PATH.into())
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

// ----------------------------------------------------------------------------

fn sync_space_view(
    deltas: &mut Vec<DataRow>,
    space_view: &SpaceViewBlueprint,
    snapshot: Option<&SpaceViewBlueprint>,
) {
    if Some(space_view) != snapshot {
        let entity_path = EntityPath::from(format!(
            "{}/{}",
            SpaceViewComponent::SPACEVIEW_PREFIX,
            space_view.id
        ));

        // TODO(jleibs): Seq instead of timeless?
        let timepoint = TimePoint::timeless();

        let component = SpaceViewComponent {
            space_view: space_view.clone(),
        };

        add_delta_from_single_component(deltas, &entity_path, &timepoint, component);
    }
}

fn clear_space_view(deltas: &mut Vec<DataRow>, space_view_id: &SpaceViewId) {
    let entity_path = EntityPath::from(format!(
        "{}/{}",
        SpaceViewComponent::SPACEVIEW_PREFIX,
        space_view_id
    ));

    // TODO(jleibs): Seq instead of timeless?
    let timepoint = TimePoint::timeless();

    let cell =
        DataCell::from_arrow_empty(SpaceViewComponent::name(), SpaceViewComponent::data_type());

    let row = DataRow::from_cells1_sized(RowId::random(), entity_path, timepoint, 0, cell);

    deltas.push(row);
}

fn sync_viewport(deltas: &mut Vec<DataRow>, viewport: &Viewport, snapshot: &Viewport) {
    let entity_path = EntityPath::from(VIEWPORT_PATH);

    // TODO(jleibs): Seq instead of timeless?
    let timepoint = TimePoint::timeless();

    if viewport.auto_space_views != snapshot.auto_space_views {
        let component = AutoSpaceViews(viewport.auto_space_views);
        add_delta_from_single_component(deltas, &entity_path, &timepoint, component);
    }

    if viewport.visible != snapshot.visible {
        let component = SpaceViewVisibility(viewport.visible.clone());
        add_delta_from_single_component(deltas, &entity_path, &timepoint, component);
    }

    if viewport.maximized != snapshot.maximized {
        let component = SpaceViewMaximized(viewport.maximized);
        add_delta_from_single_component(deltas, &entity_path, &timepoint, component);
    }

    if viewport.trees != snapshot.trees
        || viewport.has_been_user_edited != snapshot.has_been_user_edited
    {
        let component = ViewportLayout {
            space_view_keys: viewport.space_views.keys().cloned().collect(),
            trees: viewport.trees.clone(),
            has_been_user_edited: viewport.has_been_user_edited,
        };

        add_delta_from_single_component(deltas, &entity_path, &timepoint, component);

        // TODO(jleibs): Sort out this causality mess
        // If we are saving a new layout, we also need to save the visibility-set because
        // it gets mutated on load but isn't guaranteed to be mutated on layout-change
        // which means it won't get saved.
        let component = SpaceViewVisibility(viewport.visible.clone());
        add_delta_from_single_component(deltas, &entity_path, &timepoint, component);
    }
}
