//! Shared functionality for querying time series data.

use itertools::Itertools as _;

use re_chunk_store::RangeQuery;
use re_chunk_store::external::re_chunk::CastToPrimitive;
use re_log_types::{EntityPath, TimeInt};
use re_sdk_types::external::arrow;
use re_sdk_types::external::arrow::datatypes::DataType as ArrowDatatype;
use re_sdk_types::{ComponentDescriptor, ComponentIdentifier, Loggable as _, RowId, components};
use re_view::{ChunksWithComponent, HybridRangeResults, RangeResultsExt as _, clamped_or_nothing};
use re_viewer_context::{QueryContext, auto_color_egui, typed_fallback_for};

use crate::{PlotPoint, PlotSeriesKind};

type PlotPointsPerSeries = smallvec::SmallVec<[Vec<PlotPoint>; 1]>;

/// Determines how many series there are in the scalar chunks.
pub fn determine_num_series(all_scalar_chunks: &ChunksWithComponent<'_>) -> usize {
    // TODO(andreas): We should determine this only once and cache the result.
    // As data comes in we can validate that the number of series is consistent.
    // Keep in mind clears here.
    all_scalar_chunks
        .iter()
        .find_map(|chunk| {
            chunk
                // TODO(grtlr): The comment above is even more important when we do casting (allocations) here.
                // TODO(grtlr): Unless we're on the happy path this allocates and happens everytime the visualizer runs!
                .iter_slices::<CastToPrimitive<arrow::datatypes::Float64Type, f64>>()
                .find_map(|values| (!values.is_empty()).then_some(values.len()))
        })
        .unwrap_or(1)
}

/// Queries the visibility flags for all series in a query.
pub fn collect_series_visibility(
    query: &RangeQuery,
    bootstrapped_results: &re_view::HybridLatestAtResults<'_>,
    results: &HybridRangeResults<'_>,
    num_series: usize,
    visibility_component: ComponentIdentifier,
) -> Vec<bool> {
    bootstrapped_results
        .iter_as(*query.timeline(), visibility_component)
        .slice::<bool>()
        .chain(
            results
                .iter_as(*query.timeline(), visibility_component)
                .slice::<bool>(),
        )
        .next()
        .map_or_else(
            || vec![true; num_series], // By default all series are visible.
            |(_, visible)| {
                let mut flags = visible.iter().collect_vec();
                if flags.len() < num_series {
                    // If there are less flags than series, repeat the last flag (or true if there are no flags).
                    flags.extend(std::iter::repeat_n(
                        *flags.last().unwrap_or(&true),
                        num_series - flags.len(),
                    ));
                }
                flags
            },
        )
}

/// Allocates all points for the series.
pub fn allocate_plot_points(
    query: &RangeQuery,
    default_point: &PlotPoint,
    all_scalar_chunks: &ChunksWithComponent<'_>,
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

    smallvec::smallvec![points; num_series]
}

/// Allocates scalars per series into pre-allocated plot points.
pub fn collect_scalars(
    all_scalar_chunks: &ChunksWithComponent<'_>,
    points_per_series: &mut PlotPointsPerSeries,
) {
    re_tracing::profile_function!();

    if points_per_series.len() == 1 {
        let points = &mut *points_per_series[0];
        all_scalar_chunks
            .iter()
            .flat_map(|chunk| {
                // TODO(grtlr): Unless we're on the happy path this allocates and happens everytime the visualizer runs!
                chunk.iter_slices::<CastToPrimitive<arrow::datatypes::Float64Type, f64>>()
            })
            .enumerate()
            .for_each(|(i, values)| {
                if let Some(value) = values.first() {
                    points[i].value = *value;
                } else {
                    points[i].attrs.kind = PlotSeriesKind::Clear;
                }
            });
    } else {
        all_scalar_chunks
            .iter()
            .flat_map(|chunk| {
                // TODO(grtlr): Unless we're on the happy path this allocates and happens everytime the visualizer runs!
                chunk.iter_slices::<CastToPrimitive<arrow::datatypes::Float64Type, f64>>()
            })
            .enumerate()
            .for_each(|(i, values)| {
                let values_slice = values.as_ref();
                for (points, value) in points_per_series.iter_mut().zip(values_slice) {
                    points[i].value = *value;
                }
                for points in points_per_series.iter_mut().skip(values_slice.len()) {
                    points[i].attrs.kind = PlotSeriesKind::Clear;
                }
            });
    }
}

