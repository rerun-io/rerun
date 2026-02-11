use std::collections::BTreeMap;
use std::ops::ControlFlow;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use ahash::HashMap;
use egui_tiles::{SimplificationOptions, TileId};
use nohash_hasher::IntSet;
use re_chunk_store::LatestAtQuery;
use re_entity_db::EntityPath;
use re_log_types::{EntityPathHash, EntityPathSubs};
use re_mutex::Mutex;
use re_sdk_types::blueprint::archetypes as blueprint_archetypes;
use re_sdk_types::blueprint::components::{
    AutoLayout, AutoViews, RootContainer, ViewMaximized, ViewerRecommendationHash,
};
use re_sdk_types::{Archetype as _, ViewClassIdentifier};
use re_viewer_context::{
    BlueprintContext as _, ContainerId, Contents, Item, ViewId, ViewerContext, VisitorControlFlow,
    blueprint_id_to_tile_id,
};
use smallvec::SmallVec;

use crate::container::ContainerBlueprint;
use crate::{VIEWPORT_PATH, ViewBlueprint, ViewportCommand};

// ----------------------------------------------------------------------------

/// Describes the layout and contents of the Viewport Panel.
///
/// This datastructure is loaded from the blueprint store at the start of each frame.
///
/// It remain immutable during the frame.
///
/// Any change is queued up into [`Self::deferred_commands`] and applied at the end of the frame,
/// right before saving to the blueprint store.
pub struct ViewportBlueprint {
    /// Where the views are stored.
    ///
    /// Not a hashmap in order to preserve the order of the views.
    pub views: BTreeMap<ViewId, ViewBlueprint>,

    /// All the containers found in the viewport.
    pub containers: BTreeMap<ContainerId, ContainerBlueprint>,

    /// The root container.
    pub root_container: ContainerId,

    /// The layouts of all the views.
    ///
    /// If [`Self::maximized`] is set, this tree is ignored.
    pub tree: egui_tiles::Tree<ViewId>,

    /// Show only one view as maximized?
    ///
    /// If set, [`Self::tree`] is ignored.
    pub maximized: Option<ViewId>,

    /// Whether the viewport layout is determined automatically.
    ///
    /// If `true`, we auto-layout all views whenever a new view is added.
    ///
    /// Set to `false` the first time the user messes around with the viewport blueprint.
    /// Note: we use an atomic here because writes needs to be effective immediately during the frame.
    auto_layout: AtomicBool,

    /// Whether views should be created automatically for entities that are not already in a space.
    ///
    /// Note: we use an atomic here because writes needs to be effective immediately during the frame.
    auto_views: AtomicBool,

    /// Hashes of all recommended views the viewer has already added and that should not be added again.
    past_viewer_recommendations: IntSet<ViewerRecommendationHash>,

    /// Blueprint mutation events that will be processed at the end of the frame.
    pub deferred_commands: Arc<Mutex<Vec<ViewportCommand>>>,
}

