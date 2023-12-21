use std::collections::BTreeMap;

use ahash::HashMap;
use egui_tiles::{SimplificationOptions, TileId};
use re_arrow_store::LatestAtQuery;
use re_data_store::EntityPath;
use re_log_types::Timeline;
use re_query::query_archetype;
use re_viewer_context::{
    AppOptions, ContainerId, Item, SpaceViewClassIdentifier, SpaceViewId, ViewerContext,
};

use crate::{
    blueprint::components::{
        AutoLayout, AutoSpaceViews, IncludedSpaceViews, RootContainer, SpaceViewMaximized,
        ViewportLayout,
    },
    container::{blueprint_id_to_tile_id, ContainerBlueprint, Contents},
    space_view::SpaceViewBlueprint,
    viewport::TreeAction,
    VIEWPORT_PATH,
};

// ----------------------------------------------------------------------------

/// Describes the layout and contents of the Viewport Panel.
pub struct ViewportBlueprint {
    /// Where the space views are stored.
    ///
    /// Not a hashmap in order to preserve the order of the space views.
    pub space_views: BTreeMap<SpaceViewId, SpaceViewBlueprint>,

    /// All the containers found in the viewport.
    pub containers: BTreeMap<ContainerId, ContainerBlueprint>,

    /// The root container.
    pub root_container: Option<ContainerId>,

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

    /// Channel to pass Blueprint mutation messages back to the [`crate::Viewport`]
    tree_action_sender: std::sync::mpsc::Sender<TreeAction>,
}

