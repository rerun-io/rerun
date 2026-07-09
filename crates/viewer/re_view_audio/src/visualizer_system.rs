use std::collections::BTreeMap;

use re_chunk_store::{RangeQuery, RowId};
use re_log_types::{AbsoluteTimeRange, TimeInt};
use re_sdk_types::Archetype as _;
use re_sdk_types::{
    archetypes::{AudioAnnotation, AudioClip},
    components::{Color, Range1D, SampleRate, TensorData, Text},
    datatypes::TensorBuffer,
};
use re_viewer_context::{
    IdentifiedViewSystem, QueryRange, ViewContext, ViewContextCollection, ViewQuery,
    ViewSystemExecutionError, VisualizerExecutionOutput, VisualizerQueryInfo,
    VisualizerReportSeverity, VisualizerSystem,
};

#[derive(Clone, Debug, re_byte_size::SizeBytes)]
pub struct AudioChunk {
    pub row_id: RowId,
    pub start_time: TimeInt,
    pub sample_rate: f64,
    pub channels: Vec<Vec<f64>>,
}

#[derive(Clone, Debug, Default, re_byte_size::SizeBytes)]
pub struct AudioWaveform {
    pub chunks: Vec<AudioChunk>,
    pub channel_names: Vec<String>,
}

#[derive(Clone, Debug, re_byte_size::SizeBytes)]
pub struct AudioAnnotationSpan {
    pub start_time: TimeInt,
    pub end_time: TimeInt,
    pub text: String,
    pub color: Option<Color>,
}

impl AudioWaveform {
    pub fn num_channels(&self) -> usize {
        self.chunks
            .iter()
            .map(|chunk| chunk.channels.len())
            .max()
            .unwrap_or(0)
    }

    pub fn time_range_ns(&self) -> Option<(f64, f64)> {
        let mut min = f64::INFINITY;
        let mut max = f64::NEG_INFINITY;

        for chunk in &self.chunks {
            let start = chunk.start_time.as_f64();
            let samples = chunk
                .channels
                .iter()
                .map(Vec::len)
                .max()
                .unwrap_or_default();
            let end = start + samples as f64 / chunk.sample_rate * 1_000_000_000.0;
            min = min.min(start);
            max = max.max(end);
        }

        min.is_finite().then_some((min, max))
    }
}

#[derive(Default)]
pub struct AudioAnnotationSystem;

impl IdentifiedViewSystem for AudioAnnotationSystem {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        re_viewer_context::external::re_string_interner::intern_static!(
            re_viewer_context::ViewSystemIdentifier,
            "AudioAnnotation"
        )
    }
}

impl VisualizerSystem for AudioAnnotationSystem {
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo::single_required_component::<Range1D>(
            &AudioAnnotation::descriptor_span(),
            &AudioAnnotation::all_components(),
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
        let mut annotations = Vec::new();

        for (data_result, instruction) in query.iter_visualizer_instruction_for(Self::identifier())
        {
            let range_query = match data_result.query_range() {
                QueryRange::TimeRange(time_range) => {
                    let current_time = ctx.viewer_ctx.time_ctrl.time_int().unwrap_or(TimeInt::ZERO);
                    RangeQuery::new(
                        query.timeline,
                        AbsoluteTimeRange::from_relative_time_range(time_range, current_time),
                    )
                }
                QueryRange::LatestAt => {
                    RangeQuery::new(query.timeline, AbsoluteTimeRange::EVERYTHING)
                }
            };

            let range_results = re_view::range_with_blueprint_resolved_data(
                ctx,
                None,
                &range_query,
                data_result,
                AudioAnnotation::all_component_identifiers(),
                instruction,
            );
            let results =
                re_view::BlueprintResolvedResults::Range(range_query.clone(), range_results);
            let results =
                re_view::VisualizerInstructionQueryResults::new(instruction, &results, &output);

            let span_chunks = results.iter_required(AudioAnnotation::descriptor_span().component);
            let text_chunks = results.iter_required(AudioAnnotation::descriptor_text().component);
            if span_chunks.is_empty() || text_chunks.is_empty() {
                continue;
            }

            let mut spans = Vec::new();
            for chunk in span_chunks.chunks().iter() {
                for ((time, _row_id), ranges) in std::iter::zip(
                    chunk.iter_component_indices(query.timeline),
                    chunk.iter_component::<Range1D>(),
                ) {
                    spans.extend(ranges.iter().map(|range| (time, *range)));
                }
            }

            let mut texts = Vec::new();
            for chunk in text_chunks.chunks().iter() {
                for text_values in chunk.iter_component::<Text>() {
                    texts.extend(text_values.iter().map(|text| text.as_str().to_owned()));
                }
            }

            let mut colors = Vec::new();
            for chunk in results
                .iter_optional(AudioAnnotation::descriptor_color().component)
                .chunks()
                .iter()
            {
                for color_values in chunk.iter_component::<Color>() {
                    colors.extend(color_values.iter().copied());
                }
            }

            for (idx, (row_time, span)) in spans.into_iter().enumerate() {
                let text = texts.get(idx).cloned().unwrap_or_default();
                let [start_seconds, end_seconds] = span.0.0;
                let start_time = TimeInt::new_temporal(
                    row_time
                        .as_i64()
                        .saturating_add((start_seconds * 1_000_000_000.0).round() as i64),
                );
                let end_time = TimeInt::new_temporal(
                    row_time
                        .as_i64()
                        .saturating_add((end_seconds * 1_000_000_000.0).round() as i64),
                );

                annotations.push(AudioAnnotationSpan {
                    start_time,
                    end_time,
                    text,
                    color: colors.get(idx).copied(),
                });
            }
        }

        Ok(output.with_visualizer_data(annotations))
    }
}

