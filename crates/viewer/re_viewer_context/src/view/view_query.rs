use std::collections::BTreeMap;

use itertools::Itertools as _;
use nohash_hasher::IntMap;
use re_chunk::TimelineName;
use smallvec::SmallVec;

use re_chunk_store::LatestAtQuery;
use re_entity_db::{EntityPath, TimeInt};
use re_log_types::StoreKind;
use re_types::{
    blueprint::archetypes::{self as blueprint_archetypes, EntityBehavior},
    components, ComponentDescriptor,
};

use crate::{
    DataResultTree, QueryRange, ViewHighlights, ViewId, ViewSystemIdentifier, ViewerContext,
};

/// Path to a specific entity in a specific store used for overrides.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct OverridePath {
    // NOTE: StoreKind is easier to work with than a `StoreId`` or full `ChunkStore` but
    // might still be ambiguous when we have multiple stores active at a time.
    pub store_kind: StoreKind,
    pub path: EntityPath,
}

impl OverridePath {
    pub fn blueprint_path(path: EntityPath) -> Self {
        Self {
            store_kind: StoreKind::Blueprint,
            path,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PropertyOverrides {
    /// An alternative store and entity path to use for the specified component.
    ///
    /// Note that this does *not* take into account tree propagation of any special components
    /// like `Visible`, `Interactive` or transform components.
    // TODO(jleibs): Consider something like `tinymap` for this.
    // TODO(andreas): Should be a `Cow` to not do as many clones.
    pub component_overrides: IntMap<ComponentDescriptor, OverridePath>,

    /// Whether the entity is visible.
    ///
    /// This is propagated through the entity tree.
    pub visible: bool,

    /// Whether the entity is interactive.
    ///
    /// This is propagated through the entity tree.
    pub interactive: bool,

    /// `EntityPath` in the Blueprint store where updated overrides should be written back
    /// for properties that apply to the individual entity only.
    pub override_path: EntityPath,

    /// What range is queried on the chunk store.
    ///
    /// This is sourced either from an override or via the `View`'s query range.
    pub query_range: QueryRange,
}

pub type SmallVisualizerSet = SmallVec<[ViewSystemIdentifier; 4]>;

/// This is the primary mechanism through which data is passed to a `View`.
///
/// It contains everything necessary to properly use this data in the context of the
/// `ViewSystem`s that it is a part of.
#[derive(Clone, Debug)]
pub struct DataResult {
    /// Where to retrieve the data from.
    // TODO(jleibs): This should eventually become a more generalized (StoreView + EntityPath) reference to handle
    // multi-RRD or blueprint-static data references.
    pub entity_path: EntityPath,

    /// Which `ViewSystems`s to pass the `DataResult` to.
    pub visualizers: SmallVisualizerSet,

    /// If true, this path is not actually included in the query results and is just here
    /// because of a common prefix.
    ///
    /// If this is true, `visualizers` must be empty.
    pub tree_prefix_only: bool,

    /// The accumulated property overrides for this `DataResult`.
    pub property_overrides: PropertyOverrides,
}

impl DataResult {
    #[inline]
    pub fn override_path(&self) -> &EntityPath {
        &self.property_overrides.override_path
    }

    /// Overrides the `visible` behavior such that the given value becomes set next frame.
    ///
    /// If no override is set, this will always set the override.
    /// If an override is set, this will either write an override or clear it if the parent has the desired value already.
    ///
    /// In either case, this will be effective only by the next frame.
    pub fn save_visible(
        &self,
        ctx: &ViewerContext<'_>,
        data_result_tree: &DataResultTree,
        new_value: bool,
    ) {
        // Check if we should instead clear an existing override.
        if self.has_override(ctx, &EntityBehavior::descriptor_visible()) {
            let parent_visibility = self
                .entity_path
                .parent()
                .and_then(|parent_path| data_result_tree.lookup_result_by_path(&parent_path))
                .is_none_or(|data_result| data_result.is_visible());

            if parent_visibility == new_value {
                // TODO(andreas): blueprint save_empty should know about tags (`EntityBehavior::visible`'s tag)
                ctx.save_empty_blueprint_component::<components::Visible>(
                    &self.property_overrides.override_path,
                );
                return;
            }
        }

        ctx.save_blueprint_archetype(
            &self.property_overrides.override_path,
            &blueprint_archetypes::EntityBehavior::update_fields().with_visible(new_value),
        );
    }

    /// Overrides the `interactive` behavior such that the given value becomes set next frame.
    ///
    /// If no override is set, this will always set the override.
    /// If an override is set, this will either write an override or clear it if the parent has the desired value already.
    ///
    /// In either case, this will be effective only by the next frame.
    pub fn save_interactive(
        &self,
        ctx: &ViewerContext<'_>,
        data_result_tree: &DataResultTree,
        new_value: bool,
    ) {
        // Check if we should instead clear an existing override.
        if self.has_override(ctx, &EntityBehavior::descriptor_interactive()) {
            let parent_interactivity = self
                .entity_path
                .parent()
                .and_then(|parent_path| data_result_tree.lookup_result_by_path(&parent_path))
                .is_none_or(|data_result| data_result.is_interactive());

            if parent_interactivity == new_value {
                // TODO(#6889): tagged empty component.
                ctx.save_empty_blueprint_component::<components::Interactive>(
                    &self.property_overrides.override_path,
                );
                return;
            }
        }

        ctx.save_blueprint_archetype(
            &self.property_overrides.override_path,
            &blueprint_archetypes::EntityBehavior::update_fields().with_interactive(new_value),
        );
    }

    fn has_override(&self, ctx: &ViewerContext<'_>, component_descr: &ComponentDescriptor) -> bool {
        self.property_overrides
            .component_overrides
            .get(component_descr)
            .is_some_and(|OverridePath { store_kind, path }| {
                match store_kind {
                    StoreKind::Blueprint => ctx.store_context.blueprint.latest_at(
                        ctx.blueprint_query,
                        path,
                        [component_descr],
                    ),
                    StoreKind::Recording => {
                        ctx.recording()
                            .latest_at(&ctx.current_query(), path, [component_descr])
                    }
                }
                .get(component_descr)
                .is_some()
            })
    }

    /// Shorthand for checking for visibility on data overrides.
    ///
    /// Note that this won't check if the datastore store has visibility logged.
    // TODO(#6541): Check the datastore.
    #[inline]
    pub fn is_visible(&self) -> bool {
        self.property_overrides.visible
    }

    /// Shorthand for checking for interactivity on data overrides.
    ///
    /// Note that this won't check if the datastore store has interactivity logged.
    // TODO(#6541): Check the datastore.
    #[inline]
    pub fn is_interactive(&self) -> bool {
        self.property_overrides.interactive
    }

    /// Returns the query range for this data result.
    pub fn query_range(&self) -> &QueryRange {
        &self.property_overrides.query_range
    }
}

pub type PerSystemDataResults<'a> = BTreeMap<ViewSystemIdentifier, Vec<&'a DataResult>>;

#[derive(Debug)]
pub struct ViewQuery<'s> {
    /// The id of the space in which context the query happens.
    pub view_id: ViewId,

    /// The root of the space in which context the query happens.
    pub space_origin: &'s EntityPath,

    /// All [`DataResult`]s that are queried by active visualizers.
    ///
    /// Contains also invisible objects, use `iter_visible_data_results` to iterate over visible ones.
    pub per_visualizer_data_results: PerSystemDataResults<'s>,

    /// The timeline we're on.
    pub timeline: TimelineName,

    /// The time on the timeline we're currently at.
    pub latest_at: TimeInt,

    /// Hover/select highlighting information for this view.
    ///
    /// TODO(andreas): This should be the result of a [`crate::ViewContextSystem`] instead?
    pub highlights: ViewHighlights,
}

impl<'s> ViewQuery<'s> {
    /// Iter over all of the currently visible [`DataResult`]s for a given `ViewSystem`
    pub fn iter_visible_data_results<'a>(
        &'a self,
        visualizer: ViewSystemIdentifier,
    ) -> impl Iterator<Item = &'a DataResult>
    where
        's: 'a,
    {
        self.per_visualizer_data_results.get(&visualizer).map_or(
            itertools::Either::Left(std::iter::empty()),
            |results| {
                itertools::Either::Right(
                    results.iter().filter(|result| result.is_visible()).copied(),
                )
            },
        )
    }

    /// Iterates over all [`DataResult`]s of the [`ViewQuery`].
    #[inline]
    pub fn iter_all_data_results(&self) -> impl Iterator<Item = &DataResult> + '_ {
        self.per_visualizer_data_results
            .values()
            .flat_map(|data_results| data_results.iter().copied())
    }

    /// Iterates over all entities of the [`ViewQuery`].
    #[inline]
    pub fn iter_all_entities(&self) -> impl Iterator<Item = &EntityPath> + '_ {
        self.iter_all_data_results()
            .map(|data_result| &data_result.entity_path)
            .unique()
    }

    #[inline]
    pub fn latest_at_query(&self) -> LatestAtQuery {
        LatestAtQuery::new(self.timeline, self.latest_at)
    }
}
