use std::borrow::Cow;

use nohash_hasher::IntSet;
use re_query::LatestAtResults;
use re_types_core::ComponentIdentifier;
use re_viewer_context::{
    AnnotationContextQuery, VisualizerComponentMappings, VisualizerComponentSource,
};

use crate::ComponentMappingError;
use crate::blueprint_resolved_results::{
    ActiveRemapping, CheckedComponentSource, ComponentSourcesMap,
};

/// Recording-query plan after applying a visualizer's explicit component mappings.
pub struct ComponentMappingQueryPlan<'a> {
    /// All components that need to be queried from the recording.
    ///
    /// Not all of them may be present in the recording!
    /// If a source-component mapping is in [`Self::component_sources`],
    /// its missing source is an error that is recorded there.
    /// Otherwise we need to fall back to default for those missing.
    pub recording_queried_components: IntSet<ComponentIdentifier>,

    /// Describes the mapping that happens to each component.
    ///
    /// Components that are not present in this map are either not queried or are heuristically mapped.
    pub component_sources: ComponentSourcesMap<'a>,
}

impl<'a> ComponentMappingQueryPlan<'a> {
    pub fn new(
        component_mappings: Option<&'a VisualizerComponentMappings>,
        annotation_context: Option<&AnnotationContextQuery>,
        overrides: &LatestAtResults,
        queried_components: IntSet<ComponentIdentifier>,
    ) -> Self {
        let Some(component_mappings) = component_mappings else {
            return Self {
                recording_queried_components: queried_components,
                component_sources: ComponentSourcesMap::default(),
            };
        };

        let mut component_sources = ComponentSourcesMap::default();

        for (target_component, source) in component_mappings {
            // Skip mappings that are not relevant to the current query.
            if !queried_components.contains(target_component) {
                continue;
            }

            let checked_source = CheckedComponentSource::new(Cow::Borrowed(source));
            let checked_source = match source {
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
                        Ok(selector) => checked_source.with_remapping(ActiveRemapping {
                            source: *source_component,
                            selector,
                        }),
                        Err(err) => checked_source
                            .with_error(ComponentMappingError::SelectorParseFailed(err)),
                    }
                }

                VisualizerComponentSource::Override
                    if !has_non_empty_override(overrides, *target_component) =>
                {
                    checked_source.with_error(ComponentMappingError::OverrideUnavailable(
                        *target_component,
                    ))
                }

                VisualizerComponentSource::AnnotationContext
                    if !annotation_context_resolves(annotation_context, *target_component) =>
                {
                    checked_source.with_error(ComponentMappingError::AnnotationContextUnavailable(
                        *target_component,
                    ))
                }

                _ => checked_source,
            };

            component_sources.insert(*target_component, checked_source);
        }

        let recording_queried_components = {
            let mut recording_queried_components = queried_components;

            // Remove anything that is remapped.
            for mapping_target in component_mappings.keys() {
                recording_queried_components.remove(mapping_target);
            }

            // Add sources last because a source can also be the target of another mapping.
            recording_queried_components.extend(
                component_sources
                    .values()
                    .filter_map(CheckedComponentSource::remapping)
                    .map(|remapping| remapping.source),
            );

            recording_queried_components
        };

        Self {
            recording_queried_components,
            component_sources,
        }
    }
}

/// Returns `true` if the given component is resolved by the annotation context.
pub fn annotation_context_resolves(
    annotation_context: Option<&AnnotationContextQuery>,
    component: ComponentIdentifier,
) -> bool {
    annotation_context.is_some_and(|context| {
        context
            .targets
            .iter()
            .any(|target| target.descriptor().component == component)
    })
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
    use re_types_core::ComponentIdentifier;
    use re_viewer_context::{VisualizerComponentMappings, VisualizerComponentSource};

    use crate::component_mapping_query_plan::ComponentMappingQueryPlan;

    fn source_mapping(component: ComponentIdentifier) -> VisualizerComponentSource {
        VisualizerComponentSource::simple_map(component)
    }

    fn plan(
        mappings: &VisualizerComponentMappings,
        queried: impl IntoIterator<Item = ComponentIdentifier>,
    ) -> ComponentMappingQueryPlan<'_> {
        ComponentMappingQueryPlan::new(
            Some(mappings),
            None,
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
                plan.component_sources
                    .values()
                    .filter_map(|source| source.remapping())
                    .count(),
                if explicitly_map_source_to_itself {
                    3
                } else {
                    2
                }
            );
            for target in [target_a, target_b] {
                let actual_source = plan
                    .component_sources
                    .get(&target)
                    .expect("Expected a checked source-component mapping");
                assert_eq!(actual_source.source(), &source_mapping(source));
                assert_eq!(actual_source.remapping().unwrap().source, source);
                assert!(actual_source.remapping().unwrap().selector.is_none());
                assert!(actual_source.error().is_none());
            }

            if explicitly_map_source_to_itself {
                let actual_source = plan
                    .component_sources
                    .get(&source)
                    .expect("Expected a resolved identity mapping");
                assert_eq!(
                    actual_source.source(),
                    &VisualizerComponentSource::identity(source)
                );
                assert!(actual_source.remapping().is_some());
                assert!(actual_source.error().is_none());
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
        assert_eq!(
            plan.component_sources
                .values()
                .filter_map(|source| source.remapping())
                .count(),
            2
        );
    }
}
