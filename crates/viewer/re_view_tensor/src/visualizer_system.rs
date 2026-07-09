use re_chunk_store::{LatestAtQuery, RangeQuery, RowId};
use re_log_types::AbsoluteTimeRange;
use re_log_types::hash::Hash64;
use re_sdk_types::Archetype as _;
use re_sdk_types::archetypes::Tensor;
use re_sdk_types::components::{TensorData, ValueRange};
use re_sdk_types::datatypes::TensorBuffer;
use re_view::latest_at_with_blueprint_resolved_data;
use re_viewer_context::{
    IdentifiedViewSystem, ViewContext, ViewContextCollection, ViewQuery, ViewSystemExecutionError,
    VisualizerExecutionOutput, VisualizerQueryInfo, VisualizerReportSeverity, VisualizerSystem,
    typed_fallback_for,
};

#[derive(Clone, re_byte_size::SizeBytes)]
pub struct TensorVisualization {
    pub tensor_cache_key: Hash64,
    pub tensor_row_id: Option<RowId>,
    // Tensor is already counted as part of the store.
    #[size_bytes(ignore)]
    pub tensor: TensorData,
    pub data_range: ValueRange,
}

#[derive(Default)]
pub struct TensorSystem;

impl IdentifiedViewSystem for TensorSystem {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        re_viewer_context::external::re_string_interner::intern_static!(
            re_viewer_context::ViewSystemIdentifier,
            "Tensor"
        )
    }
}

#[derive(Default)]
pub struct SignalHeatmapSystem;

impl IdentifiedViewSystem for SignalHeatmapSystem {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        re_viewer_context::external::re_string_interner::intern_static!(
            re_viewer_context::ViewSystemIdentifier,
            "SignalHeatmap"
        )
    }
}

impl VisualizerSystem for SignalHeatmapSystem {
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo::single_required_component::<TensorData>(
            &Tensor::descriptor_data(),
            &Tensor::all_components(),
        )
    }

    fn execute(
        &self,
        ctx: &ViewContext<'_>,
        query: &ViewQuery<'_>,
        _context_systems: &ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        re_tracing::profile_function!();

        let output = VisualizerExecutionOutput::default();
        let mut tensors = Vec::new();

        for (data_result, instruction) in query.iter_visualizer_instruction_for(Self::identifier())
        {
            let range_query = match data_result.query_range() {
                re_viewer_context::QueryRange::TimeRange(time_range) => {
                    let current_time = ctx
                        .viewer_ctx
                        .time_ctrl
                        .time_int()
                        .unwrap_or(re_log_types::TimeInt::ZERO);
                    RangeQuery::new(
                        query.timeline,
                        AbsoluteTimeRange::from_relative_time_range(time_range, current_time),
                    )
                }
                re_viewer_context::QueryRange::LatestAt => {
                    RangeQuery::new(query.timeline, AbsoluteTimeRange::EVERYTHING)
                }
            };

            let range_results = re_view::range_with_blueprint_resolved_data(
                ctx,
                None,
                &range_query,
                data_result,
                Tensor::all_component_identifiers(),
                instruction,
            );
            let results =
                re_view::BlueprintResolvedResults::Range(range_query.clone(), range_results);
            let results =
                re_view::VisualizerInstructionQueryResults::new(instruction, &results, &output);

            let all_tensor_chunks = results.iter_required(Tensor::descriptor_data().component);
            if all_tensor_chunks.is_empty() {
                continue;
            }

            let mut tensor_rows = Vec::new();
            for chunk in all_tensor_chunks.chunks().iter() {
                for ((_time, tensor_row_id), tensor_values) in std::iter::zip(
                    chunk.iter_component_indices(query.timeline),
                    chunk.iter_component::<TensorData>(),
                ) {
                    for tensor in tensor_values.iter() {
                        tensor_rows.push((tensor_row_id, tensor.clone()));
                    }
                }
            }

            if tensor_rows.is_empty() {
                continue;
            }

            let tensors_to_concat = tensor_rows
                .iter()
                .map(|(_, tensor)| tensor)
                .collect::<Vec<_>>();
            let tensor_cache_key = Hash64::hash(
                tensor_rows
                    .iter()
                    .map(|(row_id, _)| *row_id)
                    .collect::<Vec<_>>(),
            );

            let tensor = match concatenate_tensors_along_first_dimension(&tensors_to_concat) {
                Ok(tensor) => tensor,
                Err(err) => {
                    output.report_unspecified_source(
                        instruction.id,
                        VisualizerReportSeverity::Warning,
                        format!("Could not concatenate signal heatmap tensor chunks: {err}"),
                    );
                    continue;
                }
            };

            let data_range = results
                .iter_optional(Tensor::descriptor_value_range().component)
                .slice::<[f64; 2]>()
                .last()
                .and_then(|(_, ranges)| ranges.first().copied())
                .map(|range| ValueRange(range.into()))
                .unwrap_or_else(|| {
                    typed_fallback_for(
                        &ctx.query_context(data_result, query.latest_at_query(), instruction.id),
                        Tensor::descriptor_value_range().component,
                    )
                });

            tensors.push(TensorVisualization {
                tensor_cache_key,
                tensor_row_id: tensor_rows.last().map(|(row_id, _)| *row_id),
                tensor,
                data_range,
            });
        }

        Ok(output.with_visualizer_data(tensors))
    }
}