/// Collects colors for the series into pre-allocated plot points.
pub fn collect_colors(
    entity_path: &EntityPath,
    query: &RangeQuery,
    bootstrapped_results: &re_view::HybridLatestAtResults<'_>,
    results: &re_view::HybridRangeResults<'_>,
    all_scalar_chunks: &ChunksWithComponent<'_>,
    points_per_series: &mut smallvec::SmallVec<[Vec<PlotPoint>; 1]>,
    color_descriptor: &ComponentDescriptor,
) {
    re_tracing::profile_function!();

    let num_series = points_per_series.len();

    debug_assert_eq!(components::Color::arrow_datatype(), ArrowDatatype::UInt32);

    fn map_raw_color(raw: &u32) -> re_renderer::Color32 {
        let [a, b, g, r] = raw.to_le_bytes();
        #[expect(clippy::disallowed_methods)] // This is not a hard-coded color.
        re_renderer::Color32::from_rgba_unmultiplied(r, g, b, a)
    }

    let all_color_chunks = bootstrapped_results
        .get_optional_chunks(color_descriptor.component)
        .iter()
        .cloned()
        .chain(
            results
                .get_optional_chunks(color_descriptor.component)
                .iter()
                .cloned(),
        )
        .collect_vec();

    if all_color_chunks.len() == 1 && all_color_chunks[0].is_static() {
        re_tracing::profile_scope!("override/default fast path");

        if let Some(colors) = all_color_chunks[0]
            .iter_slices::<u32>(color_descriptor.component)
            .next()
        {
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
        if num_series > 1 {
            re_tracing::profile_scope!("default color for multiple series");

            // Have to fill in additional default colors.
            // TODO(andreas): Could they somehow be provided by the fallback provider?
            // It's tricky since the fallback provider doesn't know how many colors to produce!
            for (i, points) in points_per_series.iter_mut().skip(1).enumerate() {
                // Normally we generate colors from entity names, but getting the display label needs extra processing,
                // and it's nice to not care about that here.
                let fallback_color = auto_color_egui(
                    (re_log_types::hash::Hash64::hash((entity_path, i)).hash64() % u16::MAX as u64)
                        as u16,
                );
                for point in points {
                    point.attrs.color = fallback_color;
                }
            }
        }
    } else {
        re_tracing::profile_scope!("standard path");

        let all_colors = all_color_chunks.iter().flat_map(|chunk| {
            itertools::izip!(
                chunk.iter_component_indices(*query.timeline(), color_descriptor.component),
                chunk.iter_slices::<u32>(color_descriptor.component)
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

/// Collects series names for the series into pre-allocated plot points.
pub fn collect_series_name(
    query_ctx: &QueryContext<'_>,
    bootstrapped_results: &re_view::HybridLatestAtResults<'_>,
    results: &re_view::HybridRangeResults<'_>,
    num_series: usize,
    name_descriptor: &ComponentDescriptor,
) -> Vec<String> {
    re_tracing::profile_function!();

    let mut series_names: Vec<String> = bootstrapped_results
        .get_optional_chunks(name_descriptor.component)
        .iter()
        .chain(
            results
                .get_optional_chunks(name_descriptor.component)
                .iter(),
        )
        .find(|chunk| !chunk.is_empty())
        .and_then(|chunk| {
            chunk
                .iter_slices::<String>(name_descriptor.component)
                .next()
        })
        .map(|slice| slice.into_iter().map(|s| s.to_string()).collect())
        .unwrap_or_default();

    if series_names.len() < num_series {
        let fallback_name: String =
            typed_fallback_for::<components::Name>(query_ctx, name_descriptor.component)
                .to_string();
        if num_series == 1 {
            series_names.push(fallback_name);
        } else {
            // Repeating a name never makes sense, so we fill up the remaining names with made up ones instead.
            series_names
                .extend((series_names.len()..num_series).map(|i| format!("{fallback_name}/{i}")));
        }
    }

    series_names
}

/// Collects `radius_ui` for the series into pre-allocated plot points.
pub fn collect_radius_ui(
    query: &RangeQuery,
    bootstrapped_results: &re_view::HybridLatestAtResults<'_>,
    results: &re_view::HybridRangeResults<'_>,
    all_scalar_chunks: &ChunksWithComponent<'_>,
    points_per_series: &mut smallvec::SmallVec<[Vec<PlotPoint>; 1]>,
    radius_descriptor: &ComponentDescriptor,
    radius_multiplier: f32,
) {
    re_tracing::profile_function!();

    let num_series = points_per_series.len();

    {
        let all_radius_chunks = bootstrapped_results
            .get_optional_chunks(radius_descriptor.component)
            .iter()
            .cloned()
            .chain(
                results
                    .get_optional_chunks(radius_descriptor.component)
                    .iter()
                    .cloned(),
            )
            .collect_vec();

        if all_radius_chunks.len() == 1 && all_radius_chunks[0].is_static() {
            re_tracing::profile_scope!("override/default fast path");

            if let Some(radius) = all_radius_chunks[0]
                .iter_slices::<f32>(radius_descriptor.component)
                .next()
            {
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
                    chunk.iter_component_indices(*query.timeline(), radius_descriptor.component),
                    chunk.iter_slices::<f32>(radius_descriptor.component)
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
    all_scalar_chunks: &'a ChunksWithComponent<'_>,
) -> impl Iterator<Item = ((TimeInt, RowId), ())> + 'a {
    all_scalar_chunks
        .iter()
        .flat_map(|chunk| chunk.iter_component_indices(*query.timeline()))
        // That is just so we can satisfy the `range_zip` contract later on.
        .map(|index| (index, ()))
}
