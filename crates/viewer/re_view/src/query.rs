use std::sync::Arc;

use nohash_hasher::{IntMap, IntSet};
use re_arrow_util::ArrowArrayDowncastRef as _;
use re_chunk_store::{LatestAtQuery, RangeQuery, RowId};
use re_log_types::{
    TimeInt,
    external::arrow::{self, array::Array as _},
    hash::Hash64,
};
use re_query::LatestAtResults;
use re_sdk_types::blueprint::encodings::ComponentSourceKind;
use re_types_core::{Archetype, ComponentIdentifier};
use re_viewer_context::{DataResult, QueryRange, ViewContext, ViewQuery, ViewerContext};

use crate::blueprint_resolved_results::{
    BlueprintResolvedLatestAtResults, BlueprintResolvedRangeResults, ComponentSourcesMap,
};
use crate::component_mapping_query_plan::{
    ActiveRemapping, ComponentMappingQueryPlan, has_non_empty_override,
};
use crate::{BlueprintResolvedResults, ComponentMappingError};

/// Resolve a visible time range — the range of a [`QueryRange::TimeRange`] — into absolute times.
///
/// Cursor-relative boundaries resolve against the current time cursor, falling back to
/// [`TimeInt::ZERO`] when there is none. Every view that queries a time range must resolve it this
/// way, or two views given the same range would end up showing different data.
pub fn resolve_visible_time_range(
    ctx: &ViewerContext<'_>,
    time_range: &re_sdk_types::encodings::TimeRange,
) -> re_log_types::AbsoluteTimeRange {
    let cursor = ctx.time_ctrl.time_int().unwrap_or(TimeInt::ZERO);
    re_log_types::AbsoluteTimeRange::from_relative_time_range(time_range, cursor)
}

/// A rule that decides the cast destination for a polymorphic target slot based on the
/// source's element datatype.
///
/// Returning `Some(dt)` requests that the source array be cast to `dt`.
/// Returning `None` rejects the source datatype: the surrounding query reports a
/// [`ComponentMappingError::CastFailed`] and the target slot ends up empty for that chunk.
///
/// This is the per-slot override consulted by [`range_with_blueprint_resolved_data_polymorphic`]
/// and [`latest_at_with_blueprint_resolved_data_polymorphic`]. When no rule is provided for a
/// target, the existing behavior (cast to the target component's reflection-registered datatype)
/// applies.
pub type ComponentCastRule = fn(&arrow::datatypes::DataType) -> Option<arrow::datatypes::DataType>;

/// Casts to a `ListArray` with values matching `target_value_datatype`.
///
/// Returns `source` unchanged if already the correct type (zero-copy).
fn cast_list_array(
    source: &arrow::array::ListArray,
    target_list_datatype: &arrow::datatypes::DataType,
) -> Result<arrow::array::ListArray, arrow::error::ArrowError> {
    // Happy path: already the right type.
    if source.data_type() == target_list_datatype {
        return Ok(source.clone());
    }

    // Cast the entire list array to the target type, handling both value type
    // changes (e.g., Int32 → Float32) and structural changes (e.g., FixedSizeList → List).
    let casted = arrow::compute::cast(source, target_list_datatype)?;

    casted
        .try_downcast_array::<arrow::array::ListArray>()
        .map_err(|err| {
            arrow::error::ArrowError::CastError(format!("Expected a ListArray after cast: {err}"))
        })
}

/// How to decide the cast destination for a remapped target slot.
enum CastTarget {
    /// Cast to a fixed datatype or skip the cast entirely when `None`.
    Fixed(Option<arrow::datatypes::DataType>),

    /// Derive the destination from the element datatype via the rule.
    Polymorphic(ComponentCastRule),
}

/// Applies a selector (if present) and casts the component for known datatypes (if required).
fn transform_chunk(
    mapping: &ActiveRemapping,
    cast: &CastTarget,
    chunk: &re_chunk_store::Chunk,
) -> Result<re_chunk_store::Chunk, ComponentMappingError> {
    chunk.with_shadowed_component(mapping.source, mapping.target, |arr| {
        let transformed = if let Some(selector) = &mapping.selector {
            selector
                .execute_per_row(&arr)
                .map_err(ComponentMappingError::SelectorExecutionFailed)?
                .unwrap_or_else(|| {
                    arrow::array::ListArray::new_null(
                        arrow::datatypes::Field::new_list_field(arr.value_type(), true).into(),
                        arr.len(),
                    )
                })
        } else {
            arr
        };

        let target_datatype = match cast {
            CastTarget::Polymorphic(rule) => rule(&transformed.value_type()),
            CastTarget::Fixed(dt) => dt.clone(),
        };

        // Apply casting if target datatype is known.
        if let Some(dt) = target_datatype {
            let target_list_datatype = arrow::datatypes::DataType::List(Arc::new(
                // TODO(grtlr): Ideally we'd make a more informed guess about nullability here.
                // But in the context of components setting the `ListArray` to nullable is the safe choice.
                arrow::datatypes::Field::new_list_field(dt.clone(), true),
            ));

            cast_list_array(&transformed, &target_list_datatype).map_err(|err| {
                ComponentMappingError::CastFailed {
                    source_datatype: transformed.data_type().clone(),
                    target_datatype: target_list_datatype,
                    err: Arc::new(err),
                }
            })
        } else {
            Ok(transformed)
        }
    })
}

