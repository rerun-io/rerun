//! Shared functionality for querying time series data.

use itertools::Itertools as _;

use re_chunk_store::RangeQuery;
use re_log_types::TimeInt;
use re_log_types::external::arrow::array::{self, BooleanArray};
use re_log_types::external::arrow::buffer::BooleanBuffer;
use re_sdk_types::external::arrow::datatypes::DataType as ArrowDatatype;
use re_sdk_types::{ComponentDescriptor, Loggable as _, RowId, components};
use re_view::clamped_or_nothing;
use re_viewer_context::{QueryContext, VisualizerReportSeverity};

use crate::{MAX_NUM_TIME_SERIES_SHOWN_PER_ENTITY_BY_DEFAULT, PlotPoint, PlotSeriesKind};

type PlotPointsPerSeries = smallvec::SmallVec<[Vec<PlotPoint>; 1]>;

/// Determines how many series there are in the scalar chunks.
pub fn determine_num_series(
    all_scalar_chunks: &re_view::ChunksWithComponent<'_>,
    results: &re_view::VisualizerInstructionQueryResults<'_>,
) -> usize {
    // TODO(andreas): We should determine this only once and cache the result.
    // As data comes in we can validate that the number of series is consistent.
    // Keep in mind clears here.
    let count = all_scalar_chunks
        .iter()
        .find_map(|chunk| {
            chunk
                .iter_slices::<f64>()
                .find_map(|slice| (!slice.is_empty()).then_some(slice.len()))
        })
        .unwrap_or(1);
    if count > MAX_NUM_TIME_SERIES_SHOWN_PER_ENTITY_BY_DEFAULT {
        results.report_unspecified_source(VisualizerReportSeverity::Error,format!("Number of series ({count}) exceeds the maximum ({MAX_NUM_TIME_SERIES_SHOWN_PER_ENTITY_BY_DEFAULT}). Only the first {MAX_NUM_TIME_SERIES_SHOWN_PER_ENTITY_BY_DEFAULT} series will be visualized."));
        MAX_NUM_TIME_SERIES_SHOWN_PER_ENTITY_BY_DEFAULT
    } else {
        count
    }
}

/// Queries the visibility flags for all series in a query.
pub fn collect_series_visibility(
    query_ctx: &QueryContext<'_>,
    results: &re_view::VisualizerInstructionQueryResults<'_>,
    num_series: usize,
    visibility_descriptor: &ComponentDescriptor,
) -> Vec<bool> {
    let boolean_buffer = results
        .iter_optional(visibility_descriptor.component)
        .slice::<bool>()
        .next()
        .map_or_else(
            || {
                query_ctx
                    .viewer_ctx()
                    .component_fallback_registry
                    .fallback_for(visibility_descriptor, query_ctx)
                    .as_any()
                    .downcast_ref::<BooleanArray>()
                    .map(|arr| arr.values().clone())
                    .unwrap_or_else(|| {
                        re_log::warn_once!(
                            "Failed to cast visibility fallback to BooleanArray, defaulting to true"
                        );
                        BooleanBuffer::new_set(1)
                    })
            },
            |(_, visible)| visible,
        );

    let mut flags = boolean_buffer.iter().take(num_series).collect_vec();

    // If there are less flags than series, repeat the last flag (or true if there are no flags).
    if flags.len() < num_series {
        flags.extend(std::iter::repeat_n(
            *flags.last().unwrap_or(&true),
            num_series - flags.len(),
        ));
    }

    flags
}

/// Allocates all points for the series.
pub fn allocate_plot_points(
    query: &RangeQuery,
    default_point: &PlotPoint,
    all_scalar_chunks: &re_view::ChunksWithComponent<'_>,
    num_series: usize,
) -> PlotPointsPerSeries {
    re_tracing::profile_function!();

    // TODO(andreas): skip invisible?

    let points = all_scalar_chunks
        .iter()
        .flat_map(|chunk| chunk.iter_component_indices(*query.timeline()))
        .map(|(data_time, _)| PlotPoint {
            time: data_time.as_i64(),
            ..default_point.clone()
        })
        .collect_vec();

    re_tracing::profile_scope!(
        "smallvec![]",
        format!("{} points x {} series", points.len(), num_series)
    );
    smallvec::smallvec![points; num_series]
}