#[derive(Default)]
pub struct AudioVisualizerSystem;

impl IdentifiedViewSystem for AudioVisualizerSystem {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        re_viewer_context::external::re_string_interner::intern_static!(
            re_viewer_context::ViewSystemIdentifier,
            "Audio"
        )
    }
}

impl VisualizerSystem for AudioVisualizerSystem {
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo::single_required_component::<TensorData>(
            &AudioClip::descriptor_samples(),
            &AudioClip::all_components(),
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
        let mut waveforms = BTreeMap::new();

        for (data_result, instruction) in query.iter_visualizer_instruction_for(Self::identifier())
        {
            let range_query = match data_result.query_range() {
                QueryRange::TimeRange(time_range) => {
                    let current_time = ctx.viewer_ctx.time_ctrl.time_int().unwrap_or(TimeInt::ZERO);
                    RangeQuery::new(
                        query.timeline,
                        AbsoluteTimeRange::from_relative_time_range(time_range, current_time),
                    )
                }
                QueryRange::LatestAt => {
                    RangeQuery::new(query.timeline, AbsoluteTimeRange::EVERYTHING)
                }
            };

            let range_results = re_view::range_with_blueprint_resolved_data(
                ctx,
                None,
                &range_query,
                data_result,
                AudioClip::all_component_identifiers(),
                instruction,
            );
            let results =
                re_view::BlueprintResolvedResults::Range(range_query.clone(), range_results);
            let results =
                re_view::VisualizerInstructionQueryResults::new(instruction, &results, &output);

            let sample_chunks = results.iter_required(AudioClip::descriptor_samples().component);
            let rate_chunks = results.iter_required(AudioClip::descriptor_sample_rate().component);
            if sample_chunks.is_empty() || rate_chunks.is_empty() {
                continue;
            }

            let mut audio_chunks = Vec::new();
            let mut sample_rows = Vec::new();
            for chunk in sample_chunks.chunks().iter() {
                for ((time, row_id), tensors) in std::iter::zip(
                    chunk.iter_component_indices(query.timeline),
                    chunk.iter_component::<TensorData>(),
                ) {
                    for tensor in tensors.iter() {
                        sample_rows.push((time, row_id, tensor.clone()));
                    }
                }
            }

            let mut rates = Vec::new();
            for chunk in rate_chunks.chunks().iter() {
                for ((_time, _row_id), sample_rates) in std::iter::zip(
                    chunk.iter_component_indices(query.timeline),
                    chunk.iter_component::<SampleRate>(),
                ) {
                    rates.extend(sample_rates.iter().copied());
                }
            }

            for (row_idx, (start_time, row_id, tensor)) in sample_rows.into_iter().enumerate() {
                let sample_rate = rates
                    .get(row_idx)
                    .or_else(|| rates.last())
                    .map(|rate| rate.0.0)
                    .unwrap_or_default();

                match tensor_to_channels(&tensor, sample_rate) {
                    Ok(channels) => {
                        audio_chunks.push(AudioChunk {
                            row_id,
                            start_time,
                            sample_rate,
                            channels,
                        });
                    }
                    Err(err) => output.report_unspecified_source(
                        instruction.id,
                        VisualizerReportSeverity::Warning,
                        format!("Could not display audio clip: {err}"),
                    ),
                }
            }

            if audio_chunks.is_empty() {
                continue;
            }

            let channel_names = results
                .iter_optional(AudioClip::descriptor_channel_names().component)
                .slice::<String>()
                .last()
                .map(|(_, names)| names.iter().map(ToString::to_string).collect())
                .unwrap_or_default();

            waveforms.insert(
                data_result.entity_path.clone(),
                AudioWaveform {
                    chunks: audio_chunks,
                    channel_names,
                },
            );
        }

        Ok(output.with_visualizer_data(waveforms))
    }
}

