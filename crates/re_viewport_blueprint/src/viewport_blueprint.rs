use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};

use ahash::HashMap;
use egui_tiles::{SimplificationOptions, TileId};
use nohash_hasher::IntSet;
use re_types::{Archetype as _, SpaceViewClassIdentifier};
use smallvec::SmallVec;

use re_data_store2::LatestAtQuery;
use re_entity_db::EntityPath;
use re_types::blueprint::components::ViewerRecommendationHash;
use re_types_blueprint::blueprint::archetypes as blueprint_archetypes;
use re_types_blueprint::blueprint::components::{
    AutoLayout, AutoSpaceViews, IncludedSpaceView, RootContainer, SpaceViewMaximized,
};
use re_viewer_context::{
    blueprint_id_to_tile_id, ContainerId, Contents, Item, SpaceViewId, ViewerContext,
};

use crate::{container::ContainerBlueprint, SpaceViewBlueprint, TreeAction, VIEWPORT_PATH};

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
    /// Note: we use a mutex here because writes needs to be effective immediately during the frame.
    auto_layout: AtomicBool,

    /// Whether space views should be created automatically.
    ///
    /// Note: we use a mutex here because writes needs to be effective immediately during the frame.
    auto_space_views: AtomicBool,

    /// Hashes of all recommended space views the viewer has already added and that should not be added again.
    past_viewer_recommendations: IntSet<ViewerRecommendationHash>,

    /// Channel to pass Blueprint mutation messages back to the Viewport.
    tree_action_sender: std::sync::mpsc::Sender<TreeAction>,
}