fn concatenate_tensors_along_first_dimension(
    tensors: &[&TensorData],
) -> Result<TensorData, String> {
    let Some(first) = tensors.first() else {
        return Err("no tensor chunks".to_owned());
    };

    if first.shape().is_empty() {
        return Err("expected tensors with at least one dimension".to_owned());
    }

    let mut shape = first.shape().to_vec();
    shape[0] = 0;

    for tensor in tensors {
        if tensor.dtype() != first.dtype() {
            return Err("all chunks must use the same tensor datatype".to_owned());
        }
        if tensor.shape().len() != first.shape().len() || tensor.shape()[1..] != first.shape()[1..]
        {
            return Err(
                "all chunks must have matching non-time dimensions to concatenate".to_owned(),
            );
        }
        shape[0] += tensor.shape()[0];
    }

    let names = first.names.clone();
    let buffer = concatenate_tensor_buffers(tensors)?;

    Ok(TensorData(re_sdk_types::datatypes::TensorData {
        shape: shape.into(),
        names,
        buffer,
    }))
}

fn concatenate_tensor_buffers(tensors: &[&TensorData]) -> Result<TensorBuffer, String> {
    macro_rules! concat_variant {
        ($variant:ident) => {{
            let total_len = tensors
                .iter()
                .map(|tensor| match &tensor.buffer {
                    TensorBuffer::$variant(buffer) => Ok(buffer.len()),
                    _ => Err("all chunks must use the same tensor datatype".to_owned()),
                })
                .sum::<Result<usize, String>>()?;
            let mut values = Vec::with_capacity(total_len);
            for tensor in tensors {
                let TensorBuffer::$variant(buffer) = &tensor.buffer else {
                    return Err("all chunks must use the same tensor datatype".to_owned());
                };
                values.extend(buffer.iter().copied());
            }
            Ok(TensorBuffer::$variant(values.into()))
        }};
    }

    match &tensors[0].buffer {
        TensorBuffer::U8(_) => concat_variant!(U8),
        TensorBuffer::U16(_) => concat_variant!(U16),
        TensorBuffer::U32(_) => concat_variant!(U32),
        TensorBuffer::U64(_) => concat_variant!(U64),
        TensorBuffer::I8(_) => concat_variant!(I8),
        TensorBuffer::I16(_) => concat_variant!(I16),
        TensorBuffer::I32(_) => concat_variant!(I32),
        TensorBuffer::I64(_) => concat_variant!(I64),
        TensorBuffer::F16(_) => concat_variant!(F16),
        TensorBuffer::F32(_) => concat_variant!(F32),
        TensorBuffer::F64(_) => concat_variant!(F64),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concatenates_tensor_chunks_along_first_dimension() {
        let first = TensorData(
            re_sdk_types::datatypes::TensorData::new(
                vec![2, 3],
                TensorBuffer::F32(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0].into()),
            )
            .with_dim_names(["time", "frequency"]),
        );
        let second = TensorData(
            re_sdk_types::datatypes::TensorData::new(
                vec![1, 3],
                TensorBuffer::F32(vec![7.0, 8.0, 9.0].into()),
            )
            .with_dim_names(["time", "frequency"]),
        );

        let concatenated = concatenate_tensors_along_first_dimension(&[&first, &second]).unwrap();

        assert_eq!(concatenated.shape(), &[3, 3]);
        assert_eq!(
            concatenated.dim_name(0).map(|name| name.as_str()),
            Some("time")
        );
        assert_eq!(
            concatenated.dim_name(1).map(|name| name.as_str()),
            Some("frequency")
        );
        assert_eq!(
            concatenated.buffer,
            TensorBuffer::F32(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0].into())
        );
    }

    #[test]
    fn rejects_incompatible_tensor_chunks() {
        let first = TensorData(re_sdk_types::datatypes::TensorData::new(
            vec![2, 3],
            TensorBuffer::F32(vec![0.0; 6].into()),
        ));
        let second = TensorData(re_sdk_types::datatypes::TensorData::new(
            vec![2, 4],
            TensorBuffer::F32(vec![0.0; 8].into()),
        ));

        let err = concatenate_tensors_along_first_dimension(&[&first, &second]).unwrap_err();
        assert!(err.contains("matching non-time dimensions"));
    }
}