impl ViewportBlueprint {
    /// Attempt to load a [`SpaceViewBlueprint`] from the blueprint store.
    pub fn try_from_db(
        blueprint_db: &re_data_store::StoreDb,
        app_options: &AppOptions,
        tree_action_sender: std::sync::mpsc::Sender<TreeAction>,
    ) -> Self {
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

        let all_container_ids: Vec<ContainerId> = blueprint_db
            .tree()
            .children
            .get(ContainerId::registry_part())
            .map(|tree| {
                tree.children
                    .values()
                    .map(|subtree| ContainerId::from_entity_path(&subtree.path))
                    .collect()
            })
            .unwrap_or_default();

        let containers: BTreeMap<ContainerId, ContainerBlueprint> = all_container_ids
            .into_iter()
            .filter_map(|id| ContainerBlueprint::try_from_db(blueprint_db, id))
            .map(|c| (c.id, c))
            .collect();

        let auto_layout = arch.auto_layout.unwrap_or_default().0;

        let root_container = arch.root_container.map(|id| id.0.into());

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

        let mut blueprint = ViewportBlueprint {
            space_views,
            containers,
            root_container,
            tree,
            maximized,
            auto_layout,
            auto_space_views,
            tree_action_sender,
        };

        if app_options.experimental_container_blueprints {
            let mut tree = blueprint.build_tree_from_containers();

            // TODO(abey79): Figure out if we want to simplify here or not.
            if false {
                let options = egui_tiles::SimplificationOptions {
                    all_panes_must_have_tabs: true,
                    ..Default::default()
                };

                tree.simplify(&options);
            }

            //re_log::trace!("shadow_tree: {shadow_tree:#?}");

            blueprint.tree = tree;
        }

        blueprint

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

    fn send_tree_action(&self, action: TreeAction) {
        if self.tree_action_sender.send(action).is_err() {
            re_log::warn_once!("Channel between ViewportBlueprint and Viewport is broken");
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
    /// The space view is added to the root container, or, if provided, to a given parent container.
    /// The list of created space view IDs is returned.
    ///
    /// Note that this doesn't focus the corresponding tab. Use [`Self::focus_tab`] with the returned ID
    /// if needed.
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
        parent_container: Option<egui_tiles::TileId>,
    ) -> Vec<SpaceViewId> {
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
            for id in &new_ids {
                self.send_tree_action(TreeAction::AddSpaceView(*id, parent_container));
            }

            let updated_ids: Vec<_> = self.space_views.keys().chain(new_ids.iter()).collect();

            let component =
                IncludedSpaceViews(updated_ids.into_iter().map(|id| (*id).into()).collect());

            ctx.save_blueprint_component(&VIEWPORT_PATH.into(), component);
        }

        new_ids
    }

    /// Add a container of the provided kind.
    ///
    /// The container is added to the root container or, if provided, to the given parent container.
    pub fn add_container(
        &self,
        kind: egui_tiles::ContainerKind,
        parent_container: Option<egui_tiles::TileId>,
    ) {
        self.send_tree_action(TreeAction::AddContainer(kind, parent_container));
    }

    /// Recursively remove a tile.
    ///
    /// All space views directly or indirectly contained by this tile are removed as well.
    pub fn remove(&self, tile_id: egui_tiles::TileId) {
        self.send_tree_action(TreeAction::Remove(tile_id));
    }

    /// Make sure the tab corresponding to this space view is focused.
    pub fn focus_tab(&self, space_view_id: SpaceViewId) {
        self.send_tree_action(TreeAction::FocusTab(space_view_id));
    }

    /// Set the kind of the provided container.
    pub fn set_container_kind(
        &self,
        container_id: egui_tiles::TileId,
        kind: egui_tiles::ContainerKind,
    ) {
        // no-op check
        if let Some(egui_tiles::Tile::Container(container)) = self.tree.tiles.get(container_id) {
            if container.kind() == kind {
                return;
            }
        }

        self.send_tree_action(TreeAction::SetContainerKind(container_id, kind));
    }

    /// Simplify the container subtree with the provided options.
    pub fn simplify_tree(
        &self,
        tile_id: egui_tiles::TileId,
        simplification_options: SimplificationOptions,
    ) {
        self.send_tree_action(TreeAction::SimplifyTree(tile_id, simplification_options));
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
        if self.auto_space_views != value {
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

    /// Save the current state of the viewport to the blueprint store.
    /// This should only be called if the tree was edited.
    pub fn save_tree_as_containers(
        &self,
        tree: &egui_tiles::Tree<SpaceViewId>,
        ctx: &ViewerContext<'_>,
    ) {
        re_tracing::profile_function!();
        re_log::trace!("Saving tree: {tree:#?}");

        // First, update the mapping for all the previously known containers.
        // These were inserted with their ids, so we want to keep these
        // constant if we find them again.
        let mut contents_from_tile_id: HashMap<TileId, Contents> = self
            .containers
            .keys()
            .map(|id| (blueprint_id_to_tile_id(id), Contents::Container(*id)))
            .collect();

        // Now, update the content mapping for all the new tiles in the tree.
        for (tile_id, tile) in tree.tiles.iter() {
            // If we already know about this tile, then we don't need
            // to do anything.
            if contents_from_tile_id.contains_key(tile_id) {
                continue;
            }
            match tile {
                egui_tiles::Tile::Pane(space_view_id) => {
                    // If a container has a pointer to a space-view
                    // we want it to point at the space-view in the blueprint.
                    contents_from_tile_id.insert(*tile_id, Contents::SpaceView(*space_view_id));
                }
                egui_tiles::Tile::Container(container) => {
                    if tree.root != Some(*tile_id)
                        && container.kind() == egui_tiles::ContainerKind::Tabs
                        && container.num_children() == 1
                    {
                        // If this is a tab-container with a single child, then it might be a
                        // "Trivial Tab", which egui_tiles adds to all space-views during simplification
                        // but doesn't need to be persisted back to the store.
                        if let Some(egui_tiles::Tile::Pane(space_view_id)) = container
                            .children()
                            .next()
                            .and_then(|child| tree.tiles.get(*child))
                        {
                            // This is a trivial tab -- this tile can point directly to
                            // the SpaceView and not to a Container.
                            contents_from_tile_id
                                .insert(*tile_id, Contents::SpaceView(*space_view_id));
                            continue;
                        }
                    }

                    // If this wasn't a container we knew about and wasn't a trivial container
                    // we will need to create a new container for it.
                    let container_id = ContainerId::random();
                    contents_from_tile_id.insert(*tile_id, Contents::Container(container_id));
                }
            }
        }

        // Clear any existing container blueprints that aren't referenced
        // by any tiles.
        for (container_id, container) in &self.containers {
            let tile_id = blueprint_id_to_tile_id(container_id);
            if !contents_from_tile_id.contains_key(&tile_id) {
                container.clear(ctx);
            }
        }

        // Now save any contents that are a container back to the blueprint
        for (tile_id, contents) in &contents_from_tile_id {
            if let Contents::Container(container_id) = contents {
                if let Some(egui_tiles::Tile::Container(container)) = tree.tiles.get(*tile_id) {
                    // TODO(jleibs): Make this only update the changed fields.
                    let blueprint = ContainerBlueprint::from_egui_tiles_container(
                        *container_id,
                        container,
                        &contents_from_tile_id,
                    );

                    blueprint.save_to_blueprint_store(ctx);
                }
            }
        }

        // Finally update the root
        if let Some(root_container) = tree
            .root()
            .and_then(|root| contents_from_tile_id.get(&root))
            .and_then(|contents| contents.as_container_id())
            .map(|container_id| RootContainer((container_id).into()))
        {
            ctx.save_blueprint_component(&VIEWPORT_PATH.into(), root_container);
        } else {
            ctx.save_empty_blueprint_component::<RootContainer>(&VIEWPORT_PATH.into());
        }
    }

    pub fn build_tree_from_containers(&self) -> egui_tiles::Tree<SpaceViewId> {
        re_tracing::profile_function!();
        let mut tree = egui_tiles::Tree::empty("viewport_tree");

        // First add all the space_views
        for space_view in self.space_views.keys() {
            let tile_id = blueprint_id_to_tile_id(space_view);
            let pane = egui_tiles::Tile::Pane(*space_view);
            tree.tiles.insert(tile_id, pane);
        }

        // Now add all the containers
        for container in self.containers.values() {
            let tile_id = blueprint_id_to_tile_id(&container.id);

            tree.tiles.insert(tile_id, container.to_tile());
        }

        // And finally, set the root
        if let Some(root_container) = self.root_container.map(|id| blueprint_id_to_tile_id(&id)) {
            tree.root = Some(root_container);
        }

        tree
    }
}
