use std::sync::Arc;

use re_log_types::hash::Hash64;
use re_viewer_context::VisualizerComponentSource;

use crate::{
    BlueprintResolvedLatestAtResults, BlueprintResolvedRangeResults, ComponentMappingError,
};

impl<'a> BlueprintResolvedRangeResults<'a> {
    /// Merges bootstrapped data from a latest-at query into this range query result.
    ///
    /// Latest-at bootstrapping is used for optional/recommended components (like colors, radii,
    /// labels, etc.) that should maintain stable values at the beginning of a visible time range.
    /// Without bootstrapping, these components would only appear where they have data within the
    /// visible range, causing them to change unexpectedly as the time window moves.
    ///
    /// For example, if a plot's color was set at t=50 and you're viewing t=100-200, you want
    /// that color to persist throughout the range rather than disappearing or changing.
    ///
    /// Bootstrapped results are prepended with their logged time and row ID intact. This lets
    /// range zipping apply optional data only when it was logged no later than the required data,
    /// including when events share a timestamp. Any errors from the bootstrap query are also
    /// merged into the component sources.
    // TODO(andreas): It's a bit overkill to do a full blueprint resolved query for both the range & latest-at part. This can be optimized!
    pub fn merge_bootstrapped_data(&mut self, bootstrapped: BlueprintResolvedLatestAtResults<'a>) {
        // Merge component source from bootstrap into the range results.
        #[expect(clippy::iter_over_hash_type)] // Fills up another hash type.
        for (component, bootstrap_source) in bootstrapped.component_sources {
            match self.component_sources.entry(component) {
                std::collections::hash_map::Entry::Occupied(mut range_query_source) => {
                    let bootstrap_hash = bootstrapped
                        .store_results
                        .get(component)
                        .map(|chunk| (chunk.row_id(), bootstrap_source.recording_result_hash));
                    let range_hash = range_query_source.get().recording_result_hash;
                    let merged_hash = bootstrap_hash.map_or(range_hash, |(row_id, hash)| {
                        Hash64::hash((row_id, hash, range_hash))
                    });

                    match bootstrap_source.error() {
                        None => {
                            // A component may exist only before (or after) the visible range.
                            // In that case, promote the query overall to successful.
                            let range_source = range_query_source.get();
                            // Annotation-context sources are ignored here: their resolved values
                            // are not merged from the bootstrap query.
                            // Overrides are non-temporal and should agree between both queries.
                            let bootstraps_auto_default =
                                matches!(range_source.source(), VisualizerComponentSource::Default)
                                    && matches!(
                                        bootstrap_source.source(),
                                        VisualizerComponentSource::SourceComponent { .. }
                                    );
                            if bootstraps_auto_default
                                || range_source
                                    .error()
                                    .is_some_and(|err| err.is_data_temporarily_unavailable())
                            {
                                *range_query_source.get_mut() = bootstrap_source;
                            }
                            // Otherwise, don't override. Let the range result take precedence.
                        }

                        Some(
                            ComponentMappingError::ComponentNotPresentOnEntity { .. }
                            | ComponentMappingError::NoComponentDataForQuery(_)
                            | ComponentMappingError::NoComponentDataForQueryButIsFetchable(_)
                            | ComponentMappingError::AnnotationContextUnavailable(_),
                        ) => {
                            // No component data was found in the bootstrap data.
                            // Data may only exist within the range actual range, if not it has the error already!
                        }

                        Some(
                            ComponentMappingError::SelectorParseFailed(_)
                            | ComponentMappingError::SelectorExecutionFailed(_)
                            | ComponentMappingError::CastFailed { .. }
                            | ComponentMappingError::OverrideUnavailable(_),
                        ) => {
                            // All these errors are likely to occur in the range query as well.
                            // However, in case they don't we treat it as-if we failed first and then never executed the range query.
                            // I.e. override what's there!
                            *range_query_source.get_mut() = bootstrap_source;
                        }
                    }
                    range_query_source.get_mut().recording_result_hash = merged_hash;
                }

                std::collections::hash_map::Entry::Vacant(vacant_entry) => {
                    // Data showed up only in the target range result.
                    // That's unusual since we'd expect the same query on bootstrapped & the target range result,
                    // but sure enough we can just combine in the bootstrap!
                    vacant_entry.insert(bootstrap_source);
                }
            }
        }

        // Prepend bootstrapped chunks to range results
        #[expect(clippy::iter_over_hash_type)] // Fills up another hash type.
        for (component, unit_chunk) in bootstrapped.store_results.components {
            let chunk = Arc::unwrap_or_clone(unit_chunk.into_chunk());

            // Prepend bootstrapped chunk to range results
            self.store_results
                .components
                .entry(component)
                .or_default()
                .insert(0, chunk);
        }
    }
}

