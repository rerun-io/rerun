use re_chunk_store::{LatestAtQuery, RowId};
use re_sdk_types::Archetype as _;
use re_sdk_types::archetypes::Tensor;
use re_sdk_types::components::{TensorData, ValueRange};
use re_view::{RangeResultsExt as _, latest_at_with_blueprint_resolved_data};
use re_viewer_context::{
    IdentifiedViewSystem, ViewContext, ViewContextCollection, ViewQuery, ViewSystemExecutionError,
    VisualizerExecutionOutput, VisualizerQueryInfo, VisualizerSystem, typed_fallback_for,
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
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        re_tracing::profile_function!();

        for (data_result, instruction) in query.iter_visualizer_instruction_for(Self::identifier())
        {
            let timeline_query = LatestAtQuery::new(query.timeline, query.latest_at);

            let annotations = None;
            let query_shadowed_defaults = false;
            let results = latest_at_with_blueprint_resolved_data(
                ctx,
                annotations,
                &timeline_query,
                data_result,
                Tensor::all_component_identifiers(),
                query_shadowed_defaults,
                instruction,
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

            for ((_, tensor_row_id), tensors, data_ranges) in
                re_query::range_zip_1x1(all_tensors_indexed, all_ranges.slice::<[f64; 2]>())
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
                        typed_fallback_for(
                            &ctx.query_context(data_result, &query.latest_at_query()),
                            Tensor::descriptor_value_range().component,
                        )
                    });

                self.tensors.push(TensorVisualization {
                    tensor_row_id,
                    tensor: tensor.clone(),
                    data_range,
                });
            }
        }

        Ok(VisualizerExecutionOutput::default())
    }
}
