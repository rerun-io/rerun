use std::collections::BTreeMap;

use parking_lot::Mutex;
use re_arrow_store::LatestAtQuery;
use re_data_store::EntityPath;
use re_log_types::{DataRow, RowId, TimePoint, Timeline};
use re_query::query_archetype;
use re_types_core::{archetypes::Clear, AsComponents as _};
use re_viewer_context::{
    Item, SpaceViewClassIdentifier, SpaceViewId, SystemCommand, SystemCommandSender, ViewerContext,
};

use crate::{
    blueprint::components::{
        AutoLayout, AutoSpaceViews, IncludedSpaceViews, SpaceViewMaximized, ViewportLayout,
    },
    space_view::SpaceViewBlueprint,
    VIEWPORT_PATH,
};

// ----------------------------------------------------------------------------

// We delay any modifications to the tree until the end of the frame,
// so that we don't iterate over something while modifying it.
#[derive(Clone, Default)]
pub(crate) struct TreeActions {
    pub reset: bool,
    pub create: Vec<SpaceViewId>,
    pub focus_tab: Option<SpaceViewId>,
    pub remove: Vec<egui_tiles::TileId>,
}

/// Describes the layout and contents of the Viewport Panel.
pub struct ViewportBlueprint {
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
    //TODO(jleibs): Can we use the SystemCommandSender for this, too?
    pub(crate) deferred_tree_actions: Mutex<TreeActions>,
}

impl ViewportBlueprint {
    pub fn try_from_db(blueprint_db: &re_data_store::StoreDb) -> Self {
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

    pub(crate) fn remove_space_view(&self, space_view_id: &SpaceViewId, ctx: &ViewerContext<'_>) {
        self.mark_user_interaction(ctx);

        let timepoint = TimePoint::timeless();
        let mut deltas = vec![];

        if self.maximized == Some(*space_view_id) {
            let component = SpaceViewMaximized(None);
            add_delta_from_single_component(
                &mut deltas,
                &VIEWPORT_PATH.into(),
                &timepoint,
                component,
            );
        }

        let component = IncludedSpaceViews(
            self.space_views
                .keys()
                .filter(|id| id != &space_view_id)
                .map(|id| (*id).into())
                .collect(),
        );
        add_delta_from_single_component(&mut deltas, &VIEWPORT_PATH.into(), &timepoint, component);

        clear_space_view(&mut deltas, space_view_id);

        ctx.command_sender
            .send_system(SystemCommand::UpdateBlueprint(
                ctx.store_context.blueprint.store_id().clone(),
                deltas,
            ));
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

    pub fn mark_user_interaction(&self, ctx: &ViewerContext<'_>) {
        if self.auto_layout {
            re_log::trace!("User edits - will no longer auto-layout");
        }

        let component = AutoLayout(false);
        save_single_component(&VIEWPORT_PATH.into(), component, ctx);

        let component = AutoSpaceViews(false);
        save_single_component(&VIEWPORT_PATH.into(), component, ctx);
    }

    pub fn add_space_view(
        &self,
        mut space_view: SpaceViewBlueprint,
        ctx: &ViewerContext<'_>,
    ) -> SpaceViewId {
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

        // Save the space view to the store
        space_view.save_full(ctx);

        // Update the space-view ids:
        let component = IncludedSpaceViews(
            self.space_views
                .keys()
                .map(|id| (*id).into())
                .chain(std::iter::once(space_view_id.into()))
                .collect(),
        );
        save_single_component(&VIEWPORT_PATH.into(), component, ctx);

        self.deferred_tree_actions.lock().create.push(space_view_id);

        space_view_id
    }

    pub fn add_multi_space_view(
        &self,
        space_views: impl Iterator<Item = SpaceViewBlueprint>,
        ctx: &ViewerContext<'_>,
    ) {
        let mut new_ids: Vec<_> = self.space_views.keys().cloned().collect();

        for mut space_view in space_views {
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

            // Save the space view to the store
            space_view.save_full(ctx);
            new_ids.push(space_view_id);

            // Update the space-view ids:

            self.deferred_tree_actions.lock().create.push(space_view_id);
        }

        let component = IncludedSpaceViews(new_ids.into_iter().map(|id| id.into()).collect());
        save_single_component(&VIEWPORT_PATH.into(), component, ctx);
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

    pub fn set_auto_layout(value: bool, ctx: &ViewerContext<'_>) {
        let component = AutoLayout(value);
        save_single_component(&VIEWPORT_PATH.into(), component, ctx);
    }

    pub fn set_maximized(space_view_id: Option<SpaceViewId>, ctx: &ViewerContext<'_>) {
        let component = SpaceViewMaximized(space_view_id.map(|id| id.into()));
        save_single_component(&VIEWPORT_PATH.into(), component, ctx);
    }

    pub fn set_tree(tree: egui_tiles::Tree<SpaceViewId>, ctx: &ViewerContext<'_>) {
        let component = ViewportLayout(tree);
        save_single_component(&VIEWPORT_PATH.into(), component, ctx);
    }
}

// ----------------------------------------------------------------------------

// TODO(jleibs): Move this helper to a better location
pub fn add_delta_from_single_component<'a, C>(
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

// TODO(jleibs): Move this helper to a better location
pub fn save_single_component<'a, C>(entity_path: &EntityPath, component: C, ctx: &ViewerContext<'_>)
where
    C: re_types::Component + Clone + 'a,
    std::borrow::Cow<'a, C>: std::convert::From<C>,
{
    let timepoint = TimePoint::timeless();

    let row = DataRow::from_cells1_sized(
        RowId::new(),
        entity_path.clone(),
        timepoint.clone(),
        1,
        [component],
    )
    .unwrap(); // TODO(emilk): statically check that the component is a mono-component - then this cannot fail!

    ctx.command_sender
        .send_system(SystemCommand::UpdateBlueprint(
            ctx.store_context.blueprint.store_id().clone(),
            vec![row],
        ));
}

// ----------------------------------------------------------------------------

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