/// Decide how the cast destination is chosen for one remapped target slot.
///
/// With a polymorphic `rule`, the destination is derived per-chunk from the post-selector
/// element datatype. Without a rule, fall back to the target component's
/// reflection-registered datatype.
fn cast_target_for_remapping(
    rule: Option<ComponentCastRule>,
    target: &ComponentIdentifier,
    reflection: &re_types_core::reflection::Reflection,
) -> CastTarget {
    match rule {
        Some(rule) => CastTarget::Polymorphic(rule),
        None => CastTarget::Fixed(reflection.lookup_datatype(*target).cloned()),
    }
}

/// Determines the exact reason why a component was not found.
fn component_not_found_error(
    component: ComponentIdentifier,
    entity_path: &re_log_types::EntityPath,
    missing_virtual_chunks: &[re_chunk_store::ChunkId],
    entity_db: &re_entity_db::EntityDb,
    store_engine: &re_query::StorageEngineReadGuard<'_>,
    timeline_name: Option<re_log_types::TimelineName>,
) -> ComponentMappingError {
    // Check whether the component is *ever* present on this entity.
    // Since static data would show up in both latest-at & range queries, we only care about temporal data here.
    if timeline_name.is_some_and(|timeline_name| {
        entity_db.entity_has_temporal_data_on_timeline_for_component(
            store_engine,
            &timeline_name,
            entity_path,
            component,
        )
    }) {
        ComponentMappingError::NoComponentDataForQuery(component)
    } else {
        // Check whether the data *might* come in later.
        if !missing_virtual_chunks.is_empty()
            && let Some(rrd_manifest) = entity_db.rrd_manifest_index().manifest()
        {
            let store = store_engine.store();

            let timeline = timeline_name
                .and_then(|timeline_name| store.schema().timelines().get(&timeline_name).copied());

            for missing_root_chunk_id in missing_virtual_chunks
                .iter()
                .flat_map(|chunk_id| store.find_root_chunks(chunk_id))
            {
                if let Some(per_component) = rrd_manifest.static_map().get(entity_path)
                    && per_component.get(&component) == Some(&missing_root_chunk_id)
                {
                    return ComponentMappingError::NoComponentDataForQueryButIsFetchable(component);
                }

                if let Some(timeline) = &timeline
                    && let Some(per_timeline) = rrd_manifest.temporal_map().get(entity_path)
                    && let Some(per_component) = per_timeline.get(timeline)
                    && let Some(per_chunk) = per_component.get(&component)
                    && per_chunk.contains_key(&missing_root_chunk_id)
                {
                    return ComponentMappingError::NoComponentDataForQueryButIsFetchable(component);
                }
            }
        }

        // Seems the lack of data is just specific to our current query.
        let available_components = store_engine.schema().all_components_for_entity(entity_path);
        if available_components.is_some_and(|components| components.contains(&component)) {
            // The component exists on the entity, but there is no data for it in the current query.
            ComponentMappingError::NoComponentDataForQuery(component)
        } else {
            // The component does not exist on the entity - maybe it's a typo, so provide similar.
            ComponentMappingError::component_not_present_on_entity(
                component,
                available_components.into_iter().flatten().copied(),
            )
        }
    }
}

/// Queries for the given `components` using range semantics with blueprint support.
///
/// Data will be resolved, in order of priority:
/// - Data overrides from the blueprint
/// - Data from the recording
/// - Default data from the blueprint
/// - Fallback from the visualizer
/// - Placeholder from the component.
///
/// Data should be accessed via the [`crate::BlueprintResolvedResultsExt`] trait which is implemented for
/// [`crate::BlueprintResolvedResults`].
pub fn range_with_blueprint_resolved_data<'a>(
    ctx: &'a ViewContext<'a>,
    annotations: Option<&re_viewer_context::Annotations>,
    range_query: &RangeQuery,
    data_result: &'a re_viewer_context::DataResult,
    components: impl IntoIterator<Item = ComponentIdentifier>,
    visualizer_instruction: &re_viewer_context::VisualizerInstruction,
) -> BlueprintResolvedRangeResults<'a> {
    range_with_blueprint_resolved_data_polymorphic(
        ctx,
        annotations,
        range_query,
        data_result,
        components,
        visualizer_instruction,
        &IntMap::default(),
    )
}