impl ViewportBlueprint {
    /// Attempt to load a [`SpaceViewBlueprint`] from the blueprint store.
    pub fn try_from_db(
        blueprint_db: &re_entity_db::EntityDb,
        query: &LatestAtQuery,
        tree_action_sender: std::sync::mpsc::Sender<TreeAction>,
    ) -> Self {
        re_tracing::profile_function!();

        let resolver = blueprint_db.resolver();
        let results = blueprint_db.query_caches().latest_at(
            blueprint_db.store(),
            query,
            &VIEWPORT_PATH.into(),
            blueprint_archetypes::ViewportBlueprint::all_components()
                .iter()
                .copied(),
        );

        let blueprint_archetypes::ViewportBlueprint {
            root_container,
            maximized,
            auto_layout,
            auto_space_views,
            past_viewer_recommendations,
        } = blueprint_archetypes::ViewportBlueprint {
            root_container: results.get_instance(resolver, 0),
            maximized: results.get_instance(resolver, 0),
            auto_layout: results.get_instance(resolver, 0),
            auto_space_views: results.get_instance(resolver, 0),
            past_viewer_recommendations: results.get_vec(resolver),
        };

        let all_space_view_ids: Vec<SpaceViewId> = blueprint_db
            .tree()
            .children
            .get(SpaceViewId::registry_part())
            .map(|tree| {
                tree.children
                    .values()
                    .map(|subtree| SpaceViewId::from_entity_path(&subtree.path))
                    .collect()
            })
            .unwrap_or_default();

        let space_views: BTreeMap<SpaceViewId, SpaceViewBlueprint> = all_space_view_ids
            .into_iter()
            .filter_map(|space_view: SpaceViewId| {
                SpaceViewBlueprint::try_from_db(space_view, blueprint_db, query)
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
            .filter_map(|id| ContainerBlueprint::try_from_db(blueprint_db, query, id))
            .map(|c| (c.id, c))
            .collect();

        let root_container = root_container.map(|id| id.0.into());

        // Auto layouting and auto space view are only enabled if no blueprint has been provided by the user.
        // Only enable auto-space-views if this is the app-default blueprint
        let is_app_default_blueprint = blueprint_db
            .store_info()
            .map_or(false, |ri| ri.is_app_default_blueprint());
        let auto_layout =
            AtomicBool::new(auto_layout.map_or(is_app_default_blueprint, |auto| auto.0));
        let auto_space_views =
            AtomicBool::new(auto_space_views.map_or(is_app_default_blueprint, |auto| auto.0));

        let tree = build_tree_from_space_views_and_containers(
            space_views.values(),
            containers.values(),
            root_container,
        );

        let past_viewer_recommendations = past_viewer_recommendations
            .unwrap_or_default()
            .iter()
            .cloned()
            .collect();

        Self {
            space_views,
            containers,
            root_container,
            tree,
            maximized: maximized.map(|id| id.0.into()),
            auto_layout,
            auto_space_views,
            past_viewer_recommendations,
            tree_action_sender,
        }
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
                .all(|sv| sv.class_identifier() == SpaceViewClassIdentifier::invalid())
    }

    pub fn space_view_ids(&self) -> impl Iterator<Item = &SpaceViewId> + '_ {
        self.space_views.keys()
    }

    pub fn space_view(&self, space_view: &SpaceViewId) -> Option<&SpaceViewBlueprint> {
        self.space_views.get(space_view)
    }

    pub fn container(&self, container_id: &ContainerId) -> Option<&ContainerBlueprint> {
        self.containers.get(container_id)
    }

    pub fn space_view_mut(
        &mut self,
        space_view_id: &SpaceViewId,
    ) -> Option<&mut SpaceViewBlueprint> {
        self.space_views.get_mut(space_view_id)
    }

    pub fn remove_space_view(&self, space_view_id: &SpaceViewId, ctx: &ViewerContext<'_>) {
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
        let components = self
            .space_views
            .keys()
            .filter(|id| id != &space_view_id)
            .map(|id| IncludedSpaceView((*id).into()))
            .collect::<Vec<_>>();
        ctx.save_blueprint_component(&VIEWPORT_PATH.into(), &components);
    }

    /// Duplicates a space view and its entity property overrides.
    pub fn duplicate_space_view(
        &self,
        space_view_id: &SpaceViewId,
        ctx: &ViewerContext<'_>,
    ) -> Option<SpaceViewId> {
        let space_view = self.space_view(space_view_id)?;

        let new_space_view = space_view.duplicate(ctx.store_context, ctx.blueprint_query);
        let new_space_view_id = new_space_view.id;

        let parent_and_pos =
            self.find_parent_and_position_index(&Contents::SpaceView(*space_view_id));

        self.add_space_views(
            std::iter::once(new_space_view),
            ctx,
            parent_and_pos.map(|(parent, _)| parent),
            parent_and_pos.map(|(_, pos)| pos),
        );

        self.mark_user_interaction(ctx);

        Some(new_space_view_id)
    }

    /// If `false`, the item is referring to data that is not present in this blueprint.
    ///
    /// TODO(#5742): note that `Item::DataResult` with entity path set to the space origin or some
    /// of its descendent are always considered valid.
    pub fn is_item_valid(
        &self,
        store_context: &re_viewer_context::StoreContext<'_>,
        item: &Item,
    ) -> bool {
        match item {
            Item::AppId(app_id) => store_context
                .hub
                .store_bundle()
                .entity_dbs()
                .any(|db| db.app_id() == Some(app_id)),

            Item::DataSource(_)
            | Item::StoreId(_)
            | Item::ComponentPath(_)
            | Item::InstancePath(_) => true,

            Item::SpaceView(space_view_id) => self.space_view(space_view_id).is_some(),

            Item::DataResult(space_view_id, instance_path) => {
                self.space_view(space_view_id).map_or(false, |space_view| {
                    let entity_path = &instance_path.entity_path;

                    // TODO(#5742): including any path that is—or descend from—the space origin is
                    // necessary because such items may actually be displayed in the blueprint tree.
                    entity_path == &space_view.space_origin
                        || entity_path.is_descendant_of(&space_view.space_origin)
                        || space_view
                            .contents
                            .entity_path_filter
                            .is_included(&instance_path.entity_path)
                })
            }

            Item::Container(container_id) => self.container(container_id).is_some(),
        }
    }

    fn send_tree_action(&self, action: TreeAction) {
        if self.tree_action_sender.send(action).is_err() {
            re_log::warn_once!("Channel between ViewportBlueprint and Viewport is broken");
        }
    }

    pub fn mark_user_interaction(&self, ctx: &ViewerContext<'_>) {
        if self.auto_layout() {
            re_log::trace!("User edits - will no longer auto-layout");
        }

        self.set_auto_layout(false, ctx);
        self.set_auto_space_views(false, ctx);
    }

    pub fn on_frame_start(&self, ctx: &ViewerContext<'_>) {
        if self.auto_space_views() {
            self.spawn_heuristic_space_views(ctx);
        }
    }

    fn spawn_heuristic_space_views(&self, ctx: &ViewerContext<'_>) {
        re_tracing::profile_function!();

        for entry in ctx.space_view_class_registry.iter_registry() {
            let class_id = entry.identifier;
            let mut recommended_space_views = entry.class.spawn_heuristics(ctx).into_vec();

            re_tracing::profile_scope!("filter_recommendations_for", class_id);

            // Remove all space views that we already spawned via heuristic before.
            recommended_space_views.retain(|recommended_view| {
                !self
                    .past_viewer_recommendations
                    .contains(&recommended_view.recommendation_hash(class_id))
            });

            // Each of the remaining recommendations would individually be a candidate for spawning if there were
            // no other space views in the viewport.
            // In the following steps we further filter this list depending on what's on screen already,
            // as well as redundancy within the recommendation itself BUT this is an important checkpoint:
            // All the other views may change due to user interaction, but this does *not* mean
            // that we should suddenly spawn the views we're filtering out here.
            // Therefore everything so far needs to be added to `past_viewer_recommendations`,
            // which marks this as "already processed recommendation".
            //
            // Example:
            // Recommendation contains `/**` and `/camera/**`.
            // We filter out `/camera/**` because that would be redundant to `/**`.
            // If now the user edits the space view at `/**` to be `/points/**`, that does *not*
            // mean we should suddenly add `/camera/**` to the viewport.
            if !recommended_space_views.is_empty() {
                let new_viewer_recommendation_hashes = self
                    .past_viewer_recommendations
                    .iter()
                    .cloned()
                    .chain(
                        recommended_space_views
                            .iter()
                            .map(|recommendation| recommendation.recommendation_hash(class_id)),
                    )
                    .collect::<Vec<_>>();

                ctx.save_blueprint_component(
                    &VIEWPORT_PATH.into(),
                    &new_viewer_recommendation_hashes,
                );
            }

            // Remove all space views that have all the entities we already have on screen.
            let existing_path_filters = self
                .space_views
                .values()
                .filter(|space_view| space_view.class_identifier() == class_id)
                .map(|space_view| &space_view.contents.entity_path_filter)
                .collect::<Vec<_>>();
            recommended_space_views.retain(|recommended_view| {
                existing_path_filters.iter().all(|existing_filter| {
                    !existing_filter.is_superset_of(&recommended_view.query_filter)
                })
            });

            // Remove all space views that are redundant within the remaining recommendation.
            // This n^2 loop should only run ever for frames that add new space views.
            let final_recommendations = recommended_space_views
                .iter()
                .enumerate()
                .filter(|(j, candidate)| {
                    recommended_space_views
                        .iter()
                        .enumerate()
                        .all(|(i, other)| {
                            i == *j || !other.query_filter.is_superset_of(&candidate.query_filter)
                        })
                })
                .map(|(_, recommendation)| recommendation);

            self.add_space_views(
                final_recommendations.map(|recommendation| {
                    SpaceViewBlueprint::new(class_id, recommendation.clone())
                }),
                ctx,
                None,
                None,
            );
        }
    }

    /// Add a set of space views to the viewport.
    ///
    /// The space view is added to the root container, or, if provided, to a given parent container.
    /// The list of created space view IDs is returned.
    ///
    /// Note that this doesn't focus the corresponding tab. Use [`Self::focus_tab`] with the returned ID
    /// if needed.
    pub fn add_space_views(
        &self,
        space_views: impl Iterator<Item = SpaceViewBlueprint>,
        ctx: &ViewerContext<'_>,
        parent_container: Option<ContainerId>,
        position_in_parent: Option<usize>,
    ) -> Vec<SpaceViewId> {
        let mut new_ids: Vec<_> = vec![];

        for space_view in space_views {
            let space_view_id = space_view.id;

            // Save the space view to the store
            space_view.save_to_blueprint_store(ctx);

            // Update the space-view ids:
            new_ids.push(space_view_id);
        }

        if !new_ids.is_empty() {
            for id in &new_ids {
                self.send_tree_action(TreeAction::AddSpaceView(
                    *id,
                    parent_container,
                    position_in_parent,
                ));
            }
        }

        new_ids
    }

    /// Returns an iterator over all the contents (space views and containers) in the viewport.
    pub fn contents_iter(&self) -> impl Iterator<Item = Contents> + '_ {
        self.space_views
            .keys()
            .map(|space_view_id| Contents::SpaceView(*space_view_id))
            .chain(
                self.containers
                    .keys()
                    .map(|container_id| Contents::Container(*container_id)),
            )
    }