/// Allocates scalars per series into pre-allocated plot points.
pub fn collect_scalars(
    all_scalar_chunks: &re_view::ChunksWithComponent<'_>,
    points_per_series: &mut PlotPointsPerSeries,
) {
    re_tracing::profile_function!(format!("points_per_series={}", points_per_series.len()));

    if points_per_series.len() == 1 {
        let points = &mut *points_per_series[0];
        for (i, values) in all_scalar_chunks
            .iter()
            .flat_map(|chunk| chunk.iter_slices::<f64>())
            .enumerate()
        {
            if let Some(value) = values.first() {
                points[i].value = *value;
            } else {
                points[i].attrs.kind = PlotSeriesKind::Clear;
            }
        }
    } else {
        for (i, values) in all_scalar_chunks
            .iter()
            .flat_map(|chunk| chunk.iter_slices::<f64>())
            .enumerate()
        {
            for (points, value) in points_per_series.iter_mut().zip(values) {
                points[i].value = *value;
            }
            for points in points_per_series.iter_mut().skip(values.len()) {
                points[i].attrs.kind = PlotSeriesKind::Clear;
            }
        }
    }
}

/// Collects colors for the series into pre-allocated plot points.
pub fn collect_colors(
    query_ctx: &QueryContext<'_>,
    query: &RangeQuery,
    query_results: &re_view::VisualizerInstructionQueryResults<'_>,
    all_scalar_chunks: &re_view::ChunksWithComponent<'_>,
    points_per_series: &mut PlotPointsPerSeries,
    color_descriptor: &ComponentDescriptor,
) {
    re_tracing::profile_function!();

    let num_series = points_per_series.len();

    re_log::debug_assert_eq!(components::Color::arrow_datatype(), ArrowDatatype::UInt32);

    fn map_raw_color(raw: &u32) -> re_renderer::Color32 {
        let [a, b, g, r] = raw.to_le_bytes();
        #[expect(clippy::disallowed_methods)] // This is not a hard-coded color.
        re_renderer::Color32::from_rgba_unmultiplied(r, g, b, a)
    }

    let color_iter = query_results.iter_optional(color_descriptor.component);
    let all_color_chunks = color_iter.chunks().iter().collect_vec();

    if all_color_chunks.len() == 1 && all_color_chunks[0].chunk.is_static() {
        re_tracing::profile_scope!("override/default fast path");

        if let Some(colors) = all_color_chunks[0].iter_slices::<u32>().next() {
            for (points, color) in points_per_series
                .iter_mut()
                .zip(clamped_or_nothing(colors, num_series))
            {
                let color = map_raw_color(color);
                for point in points {
                    point.attrs.color = color;
                }
            }
        }
    } else if all_color_chunks.is_empty() {
        re_tracing::profile_scope!("fallback colors");

        let fallback_array = query_ctx
            .viewer_ctx()
            .component_fallback_registry
            .fallback_for(color_descriptor, query_ctx);

        if let Some(color_array) = fallback_array.as_any().downcast_ref::<array::UInt32Array>() {
            let fallback_colors = color_array.values();

            for (points, color) in points_per_series
                .iter_mut()
                .zip(clamped_or_nothing(fallback_colors.as_ref(), num_series))
            {
                let color = map_raw_color(color);
                for point in points {
                    point.attrs.color = color;
                }
            }
        } else {
            re_log::error_once!("Failed to cast builtin color fallback to UInt32Array");
        }
    } else {
        re_tracing::profile_scope!("standard path");

        let all_colors = all_color_chunks.iter().flat_map(|chunk| {
            itertools::izip!(
                chunk.iter_component_indices(*query.timeline()),
                chunk.iter_slices::<u32>()
            )
        });

        let all_frames =
            re_query::range_zip_1x1(all_scalars_indices(query, all_scalar_chunks), all_colors)
                .enumerate();

        // Simplified path for single series.
        if num_series == 1 {
            let points = &mut *points_per_series[0];
            all_frames.for_each(|(i, (_index, _scalars, colors))| {
                if let Some(color) = colors.and_then(|c| c.first()) {
                    points[i].attrs.color = map_raw_color(color);
                }
            });
        } else {
            all_frames.for_each(|(i, (_index, _scalars, colors))| {
                if let Some(colors) = colors {
                    for (points, color) in points_per_series
                        .iter_mut()
                        .zip(clamped_or_nothing(colors, num_series))
                    {
                        points[i].attrs.color = map_raw_color(color);
                    }
                }
            });
        }
    }
}

/// Expands names to match `num_series`, adding indices for additional series.
/// For selectors like `data[]`, strips the `[]` suffix before adding indices.
fn expand_series_names(names: &[String], num_series: usize) -> Vec<String> {
    let name_count = names.len();
    (0..num_series)
        .zip(clamped_or_nothing(names, num_series))
        .map(|(i, name)| {
            if i < name_count {
                name.clone()
            } else {
                format!("{name}[{i}]")
            }
        })
        .collect()
}

