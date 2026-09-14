//! Shared functionality for querying time series data.

use std::iter::zip;

use itertools::Itertools as _;

use re_chunk_store::RangeQuery;
use re_log_types::TimeInt;
use re_log_types::external::arrow::array::AsArray as _;
use re_log_types::external::arrow::buffer::BooleanBuffer;
use re_log_types::external::arrow::datatypes::UInt32Type;
use re_sdk_types::external::arrow::datatypes::DataType as ArrowDataType;
use re_sdk_types::{
    ArrowDataType as _, ComponentDescriptor, ComponentIdentifier, RowId, components,
};
use re_view::clamped_or_nothing;
use re_viewer_context::{ViewQuery, ViewerReportSeverity};

use crate::{MAX_NUM_SERIES_FOR_REMAPPED_SCALARS, PlotPoint, PlotSeriesKind};

type PlotPointsPerSeries = smallvec::SmallVec<[Vec<PlotPoint>; 1]>;

/// All scalar rows in chunk iteration order.
fn iter_scalar_slices<'a>(
    all_scalar_chunks: &'a re_view::ChunksWithComponent<'_>,
) -> impl Iterator<Item = &'a [f64]> + 'a {
    all_scalar_chunks
        .iter()
        .flat_map(|chunk| chunk.iter_slices::<f64>())
}

/// Determines how many series there are in the scalar chunks.
///
/// Uses the first non-empty scalar slice in chunk iteration order for rendering.
/// Width consistency is checked in [`collect_scalars`].
///
/// If the scalar component has a non-identity mapping (i.e. it's sourced from a different
/// component or uses a selector), the number of series is capped at
/// [`MAX_NUM_SERIES_FOR_REMAPPED_SCALARS`].
/// Identity mappings (direct logged data) are not capped since the user explicitly
/// logged that data.
pub fn determine_num_series(
    all_scalar_chunks: &re_view::ChunksWithComponent<'_>,
    results: &re_view::VisualizerInstructionQueryResults<'_>,
    value_component: ComponentIdentifier,
) -> usize {
    let count = iter_scalar_slices(all_scalar_chunks)
        .find_map(|slice| (!slice.is_empty()).then_some(slice.len()))
        .unwrap_or(1);

    let is_identity = results.has_identity_mapping_for_component(value_component);

    let limits_enabled = results
        .query_context()
        .app_ctx()
        .app_options
        .visualizer_limits_enabled;
    if !is_identity && limits_enabled && count > MAX_NUM_SERIES_FOR_REMAPPED_SCALARS {
        results.report_unspecified_source(
            ViewerReportSeverity::Error,
            format!(
                "Too many series ({}), capping to {}. \
                This limit can be lifted in Settings.",
                re_format::format_uint(count),
                re_format::format_uint(MAX_NUM_SERIES_FOR_REMAPPED_SCALARS),
            ),
        );
        MAX_NUM_SERIES_FOR_REMAPPED_SCALARS
    } else {
        count
    }
}