impl ViewportBlueprint {
    /// Load a [`ViewBlueprint`] from the blueprint store, or fall back to defaults.
    pub fn from_db(blueprint_db: &re_entity_db::EntityDb, query: &LatestAtQuery) -> Self {
        re_tracing::profile_function!();

        let blueprint_engine = blueprint_db.storage_engine();

        let results = blueprint_engine.cache().latest_at(
            query,
            &VIEWPORT_PATH.into(),
            blueprint_archetypes::ViewportBlueprint::all_component_identifiers(),
        );

        let root_container = results.component_mono::<RootContainer>(
            blueprint_archetypes::ViewportBlueprint::descriptor_root_container().component,
        );
        let maximized = results.component_mono::<ViewMaximized>(
            blueprint_archetypes::ViewportBlueprint::descriptor_maximized().component,
        );
        let auto_layout = results.component_mono::<AutoLayout>(
            blueprint_archetypes::ViewportBlueprint::descriptor_auto_layout().component,
        );
        let auto_views = results.component_mono::<AutoViews>(
            blueprint_archetypes::ViewportBlueprint::descriptor_auto_views().component,
        );
        let past_viewer_recommendations = results.component_batch::<ViewerRecommendationHash>(
            blueprint_archetypes::ViewportBlueprint::descriptor_past_viewer_recommendations()
                .component,
        );

        let root_container: Option<ContainerId> = root_container.map(|id| id.0.into());
        re_log::trace_once!("Loaded root_container: {root_container:?}");

        let mut containers: BTreeMap<ContainerId, ContainerBlueprint> = Default::default();
        let mut all_view_ids: Vec<ViewId> = Default::default();

        if let Some(root_container) = root_container {
            re_tracing::profile_scope!("visit_all_containers");
            let mut container_ids_to_visit: Vec<ContainerId> = vec![root_container];
            while let Some(id) = container_ids_to_visit.pop() {
                if let Some(container) = ContainerBlueprint::try_from_db(blueprint_db, query, id) {
                    re_log::trace_once!("Container {id} contents: {:?}", container.contents);
                    for &content in &container.contents {
                        match content {
                            Contents::Container(id) => container_ids_to_visit.push(id),
                            Contents::View(id) => {
                                all_view_ids.push(id);
                            }
                        }
                    }
                    containers.insert(id, container);
                } else {
                    re_log::warn_once!("Failed to load container {id}");
                }
            }
        }

        let views: BTreeMap<ViewId, ViewBlueprint> = all_view_ids
            .into_iter()
            .filter_map(|view: ViewId| ViewBlueprint::try_from_db(view, blueprint_db, query))
            .map(|sv| (sv.id, sv))
            .collect();

        // Auto layouting and auto view are only enabled if no blueprint has been provided by the user.
        // Only enable auto-views if this is the app-default blueprint
        let is_app_default_blueprint = blueprint_db
            .store_info()
            .is_some_and(|ri| ri.is_app_default_blueprint());
        let auto_layout =
            AtomicBool::new(auto_layout.map_or(is_app_default_blueprint, |auto| *auto.0));
        let auto_views =
            AtomicBool::new(auto_views.map_or(is_app_default_blueprint, |auto| *auto.0));

        let root_container = root_container.unwrap_or_else(|| {
            let new_root_id = ContainerId::hashed_from_str("placeholder_root_container");
            containers.insert(new_root_id, ContainerBlueprint::new(new_root_id));
            new_root_id
        });

        let tree = build_tree_from_views_and_containers(
            views.values(),
            containers.values(),
            root_container,
        );

        re_log::trace_once!("Loaded tree: {tree:#?}");

        let past_viewer_recommendations = past_viewer_recommendations
            .unwrap_or_default()
            .iter()
            .cloned()
            .collect();

        Self {
            views,
            containers,
            root_container,
            tree,
            maximized: maximized.map(|id| id.0.into()),
            auto_layout,
            auto_views,
            past_viewer_recommendations,
            deferred_commands: Default::default(),
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
        !self.views.is_empty()
            && self
                .views
                .values()
                .all(|sv| sv.class_identifier() == ViewClassIdentifier::invalid())
    }

    pub fn view_ids(&self) -> impl Iterator<Item = &ViewId> + '_ {
        self.views.keys()
    }

    /// Find the parent container of a given contents.
    ///
    /// Returns `None` if this is unknown contents, or if it is the root contaioner.
    pub fn parent(&self, needle: &Contents) -> Option<ContainerId> {
        self.containers
            .iter()
            .find_map(|(container_id, container)| {
                container.contents.contains(needle).then_some(*container_id)
            })
    }

    pub fn view(&self, view: &ViewId) -> Option<&ViewBlueprint> {
        self.views.get(view)
    }

    pub fn container(&self, container_id: &ContainerId) -> Option<&ContainerBlueprint> {
        self.containers.get(container_id)
    }

    /// Duplicates a view and its entity property overrides.
    pub fn duplicate_view(&self, view_id: &ViewId, ctx: &ViewerContext<'_>) -> Option<ViewId> {
        let view = self.view(view_id)?;

        let new_view = view.duplicate(ctx.store_context, ctx.blueprint_query);
        let new_view_id = new_view.id;

        let parent_and_pos = self.find_parent_and_position_index(&Contents::View(*view_id));

        self.add_views(
            std::iter::once(new_view),
            parent_and_pos.map(|(parent, _)| parent),
            parent_and_pos.map(|(_, pos)| pos),
        );

        self.mark_user_interaction(ctx);

        Some(new_view_id)
    }

