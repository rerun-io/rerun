use std::collections::BTreeMap;

use itertools::Itertools as _;
use nohash_hasher::IntSet;
use re_chunk::{ComponentIdentifier, TimelineName};
use re_chunk_store::LatestAtQuery;
use re_entity_db::{EntityPath, TimeInt};
use re_sdk_types::blueprint::archetypes::{self as blueprint_archetypes, EntityBehavior};
use smallvec::SmallVec;

use crate::blueprint_helpers::BlueprintContext as _;
use crate::{
    DataResultTree, QueryRange, ViewHighlights, ViewId, ViewSystemIdentifier, ViewerContext,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VisualizerInstructionId(pub uuid::Uuid);

impl VisualizerInstructionId {
    pub fn invalid() -> Self {
        Self(uuid::Uuid::nil())
    }

    pub fn new_random() -> Self {
        Self(uuid::Uuid::new_v4())
    }

    pub fn new_deterministic(entity_path: &EntityPath, index: usize) -> Self {
        Self(uuid::Uuid::from_u64_pair(
            entity_path.hash64(),
            index as u64,
        ))
    }

    pub fn from_uuid(uuid: uuid::Uuid) -> Self {
        Self(uuid)
    }
}

/// A single component mapping for a visualizer instruction.
#[derive(Clone, Debug, Hash)]
pub struct VisualizerComponentMapping {
    pub selector: ComponentIdentifier,
    pub target: ComponentIdentifier,
}

/// A list of component mappings for a visualizer instruction.
pub type VisualizerComponentMappings = SmallVec<[VisualizerComponentMapping; 2]>;

#[derive(Clone, Debug)]
pub struct VisualizerInstruction {
    pub id: VisualizerInstructionId, // TODO(andreas,aedm): rly a string?

    pub visualizer_type: ViewSystemIdentifier,

    // TODO(aedm): only generate this path if the instruction has to be saved to the blueprint store.
    pub override_path: EntityPath,

    /// List of components that have overrides for this visualizer instruction.
    ///
    /// Note that this does *not* take into account tree propagation of any special components
    /// like `Visible`, `Interactive` or transform components.
    pub component_overrides: IntSet<ComponentIdentifier>,

    /// List of component mapping pairs.
    pub component_mappings: VisualizerComponentMappings,
}

impl VisualizerInstruction {
    pub fn new(
        id: VisualizerInstructionId,
        visualizer_type: ViewSystemIdentifier,
        override_base_path: &EntityPath,
        component_mappings: VisualizerComponentMappings,
    ) -> Self {
        Self {
            override_path: Self::override_path_for(override_base_path, &id),
            id,
            visualizer_type,
            component_overrides: IntSet::default(),
            component_mappings,
        }
    }

    pub fn override_path_for(
        override_base_path: &EntityPath,
        id: &VisualizerInstructionId,
    ) -> EntityPath {
        override_base_path.join(&EntityPath::from_single_string(id.0.to_string()))
    }

    /// The placeholder visualizer instruction implies to queries that they shouldn't query overrides from any specific visualizer id,
    /// but rather from the "general" blueprint overrides for the entity.
    /// This is used for special properties like `EntityBehavior`, `CoordinateFrame` and other "overrides" that don't affect any concrete visualizer.
    pub fn placeholder(data_result: &DataResult) -> Self {
        Self {
            id: VisualizerInstructionId::invalid(),
            visualizer_type: "___PLACEHOLDER___".into(),
            component_overrides: IntSet::default(),
            override_path: data_result.override_base_path.clone(), // TODO(aedm): create a clearly invalid one.
            component_mappings: VisualizerComponentMappings::default(),
        }
    }

    pub fn write_instruction_to_blueprint(&self, ctx: &ViewerContext<'_>) {
        let component_mappings = self.component_mappings.iter().map(|mapping| {
            re_sdk_types::blueprint::datatypes::VisualizerComponentMapping {
                selector: mapping.selector.as_str().into(),
                target: mapping.target.as_str().into(),
            }
        });
        let new_visualizer_instruction =
            re_sdk_types::blueprint::archetypes::VisualizerInstruction::new(
                self.visualizer_type.as_str(),
                component_mappings,
            );
        ctx.save_blueprint_archetype(self.override_path.clone(), &new_visualizer_instruction);
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

    /// There are any visualizers that can run on this data result.
    pub any_visualizers_available: bool,

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
    pub override_base_path: EntityPath,

    /// What range is queried on the chunk store.
    ///
    /// This is sourced either from an override or via the `View`'s query range.
    pub query_range: QueryRange,
}

impl DataResult {
    /// The override path for this data result.
    ///
    /// This is **not** the override path for a concrete visualizer instruction yet.
    /// Refer to [`VisualizerInstruction::override_path`] for that.
    ///
    /// There are certain special "overrides" that are global to the entire entity (in the context of a view).
    /// Some of them are:
    /// - `EntityBehavior` (visible, interactive)
    /// - `CoordinateFrame`
    /// - `VisibleTimeRanges`
    ///
    /// All other overrides, are specific to a visualizer instruction and should use [`VisualizerInstruction::override_path`].
    #[inline]
    pub fn override_base_path(&self) -> &EntityPath {
        &self.override_base_path
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
        if self.has_base_override(ctx, EntityBehavior::descriptor_visible().component) {
            let parent_visibility = self
                .entity_path
                .parent()
                .and_then(|parent_path| data_result_tree.lookup_result_by_path(parent_path.hash()))
                .is_none_or(|data_result| data_result.is_visible());

            if parent_visibility == new_value {
                ctx.clear_blueprint_component(
                    self.override_base_path.clone(),
                    EntityBehavior::descriptor_visible(),
                );
                return;
            }
        }

        ctx.save_blueprint_archetype(
            self.override_base_path.clone(),
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
        if self.has_base_override(ctx, EntityBehavior::descriptor_interactive().component) {
            let parent_interactivity = self
                .entity_path
                .parent()
                .and_then(|parent_path| data_result_tree.lookup_result_by_path(parent_path.hash()))
                .is_none_or(|data_result| data_result.is_interactive());

            if parent_interactivity == new_value {
                ctx.clear_blueprint_component(
                    self.override_base_path.clone(),
                    EntityBehavior::descriptor_interactive(),
                );
                return;
            }
        }

        ctx.save_blueprint_archetype(
            self.override_base_path.clone(),
            &blueprint_archetypes::EntityBehavior::update_fields().with_interactive(new_value),
        );
    }

    // TODO: only checks first visualizer instruction. plz move up to instruction. -- do it.
    fn has_base_override(&self, ctx: &ViewerContext<'_>, component: ComponentIdentifier) -> bool {
        self.visualizer_instructions
            .first()
            .is_some_and(|instruction| {
                instruction
                    .component_overrides
                    .get(&component)
                    .is_some_and(|component| {
                        ctx.store_context
                            .blueprint
                            .latest_at(
                                ctx.blueprint_query,
                                &instruction.override_path,
                                [*component],
                            )
                            .get(*component)
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