impl VisualizerSystem for TensorSystem {
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo::single_required_component::<TensorData>(
            &Tensor::descriptor_data(),
            &Tensor::all_components(),
        )
    }

    fn execute(
        &self,
        ctx: &ViewContext<'_>,
        query: &ViewQuery<'_>,
        _context_systems: &ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        re_tracing::profile_function!();

        let output = VisualizerExecutionOutput::default();
        let mut tensors = Vec::new();

        for (data_result, instruction) in query.iter_visualizer_instruction_for(Self::identifier())
        {
            let timeline_query = LatestAtQuery::new(query.timeline, query.latest_at);

            let annotations = None;
            let latest_at_results = latest_at_with_blueprint_resolved_data(
                ctx,
                annotations,
                &timeline_query,
                data_result,
                Tensor::all_component_identifiers(),
                Some(instruction),
            );
            let results =
                re_view::BlueprintResolvedResults::from((timeline_query, latest_at_results));
            let results =
                re_view::VisualizerInstructionQueryResults::new(instruction, &results, &output);

            let all_tensor_chunks = results.iter_required(Tensor::descriptor_data().component);
            if all_tensor_chunks.is_empty() {
                continue;
            }

            let all_tensors_indexed = all_tensor_chunks.chunks().iter().flat_map(move |chunk| {
                std::iter::zip(
                    chunk.iter_component_indices(query.timeline),
                    chunk.iter_component::<TensorData>(),
                )
            });
            let all_ranges = results.iter_optional(Tensor::descriptor_value_range().component);

            for ((_, tensor_row_id), tensor_values, data_ranges) in
                re_query::range_zip_1x1(all_tensors_indexed, all_ranges.slice::<[f64; 2]>())
            {
                let Some(tensor) = tensor_values.first() else {
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
                            &ctx.query_context(
                                data_result,
                                query.latest_at_query(),
                                instruction.id,
                            ),
                            Tensor::descriptor_value_range().component,
                        )
                    });

                tensors.push(TensorVisualization {
                    tensor_cache_key: Hash64::hash(tensor_row_id),
                    tensor_row_id: Some(tensor_row_id),
                    tensor: tensor.clone(),
                    data_range,
                });
            }
        }

        Ok(output.with_visualizer_data(tensors))
    }
}