/// Like [`range_with_blueprint_resolved_data`] but with per-target polymorphic cast rules.
///
/// For each target component listed in `cast_rules`, the cast destination is decided per-chunk
/// from the chunk's actual source element datatype via the supplied [`ComponentCastRule`],
/// instead of being read from the target component's reflection-registered datatype.
///
/// This lets a single mapping slot accept heterogeneous source types (e.g. ints, floats, bools,
/// strings) and canonicalize them according to caller-defined rules without coercing everything
/// to the target's nominal datatype.
pub fn range_with_blueprint_resolved_data_polymorphic<'a>(
    ctx: &'a ViewContext<'a>,
    _annotations: Option<&re_viewer_context::Annotations>,
    range_query: &RangeQuery,
    data_result: &'a re_viewer_context::DataResult,
    components: impl IntoIterator<Item = ComponentIdentifier>,
    visualizer_instruction: &re_viewer_context::VisualizerInstruction,
    cast_rules: &IntMap<ComponentIdentifier, ComponentCastRule>,
) -> BlueprintResolvedRangeResults<'a> {
    re_tracing::profile_function!(data_result.entity_path.to_string());

    // TODO(andreas): It would be great to avoid querying for overrides & store values that aren't used due to explicit source components.
    // Logic gets surprisingly complicated quickly though.

    let queried_components = components.into_iter().collect::<IntSet<_>>();

    let overrides = query_overrides(
        ctx.viewer_ctx,
        visualizer_instruction,
        queried_components.iter().copied(),
    );

    let ComponentMappingQueryPlan {
        recording_queried_components,
        active_remappings,
        mut component_sources,
    } = ComponentMappingQueryPlan::new(
        Some(&visualizer_instruction.component_mappings),
        &overrides,
        queried_components,
    );

    let engine = ctx.recording_engine();
    let mut store_results = engine.cache().range(
        re_chunk_store::ChunkTrackingMode::Report,
        range_query,
        &data_result.entity_path,
        recording_queried_components.iter().copied(),
    );

    // Now that we know which store components are present, we can auto-determine all component sources that haven't been explicitly assigned yet.
    auto_determine_remaining_sources(
        &mut component_sources,
        recording_queried_components,
        |component| store_results.components.contains_key(&component),
        &overrides,
    );

    // Buffer remapped components so every mapping reads the original query results.
    // This keeps chained mappings independent of their iteration order.
    let mut remapped_store_results = Vec::with_capacity(active_remappings.len());

    for mapping in &active_remappings {
        if let Some(chunks) = store_results.components.get(&mapping.source) {
            if mapping.is_identity() && !cast_rules.contains_key(&mapping.target) {
                continue;
            }

            // Clone instead of removing because multiple mappings may reference the same source.
            let mut chunks = chunks.clone();
            let cast = cast_target_for_remapping(
                cast_rules.get(&mapping.target).copied(),
                &mapping.target,
                ctx.viewer_ctx.reflection(),
            );
            'ctx: {
                for chunk in &mut chunks {
                    let result = transform_chunk(mapping, &cast, chunk);

                    match result {
                        Ok(modified_chunk) => *chunk = modified_chunk,
                        Err(err) => {
                            component_sources.insert(mapping.target, Err(err));
                            break 'ctx;
                        }
                    }
                }
                remapped_store_results.push((mapping.target, chunks));
            }
        } else {
            component_sources.insert(
                mapping.target,
                Err(component_not_found_error(
                    mapping.source,
                    &data_result.entity_path,
                    &store_results.missing_virtual,
                    ctx.recording(),
                    &engine,
                    Some(range_query.timeline),
                )),
            );
        }
    }
    store_results.components.extend(remapped_store_results);

    // TODO(andreas): Rather strange to have a latest-at query in here.
    let query_context = ctx.query_context(
        data_result,
        LatestAtQuery::new(range_query.timeline, range_query.range.min),
        visualizer_instruction.id,
    );

    BlueprintResolvedRangeResults {
        overrides,
        store_results,
        query_context,
        view_defaults: &ctx.query_result.view_defaults,
        component_sources,
        component_mappings_hash: Hash64::hash(&visualizer_instruction.component_mappings),
    }
}

/// Queries for the given `components` using latest-at semantics with blueprint support.
///
/// Data will be resolved, in order of priority:
/// - Data overrides from the blueprint
/// - Data from the recording
/// - Default data from the blueprint
/// - Fallback from the visualizer
/// - Placeholder from the component.
///
/// Data should be accessed via the [`crate::BlueprintResolvedResultsExt`] trait which is implemented for
/// [`crate::BlueprintResolvedResults`].
pub fn latest_at_with_blueprint_resolved_data<'a>(
    ctx: &'a ViewContext<'a>,
    annotations: Option<&'a re_viewer_context::Annotations>,
    latest_at_query: &LatestAtQuery,
    data_result: &'a re_viewer_context::DataResult,
    components: impl IntoIterator<Item = ComponentIdentifier>,
    visualizer_instruction: Option<&re_viewer_context::VisualizerInstruction>,
) -> BlueprintResolvedLatestAtResults<'a> {
    latest_at_with_blueprint_resolved_data_polymorphic(
        ctx,
        annotations,
        latest_at_query,
        data_result,
        components,
        visualizer_instruction,
        &IntMap::default(),
    )
}

