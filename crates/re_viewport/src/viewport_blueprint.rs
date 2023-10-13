use std::collections::BTreeMap;

use ahash::HashMap;
use arrow2_convert::field::ArrowField;

use re_data_store::{EntityPath, StoreDb};
use re_log_types::{DataCell, DataRow, RowId, TimePoint};
use re_types::{blueprint::AutoSpaceViews, Loggable as _};
use re_viewer_context::{
    CommandSender, Item, SpaceViewClassName, SpaceViewId, SystemCommand, SystemCommandSender,
    ViewerContext,
};

use crate::{
    blueprint_components::{SpaceViewComponent, SpaceViewMaximized, ViewportLayout, VIEWPORT_PATH},
    space_info::SpaceInfoCollection,
    space_view::SpaceViewBlueprint,
    space_view_heuristics::{default_created_space_views, identify_entities_per_system_per_class},
};

// ----------------------------------------------------------------------------

// We delay any modifications to the tree until the end of the frame,
// so that we don't iterate over something while modifying it.
#[derive(Clone, Default)]
pub(crate) struct TreeActions {
    pub focus_tab: Option<SpaceViewId>,
    pub remove: Vec<egui_tiles::TileId>,
}

/// Describes the layout and contents of the Viewport Panel.
#[derive(Clone)]
pub struct ViewportBlueprint<'a> {
    /// The StoreDb used to instantiate this blueprint
    blueprint_db: &'a StoreDb,

    /// Where the space views are stored.
    ///
    /// Not a hashmap in order to preserve the order of the space views.
    pub space_views: BTreeMap<SpaceViewId, SpaceViewBlueprint>,

    /// The layouts of all the space views.
    pub tree: egui_tiles::Tree<SpaceViewId>,

    /// Show one tab as maximized?
    pub maximized: Option<SpaceViewId>,

    /// Whether the viewport layout is determined automatically.
    ///
    /// Set to `false` the first time the user messes around with the viewport blueprint.
    pub auto_layout: bool,

    /// Whether or not space views should be created automatically.
    pub auto_space_views: bool,

    /// Actions to perform at the end of the frame.
    ///
    /// We delay any modifications to the tree until the end of the frame,
    /// so that we don't mutate something while inspecitng it.
    pub(crate) deferred_tree_actions: TreeActions,
}

impl<'a> ViewportBlueprint<'a> {
    /// Determine whether all views in a blueprint are invalid.
    ///
    /// This most commonly happens due to a change in struct definition that
    /// breaks the definition of a serde-field, which means all views will
    /// become invalid.
    ///
    /// Note: the invalid check is used to potentially reset the blueprint, so we
    /// take the conservative stance that if any view is still usable we will still
    /// treat the blueprint as valid and show it.
    pub fn is_invalid(&self) -> bool {
        !self.space_views.is_empty()
            && self
                .space_views
                .values()
                .all(|sv| sv.class_name() == &SpaceViewClassName::invalid())
    }

    /// Reset the blueprint to a default state using some heuristics.
    pub fn reset(&mut self, ctx: &ViewerContext<'_>, spaces_info: &SpaceInfoCollection) {
        // TODO(jleibs): When using blueprint API, "reset" should go back to the initially transmitted
        // blueprint, not the default blueprint.
        re_tracing::profile_function!();

        let ViewportBlueprint {
            blueprint_db: _,
            space_views,
            tree,
            maximized,
            auto_layout,
            auto_space_views,
            deferred_tree_actions: tree_actions,
        } = self;

        // Note, it's important that these values match the behavior in `load_viewport_blueprint` below.
        *space_views = Default::default();
        *tree = Default::default();
        *maximized = None;
        *auto_layout = true;
        // Only enable auto-space-views if this is the app-default blueprint
        *auto_space_views = self
            .blueprint_db
            .store_info()
            .map_or(false, |ri| ri.is_app_default_blueprint());
        *tree_actions = Default::default();

        let entities_per_system_per_class = identify_entities_per_system_per_class(ctx);
        for space_view in
            default_created_space_views(ctx, spaces_info, &entities_per_system_per_class)
        {
            self.add_space_view(space_view);
        }
    }