    /// Walk the entire [`Contents`] tree, starting from the root container.
    ///
    /// See [`Self::visit_contents_in_container`] for details.
    pub fn visit_contents(&self, visitor: &mut impl FnMut(&Contents, &SmallVec<[ContainerId; 4]>)) {
        if let Some(root_container) = self.root_container {
            self.visit_contents_in_container(&root_container, visitor);
        }
    }

    /// Walk the subtree defined by the provided container id and call `visitor` for each
    /// [`Contents`].
    ///
    /// Note:
    /// - `visitor` is first called for the container passed in argument
    /// - `visitor`'s second argument contains the hierarchy leading to the visited contents, from
    ///   (and including) the container passed in argument
    pub fn visit_contents_in_container(
        &self,
        container_id: &ContainerId,
        visitor: &mut impl FnMut(&Contents, &SmallVec<[ContainerId; 4]>),
    ) {
        let mut hierarchy = SmallVec::new();
        self.visit_contents_in_container_impl(container_id, &mut hierarchy, visitor);
    }

    fn visit_contents_in_container_impl(
        &self,
        container_id: &ContainerId,
        hierarchy: &mut SmallVec<[ContainerId; 4]>,
        visitor: &mut impl FnMut(&Contents, &SmallVec<[ContainerId; 4]>),
    ) {
        visitor(&Contents::Container(*container_id), hierarchy);
        if let Some(container) = self.container(container_id) {
            hierarchy.push(*container_id);
            for contents in &container.contents {
                visitor(contents, hierarchy);
                match contents {
                    Contents::Container(container_id) => {
                        self.visit_contents_in_container_impl(container_id, hierarchy, visitor);
                    }
                    Contents::SpaceView(_) => {}
                }
            }
            hierarchy.pop();
        }
    }