/// Like [`latest_at_with_blueprint_resolved_data`] but with per-target polymorphic cast rules.
///
/// See [`range_with_blueprint_resolved_data_polymorphic`] for the cast-rule semantics.
pub fn latest_at_with_blueprint_resolved_data_polymorphic<'a>(
    ctx: &'a ViewContext<'a>,
    _annotations: Option<&'a re_viewer_context::Annotations>,
    latest_at_query: &LatestAtQuery,
    data_result: &'a re_viewer_context::DataResult,
    components: impl IntoIterator<Item = ComponentIdentifier>,
    visualizer_instruction: Option<&re_viewer_context::VisualizerInstruction>,
    cast_rules: &IntMap<ComponentIdentifier, ComponentCastRule>,
) -> BlueprintResolvedLatestAtResults<'a> {
    // This is called very frequently, don't put a profile scope here.

    // TODO(andreas): It would be great to avoid querying for overrides & store values that aren't used due to explicit source components.
    // Logic gets surprisingly complicated quickly though.

    let queried_components = components.into_iter().collect::<IntSet<_>>();
    let overrides = if let Some(visualizer_instruction) = visualizer_instruction {
        query_overrides(
            ctx.viewer_ctx,
            visualizer_instruction,
            queried_components.iter().copied(),
        )
    } else {
        query_overrides_at_path(
            ctx.viewer_ctx,
            data_result.override_base_path(),
            queried_components.iter().copied(),
        )
    };

    let ComponentMappingQueryPlan {
        recording_queried_components: queried_components,
        active_remappings,
        mut component_sources,
    } = ComponentMappingQueryPlan::new(
        visualizer_instruction.map(|instruction| &instruction.component_mappings),
        &overrides,
        queried_components,
    );

    let engine = ctx.viewer_ctx.recording_engine();
    let mut store_results = engine.cache().latest_at(
        re_chunk_store::ChunkTrackingMode::Report,
        latest_at_query,
        &data_result.entity_path,
        queried_components.iter().copied(),
    );

    // Now that we know which store components are present, we can auto-determine all component sources that haven't been explicitly assigned yet.
    auto_determine_remaining_sources(
        &mut component_sources,
        queried_components,
        |component| store_results.components.contains_key(&component),
        &overrides,
    );

    // Buffer remapped components so every mapping reads the original query results.
    // This keeps chained mappings independent of their iteration order.
    let mut remapped_store_results = Vec::with_capacity(active_remappings.len());

    for mapping in &active_remappings {
        // Borrow instead of removing because multiple mappings may reference the same source.
        if let Some(chunk) = store_results.components.get(&mapping.source) {
            if mapping.is_identity() && !cast_rules.contains_key(&mapping.target) {
                continue;
            }
            let cast = cast_target_for_remapping(
                cast_rules.get(&mapping.target).copied(),
                &mapping.target,
                ctx.viewer_ctx.reflection(),
            );
            let result = transform_chunk(mapping, &cast, chunk);
            match result {
                Ok(modified_chunk) => {
                    let chunk = std::sync::Arc::new(modified_chunk)
                        .to_unit()
                        .expect("The source chunk was a unit chunk.");
                    remapped_store_results.push((mapping.target, chunk));
                }
                Err(err) => {
                    component_sources.insert(mapping.target, Err(err));
                }
            }
        } else {
            component_sources.insert(
                mapping.target,
                Err(component_not_found_error(
                    mapping.source,
                    &data_result.entity_path,
                    &store_results.missing_virtual,
                    ctx.viewer_ctx.recording(),
                    &engine,
                    latest_at_query.timeline(),
                )),
            );
        }
    }
    store_results.components.extend(remapped_store_results);

    let query_context = ctx.query_context(
        data_result,
        latest_at_query.clone(),
        visualizer_instruction.map(|instruction| instruction.id),
    );

    BlueprintResolvedLatestAtResults {
        overrides,
        store_results,
        view_defaults: &ctx.query_result.view_defaults,
        query_context,
        component_sources,
        component_mappings_hash: Hash64::hash(
            visualizer_instruction.map(|instruction| &instruction.component_mappings),
        ),
    }
}