    /// If `false`, the item is referring to data that is not present in this blueprint.
    ///
    /// TODO(#5742): note that `Item::DataResult` with entity path set to the space origin or some
    /// of its descendent are always considered valid.
    pub fn is_item_valid(
        &self,
        storage_ctx: &re_viewer_context::StorageContext<'_>,
        item: &Item,
    ) -> bool {
        match item {
            Item::AppId(app_id) => storage_ctx
                .hub
                .store_bundle()
                .entity_dbs()
                .any(|db| db.application_id() == app_id),

            Item::DataSource(_)
            | Item::TableId(_)
            | Item::StoreId(_)
            | Item::ComponentPath(_)
            | Item::InstancePath(_)
            | Item::RedapEntry(_)
            | Item::RedapServer(_) => true,

            Item::View(view_id) => self.view(view_id).is_some(),

            Item::DataResult(view_id, instance_path) => {
                self.view(view_id).is_some_and(|view| {
                    let entity_path = &instance_path.entity_path;

                    // TODO(#5742): including any path that is—or descend from—the space origin is
                    // necessary because such items may actually be displayed in the blueprint tree.
                    entity_path == &view.space_origin
                        || entity_path.is_descendant_of(&view.space_origin)
                        || view
                            .contents
                            .entity_path_filter()
                            .matches(&instance_path.entity_path)
                })
            }

            Item::Container(container_id) => self.container(container_id).is_some(),
        }
    }

    fn enqueue_command(&self, action: ViewportCommand) {
        self.deferred_commands.lock().push(action);
    }

    pub fn mark_user_interaction(&self, ctx: &ViewerContext<'_>) {
        if self.auto_layout() {
            re_log::trace!("User edits - will no longer auto-layout");
        }

        self.set_auto_layout(false, ctx);
        self.set_auto_views(false, ctx);
    }

    /// Spawns new views if enabled.
    pub fn spawn_heuristic_views(&self, ctx: &ViewerContext<'_>) {
        if !self.auto_views() {
            return;
        }

        re_tracing::profile_function!();

        for entry in ctx.view_class_registry().iter_registry() {
            let class_id = entry.identifier;

            let excluded_entities = re_log_types::ResolvedEntityPathFilter::properties();
            let include_entity = |ent: &EntityPath| !excluded_entities.matches(ent);

            let spawn_heuristics = entry.class.spawn_heuristics(ctx, &include_entity);
            let max_views_spawned = spawn_heuristics.max_views_spawned();
            let mut recommended_views = spawn_heuristics.into_vec();

            re_tracing::profile_scope!("filter_recommendations_for", class_id);

            // Count how many views of this class already exist.
            let existing_view_count = self
                .views
                .values()
                .filter(|view| view.class_identifier() == class_id)
                .count();

            // Limit recommendations based on max_views_spawned.
            // If we already have max or more views, don't spawn any more.
            let max_new_views = max_views_spawned.saturating_sub(existing_view_count);
            if max_new_views < recommended_views.len() {
                recommended_views.truncate(max_new_views);
            }

            // Remove all views that we already spawned via heuristic before.
            recommended_views.retain(|recommended_view| {
                !self
                    .past_viewer_recommendations
                    .contains(&recommended_view.recommendation_hash(class_id))
            });

            // Each of the remaining recommendations would individually be a candidate for spawning if there were
            // no other views in the viewport.
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
            // If now the user edits the view at `/**` to be `/points/**`, that does *not*
            // mean we should suddenly add `/camera/**` to the viewport.
            if !recommended_views.is_empty() {
                let new_viewer_recommendation_hashes: Vec<ViewerRecommendationHash> = self
                    .past_viewer_recommendations
                    .iter()
                    .cloned()
                    .chain(
                        recommended_views
                            .iter()
                            .map(|recommendation| recommendation.recommendation_hash(class_id)),
                    )
                    .collect();

                ctx.save_blueprint_component(
                    VIEWPORT_PATH.into(),
                    &blueprint_archetypes::ViewportBlueprint::descriptor_past_viewer_recommendations(),
                    &new_viewer_recommendation_hashes,
                );
            }

            // Resolve query filters for the recommended views.
            let mut recommended_views = recommended_views
                .into_iter()
                .map(|view| {
                    // Today this looks trivial and something like we could do during recommendation-creation. But in the future variable substitutions might become more complex!
                    let path_subs = EntityPathSubs::new_with_origin(&view.origin);
                    let query_filter = view.query_filter.resolve_forgiving(&path_subs);
                    (query_filter, view)
                })
                .collect::<Vec<_>>();

            // Remove all views that have all the entities we already have on screen.
            let existing_path_filters = self
                .views
                .values()
                .filter(|view| view.class_identifier() == class_id)
                .map(|view| view.contents.entity_path_filter())
                .collect::<Vec<_>>();
            recommended_views.retain(|(query_filter, _)| {
                existing_path_filters
                    .iter()
                    .all(|existing_filter| !existing_filter.is_superset_of(query_filter))
            });

            // Remove all views that are redundant within the remaining recommendation.
            // This n^2 loop should only run ever for frames that add new views.
            let final_recommendations = recommended_views
                .iter()
                .enumerate()
                .filter(|(j, (candidate_query_filter, _))| {
                    recommended_views
                        .iter()
                        .enumerate()
                        .all(|(i, (other_query_filter, _))| {
                            i == *j || !other_query_filter.is_superset_of(candidate_query_filter)
                        })
                })
                .map(|(_, recommendation)| recommendation);

            self.add_views(
                final_recommendations.map(|(_, recommendation)| {
                    ViewBlueprint::new(class_id, recommendation.clone())
                }),
                None,
                None,
            );
        }
    }