    pub fn space_view_ids(&self) -> impl Iterator<Item = &SpaceViewId> + '_ {
        self.space_views.keys()
    }

    pub fn space_view(&self, space_view: &SpaceViewId) -> Option<&SpaceViewBlueprint> {
        self.space_views.get(space_view)
    }

    pub fn space_view_mut(
        &mut self,
        space_view_id: &SpaceViewId,
    ) -> Option<&mut SpaceViewBlueprint> {
        self.space_views.get_mut(space_view_id)
    }

    pub(crate) fn remove(&mut self, space_view_id: &SpaceViewId) -> Option<SpaceViewBlueprint> {
        self.mark_user_interaction();

        let Self {
            blueprint_db: _,
            space_views,
            tree,
            maximized,
            auto_layout: _,
            auto_space_views: _,
            deferred_tree_actions: _,
        } = self;

        if *maximized == Some(*space_view_id) {
            *maximized = None;
        }

        if let Some(tile_id) = tree.tiles.find_pane(space_view_id) {
            tree.tiles.remove(tile_id);
        }

        space_views.remove(space_view_id)
    }

    /// If `false`, the item is referring to data that is not present in this blueprint.
    pub fn is_item_valid(&self, item: &Item) -> bool {
        match item {
            Item::ComponentPath(_) => true,
            Item::InstancePath(space_view_id, _) => space_view_id
                .map(|space_view_id| self.space_view(&space_view_id).is_some())
                .unwrap_or(true),
            Item::SpaceView(space_view_id) => self.space_view(space_view_id).is_some(),
            Item::DataBlueprintGroup(space_view_id, data_blueprint_group_handle) => {
                if let Some(space_view) = self.space_view(space_view_id) {
                    space_view
                        .contents
                        .group(*data_blueprint_group_handle)
                        .is_some()
                } else {
                    false
                }
            }
        }
    }

    pub fn mark_user_interaction(&mut self) {
        if self.auto_layout {
            re_log::trace!("User edits - will no longer auto-layout");
        }

        self.auto_layout = false;
        self.auto_space_views = false;
    }

    pub fn add_space_view(&mut self, mut space_view: SpaceViewBlueprint) -> SpaceViewId {
        let space_view_id = space_view.id;

        // Find a unique name for the space view
        let mut candidate_name = space_view.display_name.clone();
        let mut append_count = 1;
        let unique_name = 'outer: loop {
            for view in &self.space_views {
                if candidate_name == view.1.display_name {
                    append_count += 1;
                    candidate_name = format!("{} ({})", space_view.display_name, append_count);

                    continue 'outer;
                }
            }
            break candidate_name;
        };

        space_view.display_name = unique_name;

        self.space_views.insert(space_view_id, space_view);

        if self.auto_layout {
            // Re-run the auto-layout next frame:
            re_log::trace!("Added a space view with no user edits yet - will re-run auto-layout");
            self.tree = Default::default();
        } else {
            // Try to insert it in the tree, in the top level:
            if let Some(root_id) = self.tree.root {
                let tile_id = self.tree.tiles.insert_pane(space_view_id);
                if let Some(egui_tiles::Tile::Container(container)) =
                    self.tree.tiles.get_mut(root_id)
                {
                    re_log::trace!("Inserting new space view into root container");
                    container.add_child(tile_id);
                } else {
                    re_log::trace!("Root was not a container - will re-run auto-layout");
                    self.tree = Default::default();
                }
            } else {
                re_log::trace!("No root found - will re-run auto-layout");
            }
        }

        self.deferred_tree_actions.focus_tab = Some(space_view_id);

        space_view_id
    }

    pub fn space_views_containing_entity_path(&self, path: &EntityPath) -> Vec<SpaceViewId> {
        self.space_views
            .iter()
            .filter_map(|(space_view_id, space_view)| {
                if space_view.contents.contains_entity(path) {
                    Some(*space_view_id)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Compares the before and after snapshots and sends any necessary deltas to the store.
    pub fn sync_viewport_blueprint(
        before: &ViewportBlueprint<'_>,
        after: &ViewportBlueprint<'_>,
        command_sender: &CommandSender,
    ) {
        let mut deltas = vec![];

        let entity_path = EntityPath::from(VIEWPORT_PATH);

        // TODO(jleibs): Seq instead of timeless?
        let timepoint = TimePoint::timeless();

        if after.auto_space_views != before.auto_space_views {
            let component = AutoSpaceViews(after.auto_space_views);
            add_delta_from_single_component(&mut deltas, &entity_path, &timepoint, component);
        }

        if after.maximized != before.maximized {
            let component = SpaceViewMaximized(after.maximized);
            add_delta_from_single_component(&mut deltas, &entity_path, &timepoint, component);
        }

        if after.tree != before.tree || after.auto_layout != before.auto_layout {
            re_log::trace!("Syncing tree");

            let component = ViewportLayout {
                space_view_keys: after.space_views.keys().cloned().collect(),
                tree: after.tree.clone(),
                auto_layout: after.auto_layout,
            };

            add_delta_from_single_component(&mut deltas, &entity_path, &timepoint, component);
        }

        // Add any new or modified space views
        for id in after.space_view_ids() {
            if let Some(space_view) = after.space_view(id) {
                sync_space_view(&mut deltas, space_view, before.space_view(id));
            }
        }

        // Remove any deleted space views
        for space_view_id in before.space_view_ids() {
            if after.space_view(space_view_id).is_none() {
                clear_space_view(&mut deltas, space_view_id);
            }
        }

        command_sender.send_system(SystemCommand::UpdateBlueprint(
            after.blueprint_db.store_id().clone(),
            deltas,
        ));
    }
}

// ----------------------------------------------------------------------------

// TODO(jleibs): Move this helper to a better location
fn add_delta_from_single_component<'a, C>(
    deltas: &mut Vec<DataRow>,
    entity_path: &EntityPath,
    timepoint: &TimePoint,
    component: C,
) where
    C: re_types::Component + Clone + 'a,
    std::borrow::Cow<'a, C>: std::convert::From<C>,
{
    let row = DataRow::from_cells1_sized(
        RowId::random(),
        entity_path.clone(),
        timepoint.clone(),
        1,
        [component],
    )
    .unwrap(); // TODO(emilk): statically check that the component is a mono-component - then this cannot fail!

    deltas.push(row);
}

// ----------------------------------------------------------------------------

pub fn load_space_view_blueprint(
    path: &EntityPath,
    blueprint_db: &re_data_store::StoreDb,
) -> Option<SpaceViewBlueprint> {
    re_tracing::profile_function!();

    let mut space_view = blueprint_db
        .store()
        .query_timeless_component::<SpaceViewComponent>(path)
        .map(|c| c.value.space_view);

    // Blueprint data migrations can leave us unable to parse the expected id from the source-data
    // We always want the id to match the one derived from the EntityPath since this id is how
    // we would end up removing it from the blueprint.
    let expected_id = SpaceViewId::from_entity_path(path);
    if let Some(space_view) = &mut space_view {
        if space_view.id != SpaceViewId::invalid() && space_view.id != expected_id {
            re_log::warn_once!(
                "SpaceViewBlueprint id is inconsistent with path: {:?}",
                space_view.id
            );
        }
        space_view.id = expected_id;
    }
    space_view
}

pub fn load_viewport_blueprint(blueprint_db: &re_data_store::StoreDb) -> ViewportBlueprint<'_> {
    re_tracing::profile_function!();

    let space_views: HashMap<SpaceViewId, SpaceViewBlueprint> = if let Some(space_views) =
        blueprint_db
            .entity_db()
            .tree
            .children
            .get(&re_data_store::EntityPathPart::Name(
                SpaceViewComponent::SPACEVIEW_PREFIX.into(),
            )) {
        space_views
            .children
            .values()
            .filter_map(|view_tree| load_space_view_blueprint(&view_tree.path, blueprint_db))
            .map(|sv| (sv.id, sv))
            .collect()
    } else {
        Default::default()
    };

    let auto_space_views = blueprint_db
        .store()
        .query_timeless_component::<AutoSpaceViews>(&VIEWPORT_PATH.into())
        .map_or_else(
            || {
                // Only enable auto-space-views if this is the app-default blueprint
                AutoSpaceViews(
                    blueprint_db
                        .store_info()
                        .map_or(false, |ri| ri.is_app_default_blueprint()),
                )
            },
            |auto| auto.value,
        );

    let space_view_maximized = blueprint_db
        .store()
        .query_timeless_component::<SpaceViewMaximized>(&VIEWPORT_PATH.into())
        .map(|space_view| space_view.value)
        .unwrap_or_default();

    let viewport_layout: ViewportLayout = blueprint_db
        .store()
        .query_timeless_component::<ViewportLayout>(&VIEWPORT_PATH.into())
        .map(|space_view| space_view.value)
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

    let mut viewport = ViewportBlueprint {
        blueprint_db,
        space_views: known_space_views,
        tree: viewport_layout.tree,
        maximized: space_view_maximized.0,
        auto_layout: viewport_layout.auto_layout,
        auto_space_views: auto_space_views.0,
        deferred_tree_actions: Default::default(),
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

pub fn sync_space_view(
    deltas: &mut Vec<DataRow>,
    space_view: &SpaceViewBlueprint,
    snapshot: Option<&SpaceViewBlueprint>,
) {
    if snapshot.map_or(true, |snapshot| space_view.has_edits(snapshot)) {
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

pub fn clear_space_view(deltas: &mut Vec<DataRow>, space_view_id: &SpaceViewId) {
    let entity_path = EntityPath::from(format!(
        "{}/{}",
        SpaceViewComponent::SPACEVIEW_PREFIX,
        space_view_id
    ));

    // TODO(jleibs): Seq instead of timeless?
    let timepoint = TimePoint::timeless();

    let cell =
        DataCell::from_arrow_empty(SpaceViewComponent::name(), SpaceViewComponent::data_type());

    let row = DataRow::from_cells1_sized(RowId::random(), entity_path, timepoint, 0, cell).unwrap();

    deltas.push(row);
}