#[cfg(test)]
mod tests {
    use re_chunk_store::{LatestAtQuery, RangeQuery, RowId};
    use re_log_types::{AbsoluteTimeRange, build_frame_nr};
    use re_sdk_types::components::Color;
    use re_test_context::TestContext;
    use re_types_core::{Component as _, ComponentDescriptor};
    use re_viewer_context::{VisualizerComponentMappings, VisualizerComponentSource};

    use crate::query::{run_with_test_view_context, test_data_result, test_instruction};
    use crate::{latest_at_with_blueprint_resolved_data, range_with_blueprint_resolved_data};

    #[test]
    fn bootstrap_recovers_auto_mapped_optional_components_but_preserves_explicit_defaults() {
        let mut test = TestContext::new();
        let descriptor = ComponentDescriptor::partial("color").with_component_type(Color::name());
        let component = descriptor.component;
        test.log_entity("entity", |builder| {
            builder.with_component_batches(
                RowId::new(),
                [build_frame_nr(1)],
                [(descriptor.clone(), &[Color::from_rgb(1, 2, 3)] as _)],
            )
        });
        let data_result = test_data_result("entity");

        for explicit_default in [false, true] {
            let instruction = test_instruction(if explicit_default {
                VisualizerComponentMappings::from([(component, VisualizerComponentSource::Default)])
            } else {
                VisualizerComponentMappings::default()
            });

            run_with_test_view_context(&test, |ctx| {
                let mut range = range_with_blueprint_resolved_data(
                    ctx,
                    None,
                    &RangeQuery::new("frame_nr".into(), AbsoluteTimeRange::new(5, 20)),
                    &data_result,
                    [component],
                    &instruction,
                );
                assert!(matches!(
                    range.component_sources[&component].source(),
                    VisualizerComponentSource::Default
                ));

                let latest = latest_at_with_blueprint_resolved_data(
                    ctx,
                    None,
                    &LatestAtQuery::new("frame_nr".into(), 4),
                    &data_result,
                    [component],
                    Some(&instruction),
                );
                range.merge_bootstrapped_data(latest);
                assert_eq!(
                    matches!(
                        range.component_sources[&component].source(),
                        VisualizerComponentSource::Default
                    ),
                    explicit_default,
                );
                assert!(range.component_sources[&component].error().is_none());
            });
        }
    }

    /// Bootstrap data recovers an empty range, and subsequent changes on either side invalidate it.
    #[test]
    fn bootstrapped_hash_preserves_range_and_latest_dependencies() {
        let mut test_context = TestContext::new();
        let descriptor = ComponentDescriptor::partial("source").with_component_type(Color::name());
        let target = "target".into();
        let data_result = test_data_result("entity");
        let instruction = test_instruction(VisualizerComponentMappings::from([(
            target,
            VisualizerComponentSource::simple_map(descriptor.component),
        )]));
        let mut previous = None;

        for frame in [1, 10, 2, 11] {
            test_context.log_entity("entity", |builder| {
                builder.with_component_batches(
                    RowId::new(),
                    [build_frame_nr(frame)],
                    [(descriptor.clone(), &[Color::from_rgb(1, 2, 3)] as _)],
                )
            });

            let current = run_with_test_view_context(&test_context, |ctx| {
                let mut range = range_with_blueprint_resolved_data(
                    ctx,
                    None,
                    &RangeQuery::new("frame_nr".into(), AbsoluteTimeRange::new(5, 20)),
                    &data_result,
                    [target],
                    &instruction,
                );
                assert_eq!(
                    range.component_sources[&target].error().is_some(),
                    frame == 1,
                    "Only the initial query should need recovery from an empty range"
                );
                let latest = latest_at_with_blueprint_resolved_data(
                    ctx,
                    None,
                    &LatestAtQuery::new("frame_nr".into(), 4),
                    &data_result,
                    [target],
                    Some(&instruction),
                );
                range.merge_bootstrapped_data(latest);
                let error = range.component_sources[&target].error();
                assert!(
                    error.is_none(),
                    "Bootstrap should resolve the component: {error:?}"
                );
                range.query_result_hash()
            });

            if let Some(previous) = previous {
                assert_ne!(
                    previous, current,
                    "Logging frame {frame} must invalidate the bootstrapped result"
                );
            }
            previous = Some(current);
        }
    }
}