/// Computes the component sources for all components not yet present in `component_sources` by checking for overrides and store results.
fn auto_determine_remaining_sources(
    component_sources: &mut ComponentSourcesMap,
    queried_components: IntSet<ComponentIdentifier>,
    has_store_result: impl Fn(ComponentIdentifier) -> bool,
    overrides: &LatestAtResults,
) {
    #[expect(clippy::iter_over_hash_type)] // Doing that to fill another hashmap.
    for component in queried_components {
        let std::collections::hash_map::Entry::Vacant(entry) = component_sources.entry(component)
        else {
            continue;
        };

        let source = if has_non_empty_override(overrides, component) {
            ComponentSourceKind::Override
        } else if has_store_result(component) {
            ComponentSourceKind::SourceComponent
        } else {
            ComponentSourceKind::Default
        };

        entry.insert(Ok(source));
    }
}

fn query_overrides(
    ctx: &ViewerContext<'_>,
    visualizer_instruction: &re_viewer_context::VisualizerInstruction,
    components: impl IntoIterator<Item = ComponentIdentifier>,
) -> LatestAtResults {
    if visualizer_instruction.component_overrides.is_empty() {
        LatestAtResults::empty("<overrides>".into(), ctx.current_query())
    } else {
        query_overrides_at_path(
            ctx,
            &visualizer_instruction.override_path,
            components
                .into_iter()
                .filter(|c| visualizer_instruction.component_overrides.contains(c)),
        )
    }
}

fn query_overrides_at_path(
    ctx: &ViewerContext<'_>,
    blueprint_path: &re_log_types::EntityPath,
    components: impl IntoIterator<Item = ComponentIdentifier>,
) -> LatestAtResults {
    // First see if any components have overrides.
    let mut overrides = LatestAtResults::empty("<overrides>".into(), ctx.current_query());

    let blueprint_engine = &ctx.store_context.blueprint.storage_engine();

    for component in components {
        // TODO(andreas): Batch these queries?
        let component_override_result = blueprint_engine.cache().latest_at(
            re_chunk_store::ChunkTrackingMode::Report,
            ctx.blueprint_query,
            blueprint_path,
            [component],
        );

        // If we successfully find a non-empty override, add it to our results.
        if let Some(value) = component_override_result.get(component) {
            let index = value.index(ctx.blueprint_query.timeline().as_ref());

            // NOTE: This can never happen, but I'd rather it happens than an unwrap.
            re_log::debug_assert!(index.is_some(), "{value:#?}");
            let index = index.unwrap_or((TimeInt::STATIC, RowId::ZERO));

            overrides.add(component, index, value.clone());
        }
    }
    overrides
}

pub trait DataResultQuery {
    fn latest_at_with_blueprint_resolved_data<'a, A: re_types_core::Archetype>(
        &'a self,
        ctx: &'a ViewContext<'a>,
        latest_at_query: &'a LatestAtQuery,
        visualizer_instruction: Option<&re_viewer_context::VisualizerInstruction>,
    ) -> BlueprintResolvedLatestAtResults<'a>;

    fn latest_at_with_blueprint_resolved_data_for_component<'a>(
        &'a self,
        ctx: &'a ViewContext<'a>,
        latest_at_query: &'a LatestAtQuery,
        component: ComponentIdentifier,
        visualizer_instruction: Option<&re_viewer_context::VisualizerInstruction>,
    ) -> BlueprintResolvedLatestAtResults<'a>;

    /// Queries for the given components, taking into account:
    /// * visible history if enabled
    /// * blueprint overrides & defaults
    fn query_components_with_history<'a>(
        &'a self,
        ctx: &'a ViewContext<'a>,
        view_query: &ViewQuery<'_>,
        component_descriptors: impl IntoIterator<Item = ComponentIdentifier>,
        visualizer_instruction: &re_viewer_context::VisualizerInstruction,
    ) -> BlueprintResolvedResults<'a>;

    /// Queries for all components of an archetype, taking into account:
    /// * visible history if enabled
    /// * blueprint overrides & defaults
    fn query_archetype_with_history<'a, A: Archetype>(
        &'a self,
        ctx: &'a ViewContext<'a>,
        view_query: &ViewQuery<'_>,
        visualizer_instruction: &re_viewer_context::VisualizerInstruction,
    ) -> BlueprintResolvedResults<'a> {
        self.query_components_with_history(
            ctx,
            view_query,
            A::all_component_identifiers(),
            visualizer_instruction,
        )
    }
}

