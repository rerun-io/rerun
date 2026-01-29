use std::collections::BTreeMap;

use itertools::Itertools as _;
use nohash_hasher::IntSet;
use re_chunk::{ComponentIdentifier, TimelineName};
use re_chunk_store::LatestAtQuery;
use re_entity_db::{EntityPath, TimeInt};
use re_sdk_types::blueprint::archetypes::{self as blueprint_archetypes, EntityBehavior};
use re_sdk_types::blueprint::components::VisualizerInstructionId;
use re_sdk_types::blueprint::datatypes::{ComponentSourceKind, VisualizerComponentMapping};

use crate::blueprint_helpers::BlueprintContext as _;
use crate::{
    DataResultTree, QueryRange, ViewHighlights, ViewId, ViewSystemIdentifier, ViewerContext,
};

/// [`VisualizerComponentMapping`] but without the target.
#[derive(Clone, Debug)]
pub enum VisualizerComponentSource {
    /// See [`ComponentSourceKind::SourceComponent`].
    SourceComponent {
        source_component: ComponentIdentifier,
        selector: String,
    },

    /// See [`ComponentSourceKind::Override`].
    Override,

    /// See [`ComponentSourceKind::Default`].
    Default,
}

impl VisualizerComponentSource {
    pub fn from_blueprint_mapping(mapping: &VisualizerComponentMapping) -> Self {
        let VisualizerComponentMapping {
            target,
            source_kind,
            source_component,
            selector,
        } = mapping;

        let source_component = source_component.as_ref().unwrap_or(target);

        match source_kind {
            ComponentSourceKind::SourceComponent => Self::SourceComponent {
                source_component: source_component.as_str().into(),
                selector: selector.as_ref().map_or(String::new(), |s| s.to_string()),
            },

            ComponentSourceKind::Override => Self::Override,

            ComponentSourceKind::Default => Self::Default,
        }
    }

    pub fn source_kind(&self) -> ComponentSourceKind {
        match self {
            Self::SourceComponent { .. } => ComponentSourceKind::SourceComponent,
            Self::Override => ComponentSourceKind::Override,
            Self::Default => ComponentSourceKind::Default,
        }
    }

    /// True if the mapping have no effect on the target.
    ///
    /// I.e. it maps directly from the target back to the target.
    pub fn is_identity_mapping(&self, target: ComponentIdentifier) -> bool {
        match self {
            Self::SourceComponent {
                source_component,
                selector,
            } => source_component == &target && selector.is_empty(),
            Self::Override | Self::Default => false,
        }
    }
}

/// Component mappings for a visualizer instruction.
///
/// Maps from target component to source component (selector).
pub type VisualizerComponentMappings = BTreeMap<ComponentIdentifier, VisualizerComponentSource>;

#[derive(Clone, Debug)]
pub struct VisualizerInstruction {
    pub id: VisualizerInstructionId,

    pub visualizer_type: ViewSystemIdentifier,

    /// The blueprint path to the override values for this visualizer instruction.
    pub override_path: EntityPath,

    /// List of components that have overrides for this visualizer instruction.
    ///
    /// Note that this does *not* take into account tree propagation of any special components
    /// like `Visible`, `Interactive` or transform components.
    pub component_overrides: IntSet<ComponentIdentifier>,

    /// Component mappings from target to source (selector).
    ///
    /// Keys are target components, values describe where to source components (selectors).
    /// Any target component not present here uses an auto-mapping strategy.
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
        override_base_path.join(&EntityPath::from_single_string(id.to_string()))
    }

    /// Writes component mappings and visualizer type to the blueprint store.
    pub fn write_instruction_to_blueprint(&self, ctx: &ViewerContext<'_>) {
        let new_visualizer_instruction =
            re_sdk_types::blueprint::archetypes::VisualizerInstruction::new(
                self.visualizer_type.as_str(),
            )
            // We always have ti write the component map because it we may need to clear out old mappings.
            // TODO(andreas): can we avoid writing out needless data here? Often there are no mappings, so we keep writing empty arrays.
            .with_component_map(self.component_mappings.iter().map(|(target, mapping)| {
                let target = target.as_str().into();

                match mapping {
                    VisualizerComponentSource::SourceComponent {
                        source_component,
                        selector,
                    } => VisualizerComponentMapping {
                        target,
                        source_kind: ComponentSourceKind::SourceComponent,
                        source_component: (!source_component.is_empty())
                            .then(|| source_component.as_str().into()),
                        selector: (!selector.is_empty()).then(|| selector.as_str().into()),
                    },

                    VisualizerComponentSource::Override => VisualizerComponentMapping {
                        target,
                        source_kind: ComponentSourceKind::Override,
                        source_component: None,
                        selector: None,
                    },

                    VisualizerComponentSource::Default => VisualizerComponentMapping {
                        target,
                        source_kind: ComponentSourceKind::Default,
                        source_component: None,
                        selector: None,
                    },
                }
            }));

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
    pub entity_path: EntityPath,

    /// There are any visualizers that can run on this data result.
    pub any_visualizers_available: bool,

    /// Which `ViewSystems`s to pass the `DataResult` to.
    pub visualizer_instructions: Vec<VisualizerInstruction>,

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

    fn has_base_override(&self, ctx: &ViewerContext<'_>, component: ComponentIdentifier) -> bool {
        ctx.store_context
            .blueprint
            .latest_at(ctx.blueprint_query, &self.override_base_path, [component])
            .get(component)
            .is_some()
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