/// Collects series names for the series into pre-allocated plot points.
pub fn collect_series_name(
    query_ctx: &QueryContext<'_>,
    query_results: &re_view::VisualizerInstructionQueryResults<'_>,
    num_series: usize,
    name_descriptor: &ComponentDescriptor,
) -> Vec<String> {
    re_tracing::profile_function!();

    let name_iter = query_results.iter_optional(name_descriptor.component);
    let all_name_chunks = name_iter.chunks().iter().collect_vec();

    if let Some(slice) = all_name_chunks
        .iter()
        .find(|chunk| !chunk.chunk.is_empty())
        .and_then(|chunk| chunk.iter_slices::<String>().next())
        .filter(|slice| !slice.is_empty())
    {
        re_tracing::profile_scope!("logged names");
        let names: Vec<String> = slice.iter().map(|s| s.to_string()).collect();
        expand_series_names(&names, num_series)
    } else {
        re_tracing::profile_scope!("fallback names");

        let fallback_array = query_ctx
            .viewer_ctx()
            .component_fallback_registry
            .fallback_for(name_descriptor, query_ctx);

        if let Some(string_array) = fallback_array.as_any().downcast_ref::<array::StringArray>() {
            let fallback_names: Vec<_> = string_array
                .iter()
                .flatten()
                .map(|s| s.to_owned())
                .collect();

            if fallback_names.is_empty() {
                re_log::error_once!("Failed to retrieve fallback names");
                vec![]
            } else {
                // Due to the frame delay, we might end up with too few fallbacks here too, so we always
                // expand the array of names.
                expand_series_names(&fallback_names, num_series)
            }
        } else {
            re_log::error_once!("Failed to cast builtin name fallback to StringArray");
            vec![]
        }
    }
}

/// Collects `radius_ui` for the series into pre-allocated plot points.
pub fn collect_radius_ui(
    query: &RangeQuery,
    query_results: &re_view::VisualizerInstructionQueryResults<'_>,
    all_scalar_chunks: &re_view::ChunksWithComponent<'_>,
    points_per_series: &mut PlotPointsPerSeries,
    radius_descriptor: &ComponentDescriptor,
    radius_multiplier: f32,
) {
    re_tracing::profile_function!();

    let num_series = points_per_series.len();

    {
        let radius_iter = query_results.iter_optional(radius_descriptor.component);
        let all_radius_chunks = radius_iter.chunks().iter().collect_vec();

        if all_radius_chunks.len() == 1 && all_radius_chunks[0].chunk.is_static() {
            re_tracing::profile_scope!("override/default fast path");

            if let Some(radius) = all_radius_chunks[0].iter_slices::<f32>().next() {
                for (points, radius) in points_per_series
                    .iter_mut()
                    .zip(clamped_or_nothing(radius, num_series))
                {
                    let radius = radius * radius_multiplier;
                    for point in points {
                        point.attrs.radius_ui = radius;
                    }
                }
            }
        } else {
            re_tracing::profile_scope!("standard path");

            let all_radii = all_radius_chunks.iter().flat_map(|chunk| {
                itertools::izip!(
                    chunk.iter_component_indices(*query.timeline()),
                    chunk.iter_slices::<f32>()
                )
            });

            let all_frames =
                re_query::range_zip_1x1(all_scalars_indices(query, all_scalar_chunks), all_radii)
                    .enumerate();

            // Simplified path for single series.
            if num_series == 1 {
                let points = &mut *points_per_series[0];
                all_frames.for_each(|(i, (_index, _scalars, radius))| {
                    if let Some(stroke_width) = radius.and_then(|radius| radius.first().copied()) {
                        points[i].attrs.radius_ui = stroke_width * radius_multiplier;
                    }
                });
            } else {
                all_frames.for_each(|(i, (_index, _scalars, radius))| {
                    if let Some(radii) = radius {
                        for (points, stroke_width) in points_per_series
                            .iter_mut()
                            .zip(clamped_or_nothing(radii, num_series))
                        {
                            points[i].attrs.radius_ui = stroke_width * radius_multiplier;
                        }
                    }
                });
            }
        }
    }
}

pub fn all_scalars_indices<'a>(
    query: &'a RangeQuery,
    all_scalar_chunks: &'a re_view::ChunksWithComponent<'_>,
) -> impl Iterator<Item = ((TimeInt, RowId), ())> + 'a {
    all_scalar_chunks
        .iter()
        .flat_map(|chunk| chunk.iter_component_indices(*query.timeline()))
        // That is just so we can satisfy the `range_zip` contract later on.
        .map(|index| (index, ()))
}
