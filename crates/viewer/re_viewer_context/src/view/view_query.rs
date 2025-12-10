use std::collections::BTreeMap;

use itertools::Itertools as _;
use nohash_hasher::IntMap;
use re_chunk::{ComponentIdentifier, TimelineName};
use re_chunk_store::LatestAtQuery;
use re_entity_db::{EntityPath, TimeInt};
use re_log_types::StoreKind;
use re_sdk_types::blueprint::archetypes::{self as blueprint_archetypes, EntityBehavior};
use smallvec::SmallVec;

use crate::blueprint_helpers::BlueprintContext as _;
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
    pub component_overrides: IntMap<ComponentIdentifier, OverridePath>,
}

pub type SmallVisualizerSet = SmallVec<[ViewSystemIdentifier; 4]>;

#[derive(Clone, Debug)]
pub struct VisualizerInstruction {
    pub visualizer_type: ViewSystemIdentifier,
    pub property_overrides: PropertyOverrides,
    // visualizer_id: String,
}

impl VisualizerInstruction {
    /// The placeholder visualizer instruction implies to queries that they shouldn't query overrides from any specific visualizer id,
    /// but rather from the "general" blueprint overrides for the entity.
    /// This is used for special properties like `EntityBehavior`, `CoordinateFrame` and other "overrides" that don't affect any concrete visualizer.
    pub fn placeholder() -> Self {
        Self {
            visualizer_type: "___PLACEHOLDER___".into(),
            property_overrides: PropertyOverrides {
                component_overrides: IntMap::default(),
            },
        }
    }
}

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
    // pub visualizers: SmallVisualizerSet,
    pub visualizer_instructions: SmallVec<[VisualizerInstruction; 1]>,

    /// If true, this path is not actually included in the query results and is just here
    /// because of a common prefix.
    ///
    /// If this is true, `visualizers` must be empty.
    pub tree_prefix_only: bool,

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

impl DataResult {
    /// The override path for this data result.
    ///
    /// This is **not** the override path for a concrete visualizer instruction yet.
    #[inline]
    pub fn override_path(&self) -> &EntityPath {
        &self.override_path
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
        if self.has_override(ctx, EntityBehavior::descriptor_visible().component) {
            let parent_visibility = self
                .entity_path
                .parent()
                .and_then(|parent_path| data_result_tree.lookup_result_by_path(parent_path.hash()))
                .is_none_or(|data_result| data_result.is_visible());

            if parent_visibility == new_value {
                ctx.clear_blueprint_component(
                    self.override_path.clone(),
                    EntityBehavior::descriptor_visible(),
                );
                return;
            }
        }

        ctx.save_blueprint_archetype(
            self.override_path.clone(),
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
        if self.has_override(ctx, EntityBehavior::descriptor_interactive().component) {
            let parent_interactivity = self
                .entity_path
                .parent()
                .and_then(|parent_path| data_result_tree.lookup_result_by_path(parent_path.hash()))
                .is_none_or(|data_result| data_result.is_interactive());

            if parent_interactivity == new_value {
                ctx.clear_blueprint_component(
                    self.override_path.clone(),
                    EntityBehavior::descriptor_interactive(),
                );
                return;
            }
        }

        ctx.save_blueprint_archetype(
            self.override_path.clone(),
            &blueprint_archetypes::EntityBehavior::update_fields().with_interactive(new_value),
        );
    }

    // TODO: only checks first visualizer instruction.
    fn has_override(&self, ctx: &ViewerContext<'_>, component: ComponentIdentifier) -> bool {
        self.visualizer_instructions
            .first()
            .is_some_and(|instruction| {
                instruction
                    .property_overrides
                    .component_overrides
                    .get(&component)
                    .is_some_and(|OverridePath { store_kind, path }| {
                        match store_kind {
                            StoreKind::Blueprint => ctx.store_context.blueprint.latest_at(
                                ctx.blueprint_query,
                                path,
                                [component],
                            ),
                            StoreKind::Recording => {
                                ctx.recording()
                                    .latest_at(&ctx.current_query(), path, [component])
                            }
                        }
                        .get(component)
                        .is_some()
                    })
            })
    }

    /// Shorthand for checking for visibility on data overrides.
    ///
    /// Note that this won't check if the datastore store has visibility logged.
    // TODO(#6541): Check the datastore.
    #[inline]
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Shorthand for checking for interactivity on data overrides.
    ///
    /// Note that this won't check if the datastore store has interactivity logged.
    // TODO(#6541): Check the datastore.
    #[inline]
    pub fn is_interactive(&self) -> bool {
        self.interactive
    }

    /// Returns the query range for this data result.
    pub fn query_range(&self) -> &QueryRange {
        &self.query_range
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
    // TODO: this seems weird now, because within a dataresult we STILL have to iterate & filter for visualizers
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
    pub fn iter_visualizer_instruction_for<'a>(
        &'a self,
        visualizer: ViewSystemIdentifier,
    ) -> impl Iterator<Item = (&'a DataResult, &'a VisualizerInstruction)> + 'a
    where
        's: 'a,
    {
        self.per_visualizer_data_results.get(&visualizer).map_or(
            itertools::Either::Left(std::iter::empty()),
            |results| {
                itertools::Either::Right(
                    results
                        .iter()
                        .filter(|result| result.is_visible())
                        .flat_map(move |result| {
                            result
                                .visualizer_instructions
                                .iter()
                                .filter_map(move |instruction| {
                                    (instruction.visualizer_type == visualizer)
                                        .then_some((*result, instruction))
                                })
                        }),
                )
            },
        )
    }

    /// Iterates over all currently visible (i.e. at least one visualizer is active) [`DataResult`]s of the [`ViewQuery`].
    #[inline]
    pub fn iter_all_data_results(&self) -> impl Iterator<Item = &DataResult> + '_ {
        self.per_visualizer_data_results
            .values()
            .flat_map(|data_results| data_results.iter().copied())
            .unique_by(|data_result| data_result.entity_path.hash())
    }

    /// Iterates over all currently visible (i.e. at least one visualizer is active) entities of the [`ViewQuery`].
    #[inline]
    pub fn iter_all_entities(&self) -> impl Iterator<Item = &EntityPath> + '_ {
        self.iter_all_data_results()
            .map(|data_result| &data_result.entity_path)
    }

    #[inline]
    pub fn latest_at_query(&self) -> LatestAtQuery {
        LatestAtQuery::new(self.timeline, self.latest_at)
    }
}