    /// Given a predicate, finds the (first) matching contents by recursively walking from the root
    /// container.
    pub fn find_contents_by(&self, predicate: &impl Fn(&Contents) -> bool) -> Option<Contents> {
        if let Some(root_container) = self.root_container {
            self.find_contents_in_container_by(predicate, &root_container)
        } else {
            None
        }
    }

    /// Given a predicate, finds the (first) matching contents by recursively walking from the given
    /// container.
    pub fn find_contents_in_container_by(
        &self,
        predicate: &impl Fn(&Contents) -> bool,
        container_id: &ContainerId,
    ) -> Option<Contents> {
        if predicate(&Contents::Container(*container_id)) {
            return Some(Contents::Container(*container_id));
        }

        let container = self.container(container_id)?;

        for contents in &container.contents {
            if predicate(contents) {
                return Some(*contents);
            }

            match contents {
                Contents::Container(container_id) => {
                    let res = self.find_contents_in_container_by(predicate, container_id);
                    if res.is_some() {
                        return res;
                    }
                }
                Contents::SpaceView(_) => {}
            }
        }

        None
    }

    /// Checks if some content is (directly or indirectly) contained in the given container.
    pub fn is_contents_in_container(
        &self,
        contents: &Contents,
        container_id: &ContainerId,
    ) -> bool {
        self.find_contents_in_container_by(&|c| c == contents, container_id)
            .is_some()
    }

    /// Given a container or a space view, find its enclosing container and its position within it.
    pub fn find_parent_and_position_index(
        &self,
        contents: &Contents,
    ) -> Option<(ContainerId, usize)> {
        if let Some(container_id) = self.root_container {
            if *contents == Contents::Container(container_id) {
                // root doesn't have a parent
                return None;
            }
            self.find_parent_and_position_index_impl(contents, &container_id)
        } else {
            None
        }
    }

