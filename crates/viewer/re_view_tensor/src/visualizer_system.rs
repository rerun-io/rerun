use std::sync::Arc;

use re_chunk_store::{LatestAtQuery, RowId};
use re_log_types::EntityPath;
use re_sdk_types::Archetype as _;
use re_sdk_types::archetypes::Tensor;
use re_sdk_types::components::{Opacity, TensorData, ValueRange};
use re_view::{RangeResultsExt as _, latest_at_with_blueprint_resolved_data};
use re_viewer_context::{
    AnnotationMap, Annotations, IdentifiedViewSystem, ViewContext, ViewContextCollection, ViewQuery,
    ViewSystemExecutionError, VisualizerExecutionOutput, VisualizerQueryInfo, VisualizerSystem,
    typed_fallback_for,
};

#[derive(Clone)]
pub struct TensorVisualization {
    pub entity_path: EntityPath,
    pub tensor_row_id: RowId,
    pub tensor: TensorData,
    pub data_range: ValueRange,
    pub annotations: Arc<Annotations>,
    pub opacity: f32,
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
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        re_tracing::profile_function!();

        let timeline_query = LatestAtQuery::new(query.timeline, query.latest_at);
        let mut annotation_map = AnnotationMap::default();
        annotation_map.load(ctx.viewer_ctx, &timeline_query);

        for data_result in query.iter_visible_data_results(Self::identifier()) {
            let annotations = annotation_map.find(&data_result.entity_path);
            let query_shadowed_defaults = false;
            let results = latest_at_with_blueprint_resolved_data(
                ctx,
                Some(&annotations),
                &timeline_query,
                data_result,
                Tensor::all_component_identifiers(),
                query_shadowed_defaults,
            );

            let Some(all_tensor_chunks) =
                results.get_required_chunks(Tensor::descriptor_data().component)
            else {
                continue;
            };

            let timeline = query.timeline;
            let all_tensors_indexed = all_tensor_chunks.iter().flat_map(move |chunk| {
                chunk
                    .iter_component_indices(timeline)
                    .zip(chunk.iter_component::<TensorData>())
            });
            let all_ranges = results.iter_as(timeline, Tensor::descriptor_value_range().component);
            let all_opacities = results.iter_as(timeline, Tensor::descriptor_opacity().component);

            for ((_, tensor_row_id), tensors, data_ranges, opacities) in re_query::range_zip_1x2(
                all_tensors_indexed,
                all_ranges.slice::<[f64; 2]>(),
                all_opacities.slice::<f32>(),
            ) {
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
                        typed_fallback_for(
                            &ctx.query_context(data_result, &query.latest_at_query()),
                            Tensor::descriptor_value_range().component,
                        )
                    });

                let opacity = opacities
                    .and_then(|ops| ops.first().copied().map(Opacity::from))
                    .unwrap_or_else(|| {
                        typed_fallback_for(
                            &ctx.query_context(data_result, &query.latest_at_query()),
                            Tensor::descriptor_opacity().component,
                        )
                    });

                self.tensors.push(TensorVisualization {
                    entity_path: data_result.entity_path.clone(),
                    tensor_row_id,
                    tensor: tensor.clone(),
                    data_range,
                    annotations: annotations.clone(),
                    opacity: *opacity.0,
                });
            }
        }

        Ok(VisualizerExecutionOutput::default())
    }
}