impl DataResultQuery for DataResult {
    fn latest_at_with_blueprint_resolved_data<'a, A: re_types_core::Archetype>(
        &'a self,
        ctx: &'a ViewContext<'a>,
        latest_at_query: &'a LatestAtQuery,
        visualizer_instruction: Option<&re_viewer_context::VisualizerInstruction>,
    ) -> BlueprintResolvedLatestAtResults<'a> {
        latest_at_with_blueprint_resolved_data(
            ctx,
            None,
            latest_at_query,
            self,
            A::all_component_identifiers(),
            visualizer_instruction,
        )
    }

    fn latest_at_with_blueprint_resolved_data_for_component<'a>(
        &'a self,
        ctx: &'a ViewContext<'a>,
        latest_at_query: &'a LatestAtQuery,
        component: ComponentIdentifier,
        visualizer_instruction: Option<&re_viewer_context::VisualizerInstruction>,
    ) -> BlueprintResolvedLatestAtResults<'a> {
        latest_at_with_blueprint_resolved_data(
            ctx,
            None,
            latest_at_query,
            self,
            std::iter::once(component),
            visualizer_instruction,
        )
    }

    fn query_components_with_history<'a>(
        &'a self,
        ctx: &'a ViewContext<'a>,
        view_query: &ViewQuery<'_>,
        components: impl IntoIterator<Item = ComponentIdentifier>,
        visualizer_instruction: &re_viewer_context::VisualizerInstruction,
    ) -> BlueprintResolvedResults<'a> {
        match self.query_range() {
            QueryRange::TimeRange(time_range) => {
                let range_query = RangeQuery::new(
                    view_query.timeline,
                    re_log_types::AbsoluteTimeRange::from_relative_time_range(
                        time_range,
                        view_query.latest_at,
                    ),
                );
                let results = range_with_blueprint_resolved_data(
                    ctx,
                    None,
                    &range_query,
                    self,
                    components,
                    visualizer_instruction,
                );
                (range_query, results).into()
            }
            QueryRange::LatestAt => {
                let latest_query = LatestAtQuery::new(view_query.timeline, view_query.latest_at);
                let results = latest_at_with_blueprint_resolved_data(
                    ctx,
                    None,
                    &latest_query,
                    self,
                    components,
                    Some(visualizer_instruction),
                );
                (latest_query, results).into()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use nohash_hasher::IntMap;
    use re_chunk_store::{LatestAtQuery, RangeQuery, RowId};
    use re_log_types::{
        AbsoluteTimeRange, EntityPath, TimePoint, TimelineName, build_frame_nr,
        external::arrow::datatypes::DataType,
    };
    use re_sdk_types::archetypes;
    use re_sdk_types::blueprint::components::VisualizerInstructionId;
    use re_sdk_types::components::Color;
    use re_test_context::TestContext;
    use re_types_core::{Component as _, ComponentDescriptor, ViewClassIdentifier};
    use re_viewer_context::{
        DataQueryResult, DataResult, QueryRange, ViewContext, ViewId, ViewSystemIdentifier,
        VisualizerComponentMappings, VisualizerComponentSource, VisualizerInstruction,
    };

    use super::{
        ComponentCastRule, latest_at_with_blueprint_resolved_data,
        latest_at_with_blueprint_resolved_data_polymorphic, range_with_blueprint_resolved_data,
        range_with_blueprint_resolved_data_polymorphic,
    };
    use crate::{BlueprintResolvedResults, ComponentMappingError};

    #[test]
    fn mapped_component_without_data_for_query_reports_specific_error() {
        let mut test_context = TestContext::new();
        let egui_ctx = egui::Context::default();
        let entity_path = EntityPath::from("entity");
        let source = archetypes::Scalars::descriptor_scalars().component;
        let target = "target".into();

        test_context.log_entity(entity_path.clone(), |builder| {
            builder.with_archetype_auto_row([build_frame_nr(10)], &archetypes::Scalars::single(1.0))
        });

        let data_result = DataResult {
            entity_path,
            any_visualizers_available: true,
            visualizer_instructions: Vec::new(),
            tree_prefix_only: false,
            visible: true,
            interactive: true,
            override_base_path: EntityPath::from("override"),
            query_range: QueryRange::LatestAt,
        };
        let instruction = VisualizerInstruction::new(
            VisualizerInstructionId::new_random(),
            ViewSystemIdentifier::from_static_str("Test"),
            &EntityPath::from("override"),
            VisualizerComponentMappings::from([(
                target,
                VisualizerComponentSource::SourceComponent {
                    source_component: source,
                    selector: String::new(),
                },
            )]),
        );

        test_context.run(&egui_ctx, move |viewer_ctx| {
            let query_result = DataQueryResult::default();
            let ctx = ViewContext {
                viewer_ctx,
                view_id: ViewId::invalid(),
                view_class_identifier: ViewClassIdentifier::from_static_str("Test"),
                space_origin: &EntityPath::root(),
                view_state: &(),
                query_result: &query_result,
            };

            let latest_query = LatestAtQuery::new(TimelineName::log_tick(), 0);
            let latest_results = latest_at_with_blueprint_resolved_data(
                &ctx,
                None,
                &latest_query,
                &data_result,
                [target],
                Some(&instruction),
            );
            let latest_result = latest_results.component_sources.get(&target);
            let Some(Err(ComponentMappingError::NoComponentDataForQuery(component))) =
                latest_result
            else {
                panic!(
                    "Expected NoComponentDataForQuery from latest-at query, got {latest_result:?}"
                );
            };
            assert_eq!(*component, source);

            let range_query =
                RangeQuery::new(TimelineName::log_tick(), AbsoluteTimeRange::new(0, 0));
            let range_results = range_with_blueprint_resolved_data(
                &ctx,
                None,
                &range_query,
                &data_result,
                [target],
                &instruction,
            );
            let range_result = range_results.component_sources.get(&target);
            let Some(Err(ComponentMappingError::NoComponentDataForQuery(component))) = range_result
            else {
                panic!("Expected NoComponentDataForQuery from range query, got {range_result:?}");
            };
            assert_eq!(*component, source);
        });
    }

    fn test_data_result(entity_path: impl Into<EntityPath>) -> DataResult {
        DataResult {
            entity_path: entity_path.into(),
            any_visualizers_available: true,
            visualizer_instructions: Vec::new(),
            tree_prefix_only: false,
            visible: true,
            interactive: true,
            override_base_path: EntityPath::from("override"),
            query_range: QueryRange::LatestAt,
        }
    }

    fn run_with_test_view_context(test_context: &TestContext, func: impl FnOnce(&ViewContext<'_>)) {
        test_context.run(&egui::Context::default(), |viewer_ctx| {
            let ctx = ViewContext {
                viewer_ctx,
                view_id: ViewId::invalid(),
                view_class_identifier: ViewClassIdentifier::from_static_str("Test"),
                space_origin: &EntityPath::root(),
                view_state: &(),
                query_result: &DataQueryResult::default(),
            };
            func(&ctx);
        });
    }

    // Component mappings are parallel assignments, not sequential transformations.
    //
    // Given A ← B and B ← C, A must resolve to the original B rather than the remapped B
    // for both latest-at and range queries, regardless of mapping iteration order.
    #[test]
    fn chained_mappings_read_original_components_independent_of_mapping_order() {
        let mut test_context = TestContext::new();

        let entity_path = EntityPath::from("entity");
        let descriptors = ["a", "b", "c"]
            .map(|name| ComponentDescriptor::partial(name).with_component_type(Color::name()));
        let [a, b, c] = descriptors
            .each_ref()
            .map(|descriptor| descriptor.component);
        let original_b = Color::from_rgb(1, 2, 3);
        let original_c = Color::from_rgb(4, 5, 6);

        test_context.log_entity(entity_path.clone(), |builder| {
            builder.with_component_batches(
                RowId::new(),
                TimePoint::STATIC,
                [
                    (descriptors[1].clone(), &[original_b] as _),
                    (descriptors[2].clone(), &[original_c] as _),
                ],
            )
        });

        let data_result = test_data_result(entity_path);

        let range_query = RangeQuery::new(TimelineName::log_tick(), AbsoluteTimeRange::EVERYTHING);
        let mapping_pairs = [
            (a, VisualizerComponentSource::simple_map(b)),
            (b, VisualizerComponentSource::simple_map(c)),
        ];

        run_with_test_view_context(&test_context, move |ctx| {
            for mappings in [
                VisualizerComponentMappings::from(mapping_pairs.clone()),
                VisualizerComponentMappings::from([
                    mapping_pairs[1].clone(),
                    mapping_pairs[0].clone(),
                ]),
            ] {
                let instruction = VisualizerInstruction::new(
                    VisualizerInstructionId::new_random(),
                    ViewSystemIdentifier::from_static_str("Test"),
                    &EntityPath::from("override"),
                    mappings,
                );
                let latest = latest_at_with_blueprint_resolved_data(
                    ctx,
                    None,
                    &LatestAtQuery::new_static(),
                    &data_result,
                    [a, b],
                    Some(&instruction),
                );
                assert_eq!(latest.get_mono::<Color>(a), Some(original_b));

                let range = range_with_blueprint_resolved_data(
                    ctx,
                    None,
                    &range_query,
                    &data_result,
                    [a, b],
                    &instruction,
                );
                let range_color = range.store_results.components[&a][0]
                    .iter_component::<Color>(a)
                    .next()
                    .unwrap()
                    .as_slice()[0];
                assert_eq!(range_color, original_b);
            }
        });
    }

    // Make sure that even if there's an identity mapping (which looks like no mapping at all in many regards!),
    // we still apply the cast rules.
    #[test]
    fn identity_mapping_applies_cast_rule() {
        fn cast_to_float64(datatype: &DataType) -> Option<DataType> {
            (datatype == &DataType::UInt32).then_some(DataType::Float64)
        }

        let mut test_context = TestContext::new();
        let entity_path = EntityPath::from("entity");
        let descriptor =
            ComponentDescriptor::partial("identity").with_component_type(Color::name());
        let component = descriptor.component;

        test_context.log_entity(entity_path.clone(), |builder| {
            builder.with_component_batches(
                RowId::new(),
                TimePoint::STATIC,
                [(descriptor, &[Color::from_rgb(1, 2, 3)] as _)],
            )
        });

        let data_result = test_data_result(entity_path);
        let instruction = VisualizerInstruction::new(
            VisualizerInstructionId::new_random(),
            ViewSystemIdentifier::from_static_str("Test"),
            &EntityPath::from("override"),
            VisualizerComponentMappings::from([(
                component,
                VisualizerComponentSource::identity(component),
            )]),
        );
        let cast_rules: IntMap<_, ComponentCastRule> =
            std::iter::once((component, cast_to_float64 as ComponentCastRule)).collect();

        run_with_test_view_context(&test_context, move |ctx| {
            let latest = latest_at_with_blueprint_resolved_data_polymorphic(
                ctx,
                None,
                &LatestAtQuery::new_static(),
                &data_result,
                [component],
                Some(&instruction),
                &cast_rules,
            );
            assert_eq!(
                latest.get_raw_cell(component).unwrap().data_type(),
                &DataType::Float64
            );

            let range = range_with_blueprint_resolved_data_polymorphic(
                ctx,
                None,
                &RangeQuery::new(TimelineName::log_tick(), AbsoluteTimeRange::EVERYTHING),
                &data_result,
                [component],
                &instruction,
                &cast_rules,
            );
            assert_eq!(
                range.store_results.components[&component][0]
                    .components()
                    .get_array(component)
                    .unwrap()
                    .value_type(),
                DataType::Float64
            );
        });
    }

    #[test]
    fn query_result_hash_changes_with_component_mappings() {
        let test_context = TestContext::new();

        let data_result = test_data_result("entity");

        let target_a = "target_a".into();
        let target_b = "target_b".into();
        let source_a = "source_a".into();
        let source_b = "source_b".into();

        let instruction_id = VisualizerInstructionId::new_random();

        let test_mappings = [
            ("empty", VisualizerComponentMappings::default()),
            (
                "override",
                VisualizerComponentMappings::from([(
                    target_a,
                    VisualizerComponentSource::Override,
                )]),
            ),
            (
                "default",
                VisualizerComponentMappings::from([(target_a, VisualizerComponentSource::Default)]),
            ),
            (
                "source",
                VisualizerComponentMappings::from([(
                    target_a,
                    VisualizerComponentSource::simple_map(source_a),
                )]),
            ),
            (
                "selector",
                VisualizerComponentMappings::from([(
                    target_a,
                    VisualizerComponentSource::SourceComponent {
                        source_component: source_a,
                        selector: "$.field".to_owned(),
                    },
                )]),
            ),
            (
                "other source",
                VisualizerComponentMappings::from([(
                    target_a,
                    VisualizerComponentSource::SourceComponent {
                        source_component: source_b,
                        selector: "$.field".to_owned(),
                    },
                )]),
            ),
            (
                "other target",
                VisualizerComponentMappings::from([(
                    target_b,
                    VisualizerComponentSource::SourceComponent {
                        source_component: source_b,
                        selector: "$.field".to_owned(),
                    },
                )]),
            ),
        ];

        run_with_test_view_context(&test_context, move |ctx| {
            let range_query =
                RangeQuery::new(TimelineName::log_tick(), AbsoluteTimeRange::EVERYTHING);

            let query_hashes = |component_mappings: VisualizerComponentMappings| {
                let instruction = VisualizerInstruction::new(
                    instruction_id,
                    ViewSystemIdentifier::from_static_str("Test"),
                    &EntityPath::from("override"),
                    component_mappings,
                );
                let latest_results = latest_at_with_blueprint_resolved_data(
                    ctx,
                    None,
                    &ctx.current_query(),
                    &data_result,
                    [target_a, target_b],
                    Some(&instruction),
                );
                let range_results = range_with_blueprint_resolved_data(
                    ctx,
                    None,
                    &range_query,
                    &data_result,
                    [target_a, target_b],
                    &instruction,
                );

                (
                    BlueprintResolvedResults::from((ctx.current_query(), latest_results))
                        .query_result_hash(),
                    BlueprintResolvedResults::from((range_query.clone(), range_results))
                        .query_result_hash(),
                )
            };

            for pair in test_mappings
                .into_iter()
                .map(|(name, mappings)| (name, query_hashes(mappings)))
                .collect::<Vec<_>>()
                .windows(2)
            {
                let [(previous_name, previous), (current_name, current)] = pair else {
                    unreachable!();
                };
                assert_ne!(
                    previous.0, current.0,
                    "Latest-at hash did not change from {previous_name} to {current_name}"
                );
                assert_ne!(
                    previous.1, current.1,
                    "Range hash did not change from {previous_name} to {current_name}"
                );
            }
        });
    }
}
