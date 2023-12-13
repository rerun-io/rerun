use std::collections::BTreeMap;

use re_arrow_store::LatestAtQuery;
use re_data_store::{EntityPath, StoreDb};
use re_log_types::{DataRow, RowId, TimePoint, Timeline};
use re_query::query_archetype;
use re_types_core::{archetypes::Clear, AsComponents as _};
use re_viewer_context::{
    CommandSender, Item, SpaceViewClassIdentifier, SpaceViewId, SystemCommand, SystemCommandSender,
    ViewerContext,
};

use crate::{
    blueprint::components::{
        AutoLayout, AutoSpaceViews, IncludedSpaceViews, SpaceViewMaximized, ViewportLayout,
    },
    space_info::SpaceInfoCollection,
    space_view::SpaceViewBlueprint,
    space_view_heuristics::default_created_space_views,
    VIEWPORT_PATH,
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
                .all(|sv| sv.class_identifier() == &SpaceViewClassIdentifier::invalid())
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
        *tree = egui_tiles::Tree::empty("viewport_tree");
        *maximized = None;
        *auto_layout = true;
        // Only enable auto-space-views if this is the app-default blueprint
        *auto_space_views = self
            .blueprint_db
            .store_info()
            .map_or(false, |ri| ri.is_app_default_blueprint());
        *tree_actions = Default::default();

        for space_view in
            default_created_space_views(ctx, spaces_info, ctx.entities_per_system_per_class)
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
            Item::DataBlueprintGroup(space_view_id, query_id, _entity_path) => self
                .space_views
                .get(space_view_id)
                .map_or(false, |sv| sv.queries.iter().any(|q| q.id == *query_id)),
            Item::Container(tile_id) => {
                if Some(*tile_id) == self.tree.root {
                    // the root tile is always visible
                    true
                } else if let Some(tile) = self.tree.tiles.get(*tile_id) {
                    if let egui_tiles::Tile::Container(container) = tile {
                        // single children containers are generally hidden
                        container.num_children() > 1
                    } else {
                        true
                    }
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
            self.tree = egui_tiles::Tree::empty("viewport_tree");
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
                    self.tree = egui_tiles::Tree::empty("viewport_tree");
                }
            } else {
                re_log::trace!("No root found - will re-run auto-layout");
            }
        }

        self.deferred_tree_actions.focus_tab = Some(space_view_id);

        space_view_id
    }

    #[allow(clippy::unused_self)]
    pub fn space_views_containing_entity_path(
        &self,
        ctx: &ViewerContext<'_>,
        path: &EntityPath,
    ) -> Vec<SpaceViewId> {
        self.space_views
            .iter()
            .filter_map(|(space_view_id, space_view)| {
                let query_result = ctx.lookup_query_result(space_view.query_id());
                if query_result
                    .tree
                    .lookup_result_by_path_and_group(path, false)
                    .is_some()
                {
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

        if after.space_views.len() != before.space_views.len()
            || after
                .space_views
                .keys()
                .zip(before.space_views.keys())
                .any(|(a, b)| a != b)
        {
            let component =
                IncludedSpaceViews(after.space_views.keys().map(|id| (*id).into()).collect());
            add_delta_from_single_component(&mut deltas, &entity_path, &timepoint, component);
        }

        if after.auto_layout != before.auto_layout {
            let component = AutoLayout(after.auto_layout);
            add_delta_from_single_component(&mut deltas, &entity_path, &timepoint, component);
        }

        if after.auto_space_views != before.auto_space_views {
            let component = AutoSpaceViews(after.auto_space_views);
            add_delta_from_single_component(&mut deltas, &entity_path, &timepoint, component);
        }

        if after.maximized != before.maximized {
            let component = SpaceViewMaximized(after.maximized.map(|id| id.into()));
            add_delta_from_single_component(&mut deltas, &entity_path, &timepoint, component);
        }

        if after.tree != before.tree {
            re_log::trace!("Syncing tree");

            let component = ViewportLayout(after.tree.clone());

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
        RowId::new(),
        entity_path.clone(),
        timepoint.clone(),
        1,
        [component],
    )
    .unwrap(); // TODO(emilk): statically check that the component is a mono-component - then this cannot fail!

    deltas.push(row);
}

// ----------------------------------------------------------------------------

pub fn load_viewport_blueprint(blueprint_db: &re_data_store::StoreDb) -> ViewportBlueprint<'_> {
    re_tracing::profile_function!();

    let query = LatestAtQuery::latest(Timeline::default());

    let arch = match query_archetype::<crate::blueprint::archetypes::ViewportBlueprint>(
        blueprint_db.store(),
        &query,
        &VIEWPORT_PATH.into(),
    )
    .and_then(|arch| arch.to_archetype())
    {
        Ok(arch) => arch,
        Err(re_query::QueryError::PrimaryNotFound(_)) => {
            // Empty Store
            Default::default()
        }
        Err(err) => {
            if cfg!(debug_assertions) {
                re_log::error!("Failed to load viewport blueprint: {err}.");
            } else {
                re_log::debug!("Failed to load viewport blueprint: {err}.");
            }
            Default::default()
        }
    };

    let space_view_ids: Vec<SpaceViewId> =
        arch.space_views.0.iter().map(|id| (*id).into()).collect();

    let space_views: BTreeMap<SpaceViewId, SpaceViewBlueprint> = space_view_ids
        .into_iter()
        .filter_map(|space_view: SpaceViewId| {
            SpaceViewBlueprint::try_from_db(&space_view.as_entity_path(), blueprint_db)
        })
        .map(|sv| (sv.id, sv))
        .collect();

    let auto_layout = arch.auto_layout.unwrap_or_default().0;

    let auto_space_views = arch.auto_space_views.map_or_else(
        || {
            // Only enable auto-space-views if this is the app-default blueprint
            blueprint_db
                .store_info()
                .map_or(false, |ri| ri.is_app_default_blueprint())
        },
        |auto| auto.0,
    );

    let maximized = arch.maximized.and_then(|id| id.0.map(|id| id.into()));

    let tree = blueprint_db
        .store()
        .query_timeless_component_quiet::<ViewportLayout>(&VIEWPORT_PATH.into())
        .map(|space_view| space_view.value)
        .unwrap_or_default()
        .0;

    ViewportBlueprint {
        blueprint_db,
        space_views,
        tree,
        maximized,
        auto_layout,
        auto_space_views,
        deferred_tree_actions: Default::default(),
    }

    // TODO(jleibs): Need to figure out if we have to re-enable support for
    // auto-discovery of SpaceViews logged via the experimental blueprint APIs.
    /*
    let unknown_space_views: HashMap<_, _> = space_views
        .iter()
        .filter(|(k, _)| !viewport_layout.space_view_keys.contains(k))
        .map(|(k, v)| (*k, v.clone()))
        .collect();
    */

    // TODO(jleibs): It seems we shouldn't call this until later, after we've created
    // the snapshot. Doing this here means we are mutating the state before it goes
    // into the snapshot. For example, even if there's no visibility in the
    // store, this will end up with default-visibility, which then *won't* be saved back.
    // TODO(jleibs): what to do about auto-discovery?
    /*
    for (_, view) in unknown_space_views {
        viewport.add_space_view(view);
    }
    */
}

// ----------------------------------------------------------------------------

pub fn sync_space_view(
    deltas: &mut Vec<DataRow>,
    space_view: &SpaceViewBlueprint,
    snapshot: Option<&SpaceViewBlueprint>,
) {
    if snapshot.map_or(true, |snapshot| space_view.has_edits(snapshot)) {
        // TODO(jleibs): Seq instead of timeless?
        let timepoint = TimePoint::timeless();

        let arch = re_types::blueprint::archetypes::SpaceViewBlueprint::new(
            space_view.class_identifier().as_str(),
        )
        .with_display_name(space_view.display_name.clone())
        .with_space_origin(&space_view.space_origin)
        .with_entities_determined_by_user(space_view.entities_determined_by_user)
        .with_contents(space_view.queries.iter().map(|q| q.id));

        if let Ok(row) = DataRow::from_archetype(
            RowId::new(),
            timepoint.clone(),
            space_view.entity_path(),
            &arch,
        ) {
            deltas.push(row);
        }

        // The only time we need to create a query is if this is a new space-view. All other edits
        // happen directly via `UpdateBlueprint` commands.
        if snapshot.is_none() {
            for query in &space_view.queries {
                add_delta_from_single_component(
                    deltas,
                    &query.id.as_entity_path(),
                    &timepoint,
                    query.expressions.clone(),
                );
            }
        }
    }
}

pub fn clear_space_view(deltas: &mut Vec<DataRow>, space_view_id: &SpaceViewId) {
    // TODO(jleibs): Seq instead of timeless?
    let timepoint = TimePoint::timeless();

    if let Ok(row) = DataRow::from_component_batches(
        RowId::new(),
        timepoint,
        space_view_id.as_entity_path(),
        Clear::recursive()
            .as_component_batches()
            .iter()
            .map(|b| b.as_ref()),
    ) {
        deltas.push(row);
    }
}
