use std::collections::BTreeMap;

use re_arrow_store::LatestAtQuery;
use re_data_store::EntityPath;
use re_log_types::Timeline;
use re_query::query_archetype;
use re_viewer_context::{Item, SpaceViewClassIdentifier, SpaceViewId, ViewerContext};

use crate::{
    blueprint::components::{
        AutoLayout, AutoSpaceViews, IncludedSpaceViews, SpaceViewMaximized, ViewportLayout,
    },
    space_view::SpaceViewBlueprint,
    viewport::TreeActions,
    VIEWPORT_PATH,
};

// ----------------------------------------------------------------------------

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
                SpaceViewBlueprint::try_from_db(space_view, blueprint_db)
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

        // Remove the space view from the store
        if let Some(space_view) = self.space_views.get(space_view_id) {
            space_view.clear(ctx);
        }

        // If the space-view was maximized, clean it up
        if self.maximized == Some(*space_view_id) {
            self.set_maximized(None, ctx);
        }

        // Filter the space-view from the included space-views
        let component = IncludedSpaceViews(
            self.space_views
                .keys()
                .filter(|id| id != &space_view_id)
                .map(|id| (*id).into())
                .collect(),
        );
        ctx.save_blueprint_component(&VIEWPORT_PATH.into(), component);
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

        self.set_auto_layout(false, ctx);
        self.set_auto_space_views(false, ctx);
    }

    /// Add a set of space views to the viewport.
    ///
    /// NOTE: Calling this more than once per frame will result in lost data.
    /// Each call to `add_space_views` emits an updated list of [`IncludedSpaceViews`]
    /// Built by taking the list of [`IncludedSpaceViews`] from the current frame
    /// and adding the new space views to it. Since this the edit is not applied until
    /// the end of frame the second call will see a stale version of the data.
    // TODO(jleibs): Better safety check here.
    pub fn add_space_views(
        &self,
        space_views: impl Iterator<Item = SpaceViewBlueprint>,
        ctx: &ViewerContext<'_>,
        tree_actions: &mut TreeActions,
    ) {
        let mut new_ids: Vec<_> = vec![];

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
            space_view.save_to_blueprint_store(ctx);

            // Update the space-view ids:
            new_ids.push(space_view_id);
        }

        if !new_ids.is_empty() {
            tree_actions.create.extend(new_ids.iter());

            let updated_ids: Vec<_> = self.space_views.keys().chain(new_ids.iter()).collect();

            let component =
                IncludedSpaceViews(updated_ids.into_iter().map(|id| (*id).into()).collect());

            ctx.save_blueprint_component(&VIEWPORT_PATH.into(), component);
        }
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

    #[inline]
    pub fn set_auto_layout(&self, value: bool, ctx: &ViewerContext<'_>) {
        if self.auto_layout != value {
            let component = AutoLayout(value);
            ctx.save_blueprint_component(&VIEWPORT_PATH.into(), component);
        }
    }

    #[inline]
    pub fn set_auto_space_views(&self, value: bool, ctx: &ViewerContext<'_>) {
        if self.auto_layout != value {
            let component = AutoSpaceViews(value);
            ctx.save_blueprint_component(&VIEWPORT_PATH.into(), component);
        }
    }

    #[inline]
    pub fn set_maximized(&self, space_view_id: Option<SpaceViewId>, ctx: &ViewerContext<'_>) {
        if self.maximized != space_view_id {
            let component = SpaceViewMaximized(space_view_id.map(|id| id.into()));
            ctx.save_blueprint_component(&VIEWPORT_PATH.into(), component);
        }
    }

    #[inline]
    pub fn set_tree(&self, tree: &egui_tiles::Tree<SpaceViewId>, ctx: &ViewerContext<'_>) {
        if &self.tree != tree {
            re_log::trace!("Updating the layout tree");
            let component = ViewportLayout(tree.clone());
            ctx.save_blueprint_component(&VIEWPORT_PATH.into(), component);
        }
    }
}