    /// Add a set of views to the viewport.
    ///
    /// The view is added to the root container, or, if provided, to a given parent container.
    ///
    /// Note that this doesn't focus the corresponding tab. Use [`Self::focus_tab`] with the returned ID
    /// if needed.
    pub fn add_views(
        &self,
        views: impl Iterator<Item = ViewBlueprint>,
        parent_container: Option<ContainerId>,
        position_in_parent: Option<usize>,
    ) {
        for view in views {
            self.enqueue_command(ViewportCommand::AddView {
                view,
                parent_container,
                position_in_parent,
            });
        }
    }

    /// Add a single view to the viewport to the root container.
    ///
    /// Returns the ID of the added view.
    pub fn add_view_at_root(&self, view: ViewBlueprint) -> ViewId {
        let view_id = view.id;
        self.add_views(std::iter::once(view), None, None);
        view_id
    }

    /// Returns an iterator over all the contents (views and containers) in the viewport.
    pub fn contents_iter(&self) -> impl Iterator<Item = Contents> + '_ {
        self.views
            .keys()
            .map(|view_id| Contents::View(*view_id))
            .chain(
                self.containers
                    .keys()
                    .map(|container_id| Contents::Container(*container_id)),
            )
    }

    /// Walk the entire [`Contents`] tree, starting from the root container.
    ///
    /// See [`VisitorControlFlow`] for details on traversal behavior.
    pub fn visit_contents<B>(
        &self,
        visitor: &mut impl FnMut(&Contents, &SmallVec<[ContainerId; 4]>) -> VisitorControlFlow<B>,
    ) -> ControlFlow<B> {
        self.visit_contents_in_container(&self.root_container, visitor)
    }

    /// Walk the subtree defined by the provided container id and call `visitor` for each
    /// [`Contents`].
    ///
    /// Note:
    /// - Returns as soon as `visitor` returns `false`.
    /// - `visitor` is first called for the container passed in argument
    /// - `visitor`'s second argument contains the hierarchy leading to the visited contents, from
    ///   (and including) the container passed in argument
    pub fn visit_contents_in_container<B>(
        &self,
        container_id: &ContainerId,
        visitor: &mut impl FnMut(&Contents, &SmallVec<[ContainerId; 4]>) -> VisitorControlFlow<B>,
    ) -> ControlFlow<B> {
        let mut hierarchy = SmallVec::new();
        self.visit_contents_impl(&Contents::Container(*container_id), &mut hierarchy, visitor)
    }

    fn visit_contents_impl<B>(
        &self,
        contents: &Contents,
        hierarchy: &mut SmallVec<[ContainerId; 4]>,
        visitor: &mut impl FnMut(&Contents, &SmallVec<[ContainerId; 4]>) -> VisitorControlFlow<B>,
    ) -> ControlFlow<B> {
        let visit_children = visitor(contents, hierarchy).visit_children()?;

        if visit_children {
            match contents {
                Contents::Container(container_id) => {
                    if let Some(container) = self.container(container_id) {
                        hierarchy.push(*container_id);
                        for contents in &container.contents {
                            self.visit_contents_impl(contents, hierarchy, visitor)?;
                        }
                        hierarchy.pop();
                    }
                }
                Contents::View(_) => {} // no children
            }
        }

        ControlFlow::Continue(())
    }

    /// Given a predicate, finds the (first) matching contents by recursively walking from the root
    /// container.
    pub fn find_contents_by(&self, predicate: &impl Fn(&Contents) -> bool) -> Option<Contents> {
        self.find_contents_in_container_by(predicate, &self.root_container)
    }

    /// Given a predicate, finds the (first) matching contents by recursively walking from the given
    /// container.
    pub fn find_contents_in_container_by(
        &self,
        predicate: &impl Fn(&Contents) -> bool,
        container_id: &ContainerId,
    ) -> Option<Contents> {
        let result = self.visit_contents_in_container(container_id, &mut |contents, _| {
            if predicate(contents) {
                VisitorControlFlow::Break(*contents)
            } else {
                VisitorControlFlow::Continue
            }
        });

        result.break_value()
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

    /// Given a container or a view, find its enclosing container and its position within it.
    pub fn find_parent_and_position_index(
        &self,
        contents: &Contents,
    ) -> Option<(ContainerId, usize)> {
        if *contents == Contents::Container(self.root_container) {
            // root doesn't have a parent
            return None;
        }
        self.find_parent_and_position_index_impl(contents, &self.root_container)
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
                Contents::View(_) => {}
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
        self.enqueue_command(ViewportCommand::AddContainer {
            container_kind: kind,
            parent_container,
        });
    }

    /// Recursively remove a container or a view.
    pub fn remove_contents(&self, contents: Contents) {
        self.enqueue_command(ViewportCommand::RemoveContents(contents));
    }

    /// Move the `contents` container or view to the specified target container and position.
    pub fn move_contents(
        &self,
        contents_to_move: Vec<Contents>,
        target_container: ContainerId,
        target_position_in_container: usize,
    ) {
        self.enqueue_command(ViewportCommand::MoveContents {
            contents_to_move,
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
        self.enqueue_command(ViewportCommand::MoveContentsToNewContainer {
            contents_to_move: contents,
            new_container_kind,
            target_container,
            target_position_in_container,
        });
    }

    /// Make sure the tab corresponding to this view is focused.
    pub fn focus_tab(&self, view_id: ViewId) {
        self.enqueue_command(ViewportCommand::FocusTab(view_id));
    }

    /// Set the kind of the provided container.
    pub fn set_container_kind(&self, container_id: ContainerId, kind: egui_tiles::ContainerKind) {
        // no-op check
        if let Some(container) = self.container(&container_id)
            && container.container_kind == kind
        {
            return;
        }

        self.enqueue_command(ViewportCommand::SetContainerKind(container_id, kind));
    }

    /// Simplify the container tree with the provided options.
    pub fn simplify_container(
        &self,
        container_id: &ContainerId,
        simplification_options: SimplificationOptions,
    ) {
        self.enqueue_command(ViewportCommand::SimplifyContainer(
            *container_id,
            simplification_options,
        ));
    }

    /// Make all children of the given container the same size.
    pub fn make_all_children_same_size(&self, container_id: &ContainerId) {
        self.enqueue_command(ViewportCommand::MakeAllChildrenSameSize(*container_id));
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
            Contents::View(view_id) => {
                if let Some(view) = self.view(view_id) {
                    view.visible
                } else {
                    re_log::warn_once!(
                        "Visibility check failed due to unknown view id {view_id:?}"
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
            Contents::View(view_id) => {
                if let Some(view) = self.view(view_id) {
                    if visible != view.visible {
                        if self.auto_layout() {
                            re_log::trace!("view visibility changed - will no longer auto-layout");
                        }

                        self.set_auto_layout(false, ctx);
                        view.set_visible(ctx, visible);
                    }
                } else {
                    re_log::warn_once!(
                        "Visibility change failed due to unknown view id {view_id:?}"
                    );
                }
            }
        }
    }

    pub fn views_containing_entity_path(
        &self,
        ctx: &ViewerContext<'_>,
        path: EntityPathHash,
    ) -> Vec<ViewId> {
        self.views
            .iter()
            .filter_map(|(view_id, view)| {
                let query_result = ctx.lookup_query_result(view.id);
                if query_result.tree.lookup_result_by_path(path).is_some() {
                    Some(*view_id)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Whether the viewport layout is determined automatically.
    ///
    /// If `true`, we auto-layout all views whenever a new view is added.
    ///
    /// Set to `false` the first time the user messes around with the viewport blueprint.
    #[inline]
    pub fn auto_layout(&self) -> bool {
        self.auto_layout.load(Ordering::SeqCst)
    }

    /// Whether the viewport layout is determined automatically.
    ///
    /// If `true`, we auto-layout all views whenever a new view is added.
    ///
    /// Set to `false` the first time the user messes around with the viewport blueprint.
    #[inline]
    pub fn set_auto_layout(&self, value: bool, ctx: &ViewerContext<'_>) {
        let old_value = self.auto_layout.swap(value, Ordering::SeqCst);

        if old_value != value {
            let auto_layout = AutoLayout::from(value);
            ctx.save_blueprint_component(
                VIEWPORT_PATH.into(),
                &blueprint_archetypes::ViewportBlueprint::descriptor_auto_layout(),
                &auto_layout,
            );
        }
    }

    /// Whether views should be created automatically for entities that are not already in a space.
    #[inline]
    pub fn auto_views(&self) -> bool {
        self.auto_views.load(Ordering::SeqCst)
    }

    /// Whether views should be created automatically for entities that are not already in a space.
    #[inline]
    pub fn set_auto_views(&self, value: bool, ctx: &ViewerContext<'_>) {
        let old_value = self.auto_views.swap(value, Ordering::SeqCst);

        if old_value != value {
            let auto_views = AutoViews::from(value);
            ctx.save_blueprint_component(
                VIEWPORT_PATH.into(),
                &blueprint_archetypes::ViewportBlueprint::descriptor_auto_views(),
                &auto_views,
            );
        }
    }

    #[inline]
    pub fn set_maximized(&self, view_id: Option<ViewId>, ctx: &ViewerContext<'_>) {
        if self.maximized != view_id {
            let view_maximized = view_id.map(|id| ViewMaximized(id.into()));
            ctx.save_blueprint_component(
                VIEWPORT_PATH.into(),
                &blueprint_archetypes::ViewportBlueprint::descriptor_maximized(),
                &view_maximized,
            );
        }
    }

    /// Save the current state of the viewport to the blueprint store.
    /// This should only be called if the tree was edited.
    pub fn save_tree_as_containers(&self, ctx: &ViewerContext<'_>) {
        re_tracing::profile_function!();

        re_log::trace!("Saving tree: {:#?}", self.tree);

        // First, update the mapping for all the previously known containers.
        // These were inserted with their ids, so we want to keep these
        // constant if we find them again.
        let mut contents_from_tile_id: HashMap<TileId, Contents> = self
            .containers
            .keys()
            .map(|id| (blueprint_id_to_tile_id(id), Contents::Container(*id)))
            .collect();

        // Now, update the content mapping for all the new tiles in the tree.
        for (tile_id, tile) in self.tree.tiles.iter() {
            // If we already know about this tile, then we don't need
            // to do anything.
            if contents_from_tile_id.contains_key(tile_id) {
                continue;
            }
            match tile {
                egui_tiles::Tile::Pane(view_id) => {
                    // If a container has a pointer to a view
                    // we want it to point at the view in the blueprint.
                    contents_from_tile_id.insert(*tile_id, Contents::View(*view_id));
                }
                egui_tiles::Tile::Container(container) => {
                    if self.tree.root != Some(*tile_id)
                        && container.kind() == egui_tiles::ContainerKind::Tabs
                        && container.num_children() == 1
                    {
                        // If this is a tab-container with a single child, then it might be a
                        // "Trivial Tab", which egui_tiles adds to all views during simplification
                        // but doesn't need to be persisted back to the store.
                        if let Some(egui_tiles::Tile::Pane(view_id)) = container
                            .children()
                            .next()
                            .and_then(|child| self.tree.tiles.get(*child))
                        {
                            // This is a trivial tab -- this tile can point directly to
                            // the View and not to a Container.
                            contents_from_tile_id.insert(*tile_id, Contents::View(*view_id));
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

        // Clear any existing container blueprints that aren't referenced by any tiles,
        // allowing the GC to remove the previous (non-clear) data from the store (saving RAM).
        for (container_id, container) in &self.containers {
            let tile_id = blueprint_id_to_tile_id(container_id);
            if self.tree.tiles.get(tile_id).is_none() {
                container.clear(ctx);
            }
        }

        // Now save any contents that are a container back to the blueprint
        for (tile_id, contents) in &contents_from_tile_id {
            if let Contents::Container(container_id) = contents
                && let Some(egui_tiles::Tile::Container(container)) = self.tree.tiles.get(*tile_id)
            {
                let visible = self.tree.is_visible(*tile_id);

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

        // Finally update the root
        if let Some(root_container) = self
            .tree
            .root()
            .and_then(|root| contents_from_tile_id.get(&root))
            .and_then(|contents| contents.as_container_id())
            .map(|container_id| RootContainer((container_id).into()))
        {
            re_log::trace!("Saving with a root container");
            ctx.save_blueprint_component(
                VIEWPORT_PATH.into(),
                &blueprint_archetypes::ViewportBlueprint::descriptor_root_container(),
                &root_container,
            );
        } else {
            re_log::trace!("Saving empty viewport");
            ctx.clear_blueprint_component(
                VIEWPORT_PATH.into(),
                blueprint_archetypes::ViewportBlueprint::descriptor_root_container(),
            );
        }
    }

    /// Process any deferred [`ViewportCommand`] and then save to blueprint store (if needed).
    pub fn save_to_blueprint_store(mut self, ctx: &ViewerContext<'_>) {
        re_tracing::profile_function!();

        let commands: Vec<ViewportCommand> = self.deferred_commands.lock().drain(..).collect();

        if commands.is_empty() {
            return; // No changes this frame - no need to save to blueprint store.
        }

        let mut run_auto_layout = false;

        for command in commands {
            apply_viewport_command(ctx, &mut self, command, &mut run_auto_layout);
        }

        if run_auto_layout {
            self.tree = super::auto_layout::tree_from_views(ctx.view_class_registry(), &self.views);
        }

        // Simplify before we save the tree.
        // `egui_tiles` also runs a simplifying pass when calling `tree.ui`, but that is too late.
        // We want the simplified changes saved to the store:
        self.tree.simplify(&tree_simplification_options());

        // TODO(emilk): consider diffing the tree against the state it was in at the start of the frame,
        // so that we only save it if it actually changed.

        self.save_tree_as_containers(ctx);
    }
}

pub fn tree_simplification_options() -> egui_tiles::SimplificationOptions {
    egui_tiles::SimplificationOptions {
        prune_empty_tabs: false,
        all_panes_must_have_tabs: true,
        prune_empty_containers: false,
        prune_single_child_tabs: false,
        prune_single_child_containers: false,
        join_nested_linear_containers: true,
    }
}

fn apply_viewport_command(
    ctx: &ViewerContext<'_>,
    bp: &mut ViewportBlueprint,
    command: ViewportCommand,
    run_auto_layout: &mut bool,
) {
    re_log::trace!("Processing viewport command: {command:?}");
    match command {
        ViewportCommand::SetTree(new_tree) => {
            bp.tree = new_tree;
        }

        ViewportCommand::AddView {
            view,
            parent_container,
            position_in_parent,
        } => {
            let view_id = view.id;

            view.save_to_blueprint_store(ctx);
            bp.views.insert(view_id, view);

            if bp.auto_layout() {
                // No need to add to the tree - we'll create a new tree from scratch instead.
                re_log::trace!(
                    "Running auto-layout after adding a view because auto_layout is turned on"
                );
                *run_auto_layout = true;
            } else {
                // Add the view to the tree:
                let parent_id = parent_container.unwrap_or(bp.root_container);
                re_log::trace!("Adding view {view_id} to parent {parent_id}");
                let tile_id = bp.tree.tiles.insert_pane(view_id);
                let container_tile_id = blueprint_id_to_tile_id(&parent_id);
                if let Some(egui_tiles::Tile::Container(container)) =
                    bp.tree.tiles.get_mut(container_tile_id)
                {
                    re_log::trace!("Inserting new view into root container");
                    container.add_child(tile_id);
                    if let Some(position_in_parent) = position_in_parent {
                        bp.tree.move_tile_to_container(
                            tile_id,
                            container_tile_id,
                            position_in_parent,
                            true,
                        );
                    }
                } else {
                    re_log::trace!(
                        "Parent was not a container (or not found) - will re-run auto-layout"
                    );
                    *run_auto_layout = true;
                }
            }
        }

        ViewportCommand::AddContainer {
            container_kind,
            parent_container,
        } => {
            let parent_id = parent_container.unwrap_or(bp.root_container);

            let tile_id = bp
                .tree
                .tiles
                .insert_container(egui_tiles::Container::new(container_kind, vec![]));

            re_log::trace!("Adding container {container_kind:?} to parent {parent_id}");

            if let Some(egui_tiles::Tile::Container(parent_container)) =
                bp.tree.tiles.get_mut(blueprint_id_to_tile_id(&parent_id))
            {
                re_log::trace!("Inserting new view into container {parent_id:?}");
                parent_container.add_child(tile_id);
            } else {
                re_log::trace!("Parent or root was not a container - will re-run auto-layout");
                *run_auto_layout = true;
            }
        }

        ViewportCommand::SetContainerKind(container_id, container_kind) => {
            if let Some(egui_tiles::Tile::Container(container)) = bp
                .tree
                .tiles
                .get_mut(blueprint_id_to_tile_id(&container_id))
            {
                re_log::trace!("Mutating container {container_id:?} to {container_kind:?}");
                container.set_kind(container_kind);
            } else {
                re_log::trace!("No root found - will re-run auto-layout");
            }
        }

        ViewportCommand::FocusTab(view_id) => {
            let found = bp.tree.make_active(|_, tile| match tile {
                egui_tiles::Tile::Pane(this_view_id) => *this_view_id == view_id,
                egui_tiles::Tile::Container(_) => false,
            });
            re_log::trace!("Found tab to focus on for view ID {view_id}: {found}");
        }

        ViewportCommand::RemoveContents(contents) => {
            let tile_id = contents.as_tile_id();

            for tile in bp.tree.remove_recursively(tile_id) {
                re_log::trace!("Removing tile {tile_id:?}");
                match tile {
                    egui_tiles::Tile::Pane(view_id) => {
                        re_log::trace!("Removing view {view_id}");

                        // Remove the view from the store
                        if let Some(view) = bp.views.get(&view_id) {
                            view.clear(ctx);
                        }

                        // If the view was maximized, clean it up
                        if bp.maximized == Some(view_id) {
                            bp.set_maximized(None, ctx);
                        }

                        bp.views.remove(&view_id);
                    }
                    egui_tiles::Tile::Container(_) => {
                        // Empty containers (like this one) will be auto-removed by the tree simplification algorithm,
                        // that will run later because of this tree edit.
                    }
                }
            }

            bp.mark_user_interaction(ctx);

            if Some(tile_id) == bp.tree.root {
                bp.tree.root = None;
            }
        }

        ViewportCommand::SimplifyContainer(container_id, options) => {
            re_log::trace!("Simplifying tree with options: {options:?}");
            let tile_id = blueprint_id_to_tile_id(&container_id);
            bp.tree.simplify_children_of_tile(tile_id, &options);
        }

        ViewportCommand::MakeAllChildrenSameSize(container_id) => {
            let tile_id = blueprint_id_to_tile_id(&container_id);
            if let Some(egui_tiles::Tile::Container(container)) = bp.tree.tiles.get_mut(tile_id) {
                match container {
                    egui_tiles::Container::Tabs(_) => {}
                    egui_tiles::Container::Linear(linear) => {
                        linear.shares = Default::default();
                    }
                    egui_tiles::Container::Grid(grid) => {
                        grid.col_shares = Default::default();
                        grid.row_shares = Default::default();
                    }
                }
            }
        }

        ViewportCommand::MoveContents {
            contents_to_move,
            target_container,
            target_position_in_container,
        } => {
            re_log::trace!(
                "Moving {contents_to_move:?} to container {target_container:?} at pos \
                        {target_position_in_container}"
            );

            // TODO(ab): the `rev()` is better preserve ordering when moving a group of items. There
            // remains some ordering (and possibly insertion point error) edge cases when dragging
            // multiple item within the same container. This should be addressed by egui_tiles:
            // https://github.com/rerun-io/egui_tiles/issues/90
            for contents in contents_to_move.iter().rev() {
                let contents_tile_id = contents.as_tile_id();
                let target_container_tile_id = blueprint_id_to_tile_id(&target_container);

                bp.tree.move_tile_to_container(
                    contents_tile_id,
                    target_container_tile_id,
                    target_position_in_container,
                    true,
                );
            }
        }

        ViewportCommand::MoveContentsToNewContainer {
            contents_to_move,
            new_container_kind,
            target_container,
            target_position_in_container,
        } => {
            let new_container_tile_id = bp
                .tree
                .tiles
                .insert_container(egui_tiles::Container::new(new_container_kind, vec![]));

            let target_container_tile_id = blueprint_id_to_tile_id(&target_container);
            bp.tree.move_tile_to_container(
                new_container_tile_id,
                target_container_tile_id,
                target_position_in_container,
                true, // reflow grid if needed
            );

            for (pos, content) in contents_to_move.into_iter().enumerate() {
                bp.tree.move_tile_to_container(
                    content.as_tile_id(),
                    new_container_tile_id,
                    pos,
                    true, // reflow grid if needed
                );
            }
        }
    }
}

fn build_tree_from_views_and_containers<'a>(
    views: impl Iterator<Item = &'a ViewBlueprint>,
    containers: impl Iterator<Item = &'a ContainerBlueprint>,
    root_container: ContainerId,
) -> egui_tiles::Tree<ViewId> {
    re_tracing::profile_function!();
    let mut tree = egui_tiles::Tree::empty("viewport_tree");

    // First add all the views
    for view in views {
        let tile_id = blueprint_id_to_tile_id(&view.id);
        let pane = egui_tiles::Tile::Pane(view.id);
        tree.tiles.insert(tile_id, pane);
        tree.set_visible(tile_id, view.visible);
    }

    // Now add all the containers
    for container in containers {
        let tile_id = blueprint_id_to_tile_id(&container.id);

        tree.tiles.insert(tile_id, container.to_tile());
        tree.set_visible(tile_id, container.visible);
    }

    // And finally, set the root

    tree.root = Some(blueprint_id_to_tile_id(&root_container));

    tree
}
