use nohash_hasher::IntSet;
use re_query::LatestAtResults;
use re_types_core::ComponentIdentifier;
use re_viewer_context::{VisualizerComponentMappings, VisualizerComponentSource};

use crate::ComponentMappingError;
use crate::blueprint_resolved_results::ComponentSourcesMap;

/// All information required to resolve an explicit source-component mapping.
///
/// Also applies a [`re_lenses_core::Selector`], if specified.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct ActiveRemapping {
    pub target: ComponentIdentifier,
    pub source: ComponentIdentifier,
    pub selector: Option<re_lenses_core::Selector>,
}

impl ActiveRemapping {
    pub fn is_identity(&self) -> bool {
        self.selector.is_none() && self.target == self.source
    }
}

/// Recording-query plan after applying a visualizer's explicit component mappings.
pub struct ComponentMappingQueryPlan {
    /// All components that need to be queried from the recording.
    ///
    /// Not all of them may be present in the recording!
    /// If there's a remapping in [`Self::active_remappings`],
    /// that's an error (because someone explicitly requested it) which later needs to be set in [`Self::component_sources`].
    /// Otherwise we need to fall back to default for those missing.
    pub recording_queried_components: IntSet<ComponentIdentifier>,

    /// All the remappings that need to be applied to the results after querying the recording.
    pub active_remappings: Vec<ActiveRemapping>,

    /// Describes the mapping that happens to each component.
    ///
    /// Components that are not present in this map are either not queried or are heuristically mapped.
    pub component_sources: ComponentSourcesMap,
}

impl ComponentMappingQueryPlan {
    pub fn new(
        component_mappings: Option<&VisualizerComponentMappings>,
        overrides: &LatestAtResults,
        queried_components: IntSet<ComponentIdentifier>,
    ) -> Self {
        let Some(component_mappings) = component_mappings else {
            return Self {
                recording_queried_components: queried_components,
                active_remappings: Vec::new(),
                component_sources: ComponentSourcesMap::default(),
            };
        };

        let mut active_remappings = Vec::new();
        let mut component_sources = ComponentSourcesMap::default();

        for (target_component, source) in component_mappings {
            // Skip mappings that are not relevant to the current query.
            if !queried_components.contains(target_component) {
                continue;
            }

            let source_result = match source {
                VisualizerComponentSource::SourceComponent {
                    source_component,
                    selector,
                } => {
                    let selector = if selector.is_empty() {
                        Ok(None)
                    } else {
                        selector.parse::<re_lenses_core::Selector>().map(Some)
                    };

                    match selector {
                        Ok(selector) => {
                            // Keep identity mappings so the query path validates that the selected source exists.
                            active_remappings.push(ActiveRemapping {
                                target: *target_component,
                                source: *source_component,
                                selector,
                            });
                            Ok(source.source_kind())
                        }
                        Err(err) => Err(ComponentMappingError::SelectorParseFailed(err)),
                    }
                }

                VisualizerComponentSource::Override
                    if !has_non_empty_override(overrides, *target_component) =>
                {
                    Err(ComponentMappingError::OverrideUnavailable(
                        *target_component,
                    ))
                }

                _ => Ok(source.source_kind()),
            };

            component_sources.insert(*target_component, source_result);
        }

        let recording_queried_components = {
            let mut recording_queried_components = queried_components;

            // Remove anything that is remapped.
            for mapping_target in component_mappings.keys() {
                recording_queried_components.remove(mapping_target);
            }

            // Add sources last because a source can also be the target of another mapping.
            recording_queried_components
                .extend(active_remappings.iter().map(|remapping| remapping.source));

            recording_queried_components
        };

        Self {
            recording_queried_components,
            active_remappings,
            component_sources,
        }
    }
}

/// Returns `true` if the given component has a non-empty override.
///
/// Cleared overrides contain an empty Arrow array and must be treated as absent.
/// This only affects automatically determined sources; explicit overrides are validated by the plan.
pub fn has_non_empty_override(overrides: &LatestAtResults, component: ComponentIdentifier) -> bool {
    overrides
        .get(component)
        .and_then(|chunk| chunk.non_empty_component_batch_raw(component))
        .is_some()
}

#[cfg(test)]
mod tests {
    use re_log_types::EntityPath;
    use re_sdk_types::blueprint::encodings::ComponentSourceKind;
    use re_types_core::ComponentIdentifier;
    use re_viewer_context::{VisualizerComponentMappings, VisualizerComponentSource};

    use crate::component_mapping_query_plan::ComponentMappingQueryPlan;

    fn source_mapping(component: ComponentIdentifier) -> VisualizerComponentSource {
        VisualizerComponentSource::simple_map(component)
    }

    fn plan(
        mappings: &VisualizerComponentMappings,
        queried: impl IntoIterator<Item = ComponentIdentifier>,
    ) -> ComponentMappingQueryPlan {
        ComponentMappingQueryPlan::new(
            Some(mappings),
            &re_query::LatestAtResults::empty(
                EntityPath::root(),
                re_chunk_store::LatestAtQuery::new_static(),
            ),
            queried.into_iter().collect(),
        )
    }

    #[test]
    fn plan_one_source_for_multiple_targets_and_itself() {
        let source = "source".into();
        let target_a = "target_a".into();
        let target_b = "target_b".into();

        for explicitly_map_source_to_itself in [false, true] {
            let mut mappings = VisualizerComponentMappings::from([
                (target_a, source_mapping(source)),
                (target_b, source_mapping(source)),
            ]);
            if explicitly_map_source_to_itself {
                mappings.insert(source, VisualizerComponentSource::identity(source));
            }

            let plan = plan(&mappings, [source, target_a, target_b]);

            assert_eq!(plan.recording_queried_components.len(), 1);
            assert!(plan.recording_queried_components.contains(&source));
            assert_eq!(
                plan.active_remappings.len(),
                if explicitly_map_source_to_itself {
                    3
                } else {
                    2
                }
            );
            for target in [target_a, target_b] {
                assert!(plan.active_remappings.iter().any(|remapping| {
                    remapping.target == target
                        && remapping.source == source
                        && remapping.selector.is_none()
                }));
                assert!(matches!(
                    plan.component_sources.get(&target),
                    Some(Ok(ComponentSourceKind::SourceComponent))
                ));
            }

            assert_eq!(
                explicitly_map_source_to_itself,
                plan.active_remappings.iter().any(|remapping| {
                    remapping.target == source
                        && remapping.source == source
                        && remapping.selector.is_none()
                })
            );
            if explicitly_map_source_to_itself {
                assert!(matches!(
                    plan.component_sources.get(&source),
                    Some(Ok(ComponentSourceKind::SourceComponent))
                ));
            } else {
                assert!(!plan.component_sources.contains_key(&source));
            }
        }
    }

    #[test]
    fn plan_keeps_a_remapped_target_that_is_also_a_required_source() {
        let other_target = "a_other_target".into();
        let source_and_target = "b_source_and_target".into();
        let upstream_source = "upstream".into();

        let mappings = VisualizerComponentMappings::from([
            (other_target, source_mapping(source_and_target)),
            (source_and_target, source_mapping(upstream_source)),
        ]);

        let plan = plan(&mappings, [source_and_target, other_target]);

        assert_eq!(plan.recording_queried_components.len(), 2);
        assert!(
            plan.recording_queried_components
                .contains(&source_and_target)
        );
        assert!(plan.recording_queried_components.contains(&upstream_source));
        assert_eq!(plan.active_remappings.len(), 2);
    }
}