/// Queries the visibility flags for all series in a query.
pub fn collect_series_visibility(
    results: &re_view::VisualizerInstructionQueryResults<'_>,
    num_series: usize,
    visibility_descriptor: &ComponentDescriptor,
) -> Vec<bool> {
    let query_ctx = results.query_context();
    let boolean_buffer = results
        .iter_optional(visibility_descriptor.component)
        .slice::<bool>()
        .next()
        .map_or_else(
            || {
                query_ctx
                    .viewer_ctx()
                    .component_fallback_registry()
                    .fallback_for(visibility_descriptor, query_ctx)
                    .as_boolean_opt()
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
///
/// Warns once if non-empty rows have different widths.
pub fn collect_scalars(
    all_scalar_chunks: &re_view::ChunksWithComponent<'_>,
    results: &re_view::VisualizerInstructionQueryResults<'_>,
    points_per_series: &mut PlotPointsPerSeries,
) {
    re_tracing::profile_function!(format!("points_per_series={}", points_per_series.len()));

    let num_series = points_per_series.len();
    let mut expected_width = None;
    let mut width_mismatch = false;

    // `i` is the time index.
    for (i, values) in iter_scalar_slices(all_scalar_chunks).enumerate() {
        if !values.is_empty() {
            let expected_width = *expected_width.get_or_insert(values.len());
            width_mismatch |= values.len() != expected_width;
        }

        if num_series == 1 {
            let points = &mut points_per_series[0];
            if let Some(value) = values.first() {
                points[i].value = *value;
            } else {
                points[i].attrs.kind = PlotSeriesKind::Clear;
            }
        } else {
            for (points, value) in zip(&mut *points_per_series, values) {
                points[i].value = *value;
            }
            // `zip` stops at the shorter iterator — extra scalars in `values` are ignored.
            for points in points_per_series.iter_mut().skip(values.len()) {
                points[i].attrs.kind = PlotSeriesKind::Clear;
            }
        }
    }

    if width_mismatch {
        results.report_unspecified_source(
            ViewerReportSeverity::Warning,
            format!(
                "Number of scalars for entity `{}` varies between timestamps in the query, \
                currently rendering {} series",
                results.entity_path(),
                re_format::format_uint(num_series),
            ),
        );
    }
}

/// Collects colors for the series into pre-allocated plot points.
pub fn collect_colors(
    query: &RangeQuery,
    query_results: &re_view::VisualizerInstructionQueryResults<'_>,
    all_scalar_chunks: &re_view::ChunksWithComponent<'_>,
    points_per_series: &mut PlotPointsPerSeries,
    color_descriptor: &ComponentDescriptor,
) {
    re_tracing::profile_function!();

    let query_ctx = query_results.query_context();
    let num_series = points_per_series.len();

    re_log::debug_assert_eq!(components::Color::arrow_data_type(), ArrowDataType::UInt32);

    fn map_raw_color(raw: &u32) -> re_renderer::Color32 {
        let [a, b, g, r] = raw.to_le_bytes();
        #[expect(clippy::disallowed_methods)] // This is not a hard-coded color.
        re_renderer::Color32::from_rgba_unmultiplied(r, g, b, a)
    }

    let color_iter = query_results.iter_optional(color_descriptor.component);
    let all_color_chunks = color_iter.chunks().iter().collect_vec();

    if all_color_chunks.len() == 1 && all_color_chunks[0].chunk.num_rows() == 1 {
        re_tracing::profile_scope!("override/default fast path");

        if let Some(colors) = all_color_chunks[0].iter_slices::<u32>().next() {
            for (points, color) in std::iter::zip(
                points_per_series.iter_mut(),
                clamped_or_nothing(colors, num_series),
            ) {
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
            .component_fallback_registry()
            .fallback_for(color_descriptor, query_ctx);

        if let Some(color_array) = fallback_array.as_primitive_opt::<UInt32Type>() {
            let fallback_colors = color_array.values();

            for (points, color) in std::iter::zip(
                points_per_series.iter_mut(),
                clamped_or_nothing(fallback_colors.as_ref(), num_series),
            ) {
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
                    for (points, color) in std::iter::zip(
                        points_per_series.iter_mut(),
                        clamped_or_nothing(colors, num_series),
                    ) {
                        points[i].attrs.color = map_raw_color(color);
                    }
                }
            });
        }
    }
}

/// Expands names to match `num_series`, adding indices for additional series.
/// For selectors like `data[]`, strips the `[]` suffix before adding indices.
/// First non-empty string batch logged for a per-series component.
///
/// Per-series strings (names, units) are expected to be unchanging over time, so the first
/// non-empty reading wins.
fn first_string_batch(
    query_results: &re_view::VisualizerInstructionQueryResults<'_>,
    descriptor: &ComponentDescriptor,
) -> Option<Vec<String>> {
    let iter = query_results.iter_optional(descriptor.component);
    let slice = iter
        .chunks()
        .iter()
        .flat_map(|chunk| chunk.iter_slices::<String>())
        .find(|slice| !slice.is_empty())?;

    Some(slice.iter().map(|s| s.to_string()).collect())
}

fn expand_series_names(names: &[String], num_series: usize) -> Vec<String> {
    let name_count = names.len();
    std::iter::zip(0..num_series, clamped_or_nothing(names, num_series))
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
    query_results: &re_view::VisualizerInstructionQueryResults<'_>,
    num_series: usize,
    name_descriptor: &ComponentDescriptor,
) -> Vec<String> {
    re_tracing::profile_function!();

    let query_ctx = query_results.query_context();

    if let Some(names) = first_string_batch(query_results, name_descriptor) {
        re_tracing::profile_scope!("logged names");
        expand_series_names(&names, num_series)
    } else {
        re_tracing::profile_scope!("fallback names");

        let fallback_array = query_ctx
            .viewer_ctx()
            .component_fallback_registry()
            .fallback_for(name_descriptor, query_ctx);

        if let Some(string_array) = fallback_array.as_string_opt::<i32>() {
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

        if all_radius_chunks.len() == 1 && all_radius_chunks[0].chunk.num_rows() == 1 {
            re_tracing::profile_scope!("override/default fast path");

            if let Some(radius) = all_radius_chunks[0].iter_slices::<f32>().next() {
                for (points, radius) in std::iter::zip(
                    points_per_series.iter_mut(),
                    clamped_or_nothing(radius, num_series),
                ) {
                    let radius = radius * radius_multiplier;
                    for point in points {
                        point.attrs.radius_ui = radius;
                    }
                }
            }
        } else if !all_radius_chunks.is_empty() {
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
                        for (points, stroke_width) in std::iter::zip(
                            points_per_series.iter_mut(),
                            clamped_or_nothing(radii, num_series),
                        ) {
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

/// Returns true if `series` should be drawn with the highlighted (hovered/selected) style.
///
/// Used by both the line visualizer (to thicken the stroke) and the marker painter (to grow
/// the markers), so the visual highlight stays consistent across line and scatter series.
pub(crate) fn is_series_highlighted(query: &ViewQuery<'_>, series: &crate::PlotSeries) -> bool {
    query
        .highlights
        .entity_highlight(series.instance_path.entity_path.hash())
        .index_highlight(
            series.instance_path.instance,
            series.visualizer_instruction_id,
        )
        .any()
}

/// Collects per-point variances (σ²) for the series into pre-allocated plot points.
///
/// Stored as logged: the square root that turns a variance into a band offset is only taken when
/// the band mesh is built, which is after aggregation has collapsed most of the points.
///
/// Returns whether a variance column was found at all, i.e. whether the series get a band. The
/// column is broadcast across all series of the entity, so this is the same answer for each.
pub fn collect_variances(
    query: &RangeQuery,
    query_results: &re_view::VisualizerInstructionQueryResults<'_>,
    all_scalar_chunks: &re_view::ChunksWithComponent<'_>,
    points_per_series: &mut PlotPointsPerSeries,
    variance_descriptor: &ComponentDescriptor,
) -> bool {
    re_tracing::profile_function!();

    let num_series = points_per_series.len();

    let variance_iter = query_results.iter_optional(variance_descriptor.component);
    let all_variance_chunks = variance_iter.chunks().iter().collect_vec();
    if all_variance_chunks.is_empty() {
        return false;
    }

    if all_variance_chunks.len() == 1 && all_variance_chunks[0].chunk.num_rows() == 1 {
        re_tracing::profile_scope!("override/default fast path");

        let Some(variances) = all_variance_chunks[0].iter_slices::<f64>().next() else {
            return false;
        };

        // A single broadcast row is cheap to inspect, and an all-zero one (e.g. a blueprint
        // default) would otherwise build a band mesh that draws nothing.
        let mut has_variances = false;
        for (points, variance) in std::iter::zip(
            points_per_series.iter_mut(),
            clamped_or_nothing(variances, num_series),
        ) {
            let variance = *variance as f32;
            has_variances |= variance > 0.0;
            for point in points {
                point.variance = variance;
            }
        }
        return has_variances;
    }

    {
        re_tracing::profile_scope!("standard path");

        let all_variances = all_variance_chunks.iter().flat_map(|chunk| {
            itertools::izip!(
                chunk.iter_component_indices(*query.timeline()),
                chunk.iter_slices::<f64>()
            )
        });

        let all_frames =
            re_query::range_zip_1x1(all_scalars_indices(query, all_scalar_chunks), all_variances)
                .enumerate();

        all_frames.for_each(|(i, (_index, _scalars, variances))| {
            let Some(variances) = variances else {
                return;
            };
            for (points, variance) in std::iter::zip(
                points_per_series.iter_mut(),
                clamped_or_nothing(variances, num_series),
            ) {
                points[i].variance = *variance as f32;
            }
        });
    }

    true
}

/// Collects units for the series, from an optional unit column.
///
/// An empty string means "no unit".
pub fn collect_series_units(
    query_results: &re_view::VisualizerInstructionQueryResults<'_>,
    num_series: usize,
    unit_descriptor: &ComponentDescriptor,
) -> Vec<Option<String>> {
    re_tracing::profile_function!();

    let Some(units) = first_string_batch(query_results, unit_descriptor) else {
        return vec![None; num_series];
    };

    // Like the other per-series components, a short column repeats its last entry.
    clamped_or_nothing(&units, num_series)
        .map(|unit| (!unit.is_empty()).then(|| unit.clone()))
        .collect()
}
