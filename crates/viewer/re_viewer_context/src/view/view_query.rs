use std::collections::BTreeMap;

use itertools::Either;
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
#[derive(Clone, Debug, PartialEq, Eq)]
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

        match source_kind {
            ComponentSourceKind::SourceComponent => Self::SourceComponent {
                source_component: source_component
                    .as_ref()
                    .map(|c| c.as_str())
                    .unwrap_or_else(|| target.as_str())
                    .into(),
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

    pub fn component_source_kind(&self) -> ComponentSourceKind {
        match self {
            Self::SourceComponent { .. } => ComponentSourceKind::SourceComponent,
            Self::Override => ComponentSourceKind::Override,
            Self::Default => ComponentSourceKind::Default,
        }
    }

    /// The identity mapping for the given target component.
    pub fn identity(target: ComponentIdentifier) -> Self {
        Self::SourceComponent {
            source_component: target,
            selector: String::new(),
        }
    }

    /// True if the mapping have no effect on the target.
    ///
    /// I.e. it maps directly from the target back to the target.
    pub fn is_identity_mapping(&self, target: ComponentIdentifier) -> bool {
        self == &Self::identity(target)
    }
}

/// Component mappings for a visualizer instruction.
///
/// Maps from target component to source component (selector).
pub type VisualizerComponentMappings = BTreeMap<ComponentIdentifier, VisualizerComponentSource>;

/// A set of mandatory component mappings that form a single recommendation.
///
/// The invariant is that the identity mapping is always a valid state (i.e. `Default` produces
/// an empty/identity mapping that any visualizer satisfies).
#[derive(Clone, Debug, Default)]
pub struct RecommendedMappings {
    mandatory_mappings: VisualizerComponentMappings,
}

impl RecommendedMappings {
    /// Creates a recommendation with a single mandatory mapping.
    pub fn new(target: ComponentIdentifier, source: VisualizerComponentSource) -> Self {
        Self {
            mandatory_mappings: BTreeMap::from([(target, source)]),
        }
    }

    /// Returns `true` if all mandatory mappings in this recommendation are already
    /// satisfied by the given existing component mappings.
    pub fn is_covered_by(&self, existing_mappings: &VisualizerComponentMappings) -> bool {
        self.mandatory_mappings
            .iter()
            .all(|(component, recommended_source)| {
                let current_source = existing_mappings.get(component);
                let recommendation_is_identity = recommended_source.is_identity_mapping(*component);
                let current_mapping_is_identity =
                    current_source.is_none_or(|m| m.is_identity_mapping(*component));

                // Two mappings are considered equivalent when both are identity mappings for the
                // same target, or when they are exactly equal.
                (recommendation_is_identity && current_mapping_is_identity)
                    || current_source == Some(recommended_source)
            })
    }

    /// Returns a [`VisualizerInstruction`] with the recommended mappings.
    pub fn into_visualizer_instruction(
        self,
        id: VisualizerInstructionId,
        visualizer_type: ViewSystemIdentifier,
        override_base_path: &EntityPath,
    ) -> VisualizerInstruction {
        VisualizerInstruction::new(
            id,
            visualizer_type,
            override_base_path,
            self.mandatory_mappings,
        )
    }

    /// True if there is a mapping targeting the given component.
    pub fn contains_mapping_for_component(&self, component: &ComponentIdentifier) -> bool {
        self.mandatory_mappings.contains_key(component)
    }

    /// Returns the underlying component mappings.
    pub fn into_mappings(self) -> VisualizerComponentMappings {
        self.mandatory_mappings
    }

    /// Human-readable display name derived from the first component source.
    pub fn display_name(&self) -> Option<String> {
        self.mandatory_mappings
            .iter()
            .find_map(|(_target, source)| match source {
                VisualizerComponentSource::SourceComponent {
                    source_component,
                    selector,
                } => {
                    let name = source_component.as_str();
                    let short_name = name
                        .strip_prefix("rerun.components.")
                        .or_else(|| name.strip_prefix("rerun."))
                        .unwrap_or(name);
                    Some(format!("{short_name}{selector}"))
                }
                _ => None,
            })
    }

    /// Returns the source for the given component mapping.
    pub fn get_source_for_component(
        &self,
        target: &ComponentIdentifier,
    ) -> Option<&VisualizerComponentSource> {
        self.mandatory_mappings.get(target)
    }
}

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

    /// There are any visualizers that *can* run on this data result.
    pub any_visualizers_available: bool,

    /// Describes which visualizers should run for this data result.
    ///
    /// Invisible data results may still have visualizer instructions here,
    /// but they aren't considered active.
    ///
    /// [`Self::any_visualizers_available`] may be true even if this is empty.
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
    ///
    /// This is where "visible time range" is appled, if at all.
    pub fn query_range(&self) -> &QueryRange {
        &self.query_range
    }
}

pub type VisualizerInstructionsPerType<'a> =
    BTreeMap<ViewSystemIdentifier, Vec<(&'a DataResult, &'a VisualizerInstruction)>>;

#[derive(Debug)]
pub struct ViewQuery<'s> {
    /// The id of the space in which context the query happens.
    pub view_id: ViewId,

    /// The root of the space in which context the query happens.
    pub space_origin: &'s EntityPath,

    /// All active visualizer instructions for each visualizer type.
    ///
    /// These are only from visible data results.
    pub active_visualizer_instructions_per_type: VisualizerInstructionsPerType<'s>,

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
    /// Iter over all visible data results and their visualizer instructions for the given visualizer type.
    pub fn iter_visualizer_instruction_for(
        &self,
        visualizer: ViewSystemIdentifier,
    ) -> impl Iterator<Item = (&'s DataResult, &'s VisualizerInstruction)> {
        if let Some(instructions) = self
            .active_visualizer_instructions_per_type
            .get(&visualizer)
        {
            Either::Left(instructions.iter().copied())
        } else {
            Either::Right(std::iter::empty())
        }
    }

    #[inline]
    pub fn latest_at_query(&self) -> LatestAtQuery {
        LatestAtQuery::new(self.timeline, self.latest_at)
    }
}