    fn find_parent_and_position_index_impl(
        &self,
        contents: &Contents,
        container_id: &ContainerId,
    ) -> Option<(ContainerId, usize)> {
        let container = self.container(container_id)?;

        for (pos, child_contents) in container.contents.iter().enumerate() {
            if child_contents == contents {
                return Some((*container_id, pos));
            }

            match child_contents {
                Contents::Container(child_container_id) => {
                    let res =
                        self.find_parent_and_position_index_impl(contents, child_container_id);
                    if res.is_some() {
                        return res;
                    }
                }
                Contents::SpaceView(_) => {}
            }
        }

        None
    }

    /// Add a container of the provided kind.
    ///
    /// The container is added to the root container or, if provided, to the given parent container.
    pub fn add_container(
        &self,
        kind: egui_tiles::ContainerKind,
        parent_container: Option<ContainerId>,
    ) {
        self.send_tree_action(TreeAction::AddContainer(kind, parent_container));
    }

    /// Recursively remove a container or a space view.
    pub fn remove_contents(&self, contents: Contents) {
        self.send_tree_action(TreeAction::RemoveContents(contents));
    }

    /// Move the `contents` container or space view to the specified target container and position.
    pub fn move_contents(
        &self,
        contents: Contents,
        target_container: ContainerId,
        target_position_in_container: usize,
    ) {
        self.send_tree_action(TreeAction::MoveContents {
            contents_to_move: contents,
            target_container,
            target_position_in_container,
        });
    }

    /// Move some [`Contents`] to a newly created container of the given kind.
    pub fn move_contents_to_new_container(
        &self,
        contents: Vec<Contents>,
        new_container_kind: egui_tiles::ContainerKind,
        target_container: ContainerId,
        target_position_in_container: usize,
    ) {
        self.send_tree_action(TreeAction::MoveContentsToNewContainer {
            contents_to_move: contents,
            new_container_kind,
            target_container,
            target_position_in_container,
        });
    }

    /// Make sure the tab corresponding to this space view is focused.
    pub fn focus_tab(&self, space_view_id: SpaceViewId) {
        self.send_tree_action(TreeAction::FocusTab(space_view_id));
    }

    /// Set the kind of the provided container.
    pub fn set_container_kind(&self, container_id: ContainerId, kind: egui_tiles::ContainerKind) {
        // no-op check
        if let Some(container) = self.container(&container_id) {
            if container.container_kind == kind {
                return;
            }
        }

        self.send_tree_action(TreeAction::SetContainerKind(container_id, kind));
    }

    /// Simplify the container tree with the provided options.
    pub fn simplify_container(
        &self,
        container_id: &ContainerId,
        simplification_options: SimplificationOptions,
    ) {
        self.send_tree_action(TreeAction::SimplifyContainer(
            *container_id,
            simplification_options,
        ));
    }

    /// Make all children of the given container the same size.
    pub fn make_all_children_same_size(&self, container_id: &ContainerId) {
        self.send_tree_action(TreeAction::MakeAllChildrenSameSize(*container_id));
    }

    /// Check the visibility of the provided content.
    ///
    /// This function may be called from UI code.
    pub fn is_contents_visible(&self, contents: &Contents) -> bool {
        match contents {
            Contents::Container(container_id) => {
                if let Some(container) = self.container(container_id) {
                    container.visible
                } else {
                    re_log::warn_once!(
                        "Visibility check failed due to unknown container id {container_id:?}"
                    );

                    false
                }
            }
            Contents::SpaceView(space_view_id) => {
                if let Some(space_view) = self.space_view(space_view_id) {
                    space_view.visible
                } else {
                    re_log::warn_once!(
                        "Visibility check failed due to unknown space view id {space_view_id:?}"
                    );

                    false
                }
            }
        }
    }

