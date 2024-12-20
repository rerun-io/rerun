use re_chunk_store::{LatestAtQuery, RowId};
use re_types::{
    archetypes::Tensor,
    components::{TensorData, ValueRange},
    Component as _,
};
use re_view::{latest_at_with_blueprint_resolved_data, RangeResultsExt};
use re_viewer_context::{
    IdentifiedViewSystem, TensorStats, TensorStatsCache, TypedComponentFallbackProvider,
    ViewContext, ViewContextCollection, ViewQuery, ViewSystemExecutionError, VisualizerQueryInfo,
    VisualizerSystem,
};

#[derive(Clone)]
pub struct TensorVisualization {
    pub tensor_row_id: RowId,
    pub tensor: TensorData,
    pub data_range: ValueRange,
}

#[derive(Default)]
pub struct TensorSystem {
    pub tensors: Vec<TensorVisualization>,
}

impl IdentifiedViewSystem for TensorSystem {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Tensor".into()
    }
}

impl VisualizerSystem for TensorSystem {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<Tensor>()
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        query: &ViewQuery<'_>,
        _context_systems: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, ViewSystemExecutionError> {
        re_tracing::profile_function!();

        for data_result in query.iter_visible_data_results(ctx, Self::identifier()) {
            let timeline_query = LatestAtQuery::new(query.timeline, query.latest_at);

            let annotations = None;
            let query_shadowed_defaults = false;
            let results = latest_at_with_blueprint_resolved_data(
                ctx,
                annotations,
                &timeline_query,
                data_result,
                [TensorData::name(), ValueRange::name()].into_iter(),
                query_shadowed_defaults,
            );

            let Some(all_tensor_chunks) = results.get_required_chunks(&TensorData::name()) else {
                continue;
            };

            let timeline = query.timeline;
            let all_tensors_indexed = all_tensor_chunks.iter().flat_map(move |chunk| {
                chunk
                    .iter_component_indices(&timeline, &TensorData::name())
                    .zip(chunk.iter_component::<TensorData>())
            });
            let all_ranges = results.iter_as(timeline, ValueRange::name());

            for ((_, tensor_row_id), tensors, data_ranges) in
                re_query::range_zip_1x1(all_tensors_indexed, all_ranges.primitive_array::<2, f64>())
            {
                let Some(tensor) = tensors.first() else {
                    continue;
                };
                let data_range = data_ranges
                    .and_then(|ranges| {
                        ranges
                            .first()
                            .copied()
                            .map(|range| ValueRange(range.into()))
                    })
                    .unwrap_or_else(|| {
                        let tensor_stats = ctx
                            .viewer_ctx
                            .cache
                            .entry(|c: &mut TensorStatsCache| c.entry(tensor_row_id, tensor));
                        tensor_data_range_heuristic(&tensor_stats, tensor.dtype())
                    });

                self.tensors.push(TensorVisualization {
                    tensor_row_id,
                    tensor: tensor.clone(),
                    data_range,
                });
            }
        }

        Ok(Vec::new())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn fallback_provider(&self) -> &dyn re_viewer_context::ComponentFallbackProvider {
        self
    }
}

/// Get a valid, finite range for the gpu to use.
pub fn tensor_data_range_heuristic(
    tensor_stats: &TensorStats,
    data_type: re_types::tensor_data::TensorDataType,
) -> ValueRange {
    let (min, max) = tensor_stats.finite_range;

    // Apply heuristic for ranges that are typically expected depending on the data type and the finite (!) range.
    // (we ignore NaN/Inf values heres, since they are usually there by accident!)
    #[allow(clippy::tuple_array_conversions)]
    ValueRange::from(if data_type.is_float() && 0.0 <= min && max <= 1.0 {
        // Float values that are all between 0 and 1, assume that this is the range.
        [0.0, 1.0]
    } else if 0.0 <= min && max <= 255.0 {
        // If all values are between 0 and 255, assume this is the range.
        // (This is very common, independent of the data type)
        [0.0, 255.0]
    } else if min == max {
        // uniform range. This can explode the colormapping, so let's map all colors to the middle:
        [min - 1.0, max + 1.0]
    } else {
        // Use range as is if nothing matches.
        [min, max]
    })
}

impl TypedComponentFallbackProvider<re_types::components::ValueRange> for TensorSystem {
    fn fallback_for(
        &self,
        ctx: &re_viewer_context::QueryContext<'_>,
    ) -> re_types::components::ValueRange {
        if let Some(((_time, row_id), tensor)) = ctx
            .recording()
            .latest_at_component::<TensorData>(ctx.target_entity_path, ctx.query)
        {
            let tensor_stats = ctx
                .viewer_ctx
                .cache
                .entry(|c: &mut TensorStatsCache| c.entry(row_id, &tensor));
            tensor_data_range_heuristic(&tensor_stats, tensor.dtype())
        } else {
            ValueRange::new(0.0, 1.0)
        }
    }
}

re_viewer_context::impl_component_fallback_provider!(TensorSystem => [re_types::components::ValueRange]);