fn tensor_to_channels(tensor: &TensorData, sample_rate: f64) -> Result<Vec<Vec<f64>>, String> {
    if !sample_rate.is_finite() || sample_rate <= 0.0 {
        return Err("sample_rate must be a positive finite value".to_owned());
    }

    let shape = tensor.shape();
    let (num_samples, num_channels) = match shape {
        [samples] => (*samples as usize, 1),
        [samples, channels] => (*samples as usize, *channels as usize),
        _ => return Err("expected a 1D mono tensor or 2D [sample, channel] tensor".to_owned()),
    };

    if num_samples == 0 || num_channels == 0 {
        return Err("audio tensor must contain at least one sample and channel".to_owned());
    }

    let expected_len = num_samples
        .checked_mul(num_channels)
        .ok_or_else(|| "audio tensor shape is too large".to_owned())?;

    let values = tensor_buffer_to_f64(&tensor.buffer);
    if values.len() != expected_len {
        return Err("audio tensor shape does not match its buffer length".to_owned());
    }

    let mut channels = vec![Vec::with_capacity(num_samples); num_channels];
    if num_channels == 1 {
        channels[0].extend(values);
    } else {
        for sample_idx in 0..num_samples {
            let offset = sample_idx * num_channels;
            for channel_idx in 0..num_channels {
                channels[channel_idx].push(values[offset + channel_idx]);
            }
        }
    }

    Ok(channels)
}

fn tensor_buffer_to_f64(buffer: &TensorBuffer) -> Vec<f64> {
    macro_rules! collect_as_f64 {
        ($values:expr) => {
            $values.iter().map(|value| (*value).into()).collect()
        };
    }

    match buffer {
        TensorBuffer::U8(values) => collect_as_f64!(values),
        TensorBuffer::U16(values) => collect_as_f64!(values),
        TensorBuffer::U32(values) => collect_as_f64!(values),
        TensorBuffer::U64(values) => values.iter().map(|value| *value as f64).collect(),
        TensorBuffer::I8(values) => collect_as_f64!(values),
        TensorBuffer::I16(values) => collect_as_f64!(values),
        TensorBuffer::I32(values) => collect_as_f64!(values),
        TensorBuffer::I64(values) => values.iter().map(|value| *value as f64).collect(),
        TensorBuffer::F16(values) => values
            .iter()
            .map(|value| f32::from(*value) as f64)
            .collect(),
        TensorBuffer::F32(values) => values.iter().map(|value| *value as f64).collect(),
        TensorBuffer::F64(values) => values.to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_interleaved_stereo_tensor_to_channels() {
        let tensor = TensorData(
            re_sdk_types::datatypes::TensorData::new(
                vec![3, 2],
                TensorBuffer::F32(vec![1.0, 10.0, 2.0, 20.0, 3.0, 30.0].into()),
            )
            .with_dim_names(["sample", "channel"]),
        );

        let channels = tensor_to_channels(&tensor, 16_000.0).unwrap();

        assert_eq!(channels, vec![vec![1.0, 2.0, 3.0], vec![10.0, 20.0, 30.0]]);
    }

    #[test]
    fn rejects_non_audio_shapes() {
        let tensor = TensorData(re_sdk_types::datatypes::TensorData::new(
            vec![2, 2, 2],
            TensorBuffer::F32(vec![0.0; 8].into()),
        ));

        let err = tensor_to_channels(&tensor, 16_000.0).unwrap_err();
        assert!(err.contains("1D mono tensor or 2D"));
    }
}