    /// Sets the visibility for the provided content.
    ///
    /// This function may be called from UI code.
    pub fn set_content_visibility(
        &self,
        ctx: &ViewerContext<'_>,
        contents: &Contents,
        visible: bool,
    ) {
        match contents {
            Contents::Container(container_id) => {
                if let Some(container) = self.container(container_id) {
                    if visible != container.visible {
                        if self.auto_layout() {
                            re_log::trace!(
                                "Container visibility changed - will no longer auto-layout"
                            );
                        }

                        self.set_auto_layout(false, ctx);
                        container.set_visible(ctx, visible);
                    }
                } else {
                    re_log::warn_once!(
                        "Visibility change failed due to unknown container id {container_id:?}"
                    );
                }
            }
            Contents::SpaceView(space_view_id) => {
                if let Some(space_view) = self.space_view(space_view_id) {
                    if visible != space_view.visible {
                        if self.auto_layout() {
                            re_log::trace!(
                                "Space-view visibility changed - will no longer auto-layout"
                            );
                        }

                        self.set_auto_layout(false, ctx);
                        space_view.set_visible(ctx, visible);
                    }
                } else {
                    re_log::warn_once!(
                        "Visibility change failed due to unknown space view id {space_view_id:?}"
                    );
                }
            }
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
                let query_result = ctx.lookup_query_result(space_view.id);
                if query_result.tree.lookup_result_by_path(path).is_some() {
                    Some(*space_view_id)
                } else {
                    None
                }
            })
            .collect()
    }

    #[inline]
    pub fn set_auto_layout(&self, value: bool, ctx: &ViewerContext<'_>) {
        let old_value = self.auto_layout.swap(value, Ordering::SeqCst);

        if old_value != value {
            let component = AutoLayout(value);
            ctx.save_blueprint_component(&VIEWPORT_PATH.into(), &component);
        }
    }

    #[inline]
    pub fn auto_layout(&self) -> bool {
        self.auto_layout.load(Ordering::SeqCst)
    }

    #[inline]
    pub fn set_auto_space_views(&self, value: bool, ctx: &ViewerContext<'_>) {
        let old_value = self.auto_space_views.swap(value, Ordering::SeqCst);

        if old_value != value {
            let component = AutoSpaceViews(value);
            ctx.save_blueprint_component(&VIEWPORT_PATH.into(), &component);
        }
    }

    #[inline]
    pub fn auto_space_views(&self) -> bool {
        self.auto_space_views.load(Ordering::SeqCst)
    }

    #[inline]
    pub fn set_maximized(&self, space_view_id: Option<SpaceViewId>, ctx: &ViewerContext<'_>) {
        if self.maximized != space_view_id {
            let component_batch = space_view_id.map(|id| SpaceViewMaximized(id.into()));
            ctx.save_blueprint_component(&VIEWPORT_PATH.into(), &component_batch);
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
            if tree.tiles.get(tile_id).is_none() {
                container.clear(ctx);
            }
        }

        // Now save any contents that are a container back to the blueprint
        for (tile_id, contents) in &contents_from_tile_id {
            if let Contents::Container(container_id) = contents {
                if let Some(egui_tiles::Tile::Container(container)) = tree.tiles.get(*tile_id) {
                    let visible = tree.is_visible(*tile_id);

                    // TODO(jleibs): Make this only update the changed fields.
                    let blueprint = ContainerBlueprint::from_egui_tiles_container(
                        *container_id,
                        container,
                        visible,
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
            ctx.save_blueprint_component(&VIEWPORT_PATH.into(), &root_container);
        } else {
            ctx.save_empty_blueprint_component::<RootContainer>(&VIEWPORT_PATH.into());
        }
    }
}

fn build_tree_from_space_views_and_containers<'a>(
    space_views: impl Iterator<Item = &'a SpaceViewBlueprint>,
    containers: impl Iterator<Item = &'a ContainerBlueprint>,
    root_container: Option<ContainerId>,
) -> egui_tiles::Tree<SpaceViewId> {
    re_tracing::profile_function!();
    let mut tree = egui_tiles::Tree::empty("viewport_tree");

    // First add all the space_views
    for space_view in space_views {
        let tile_id = blueprint_id_to_tile_id(&space_view.id);
        let pane = egui_tiles::Tile::Pane(space_view.id);
        tree.tiles.insert(tile_id, pane);
        tree.set_visible(tile_id, space_view.visible);
    }

    // Now add all the containers
    for container in containers {
        let tile_id = blueprint_id_to_tile_id(&container.id);

        tree.tiles.insert(tile_id, container.to_tile());
        tree.set_visible(tile_id, container.visible);
    }

    // And finally, set the root
    if let Some(root_container) = root_container.map(|id| blueprint_id_to_tile_id(&id)) {
        tree.root = Some(root_container);
    }

    tree
}
