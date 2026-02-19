use itertools::Itertools as _;
use rayon::prelude::*;
use re_sdk_types::archetypes::SeriesPoints;
use re_sdk_types::components::{self, MarkerShape, MarkerSize};
use re_sdk_types::{Archetype as _, Component as _, Loggable as _, archetypes};
use re_view::{clamped_or_nothing, range_with_blueprint_resolved_data};
use re_viewer_context::external::re_entity_db::InstancePath;
use re_viewer_context::{
    AnyPhysicalDatatypeRequirement, IdentifiedViewSystem, ViewContext, ViewQuery,
    ViewSystemExecutionError, VisualizerExecutionOutput, VisualizerQueryInfo, VisualizerSystem,
    typed_fallback_for,
};

use crate::series_query::{
    all_scalars_indices, allocate_plot_points, collect_colors, collect_radius_ui, collect_scalars,
    collect_series_name, collect_series_visibility, determine_num_series,
};
use crate::{
    PlotPoint, PlotPointAttrs, PlotSeries, PlotSeriesKind, ScatterAttrs, ViewPropertyQueryError,
    util,
};

/// The system for rendering [`archetypes::SeriesPoints`] archetypes.
#[derive(Default, Debug)]
pub struct SeriesPointsSystem {
    pub all_series: Vec<PlotSeries>,
}

impl IdentifiedViewSystem for SeriesPointsSystem {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "SeriesPoints".into()
    }
}

impl VisualizerSystem for SeriesPointsSystem {
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo {
            relevant_archetype: archetypes::SeriesPoints::name().into(),
            required: AnyPhysicalDatatypeRequirement {
                target_component: archetypes::Scalars::descriptor_scalars().component,
                semantic_type: components::Scalar::name(),
                physical_types: util::series_supported_datatypes().into_iter().collect(),
                allow_static_data: false,
            }
            .into(),
            queried: archetypes::Scalars::all_components()
                .iter()
                .chain(archetypes::SeriesPoints::all_components().iter())
                .cloned()
                .collect(),
        }
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        query: &ViewQuery<'_>,
        _context: &re_viewer_context::ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        re_tracing::profile_function!();

        let output = VisualizerExecutionOutput::default();

        let plot_mem =
            egui_plot::PlotMemory::load(ctx.viewer_ctx.egui_ctx(), crate::plot_id(query.view_id));
        let time_per_pixel = util::determine_time_per_pixel(ctx.viewer_ctx, plot_mem.as_ref());

        let data_results: Vec<_> = query
            .iter_visualizer_instruction_for(Self::identifier())
            .collect();

        let all_series: Result<Vec<_>, _> = data_results
            .par_iter()
            .map(|(data_result, instruction)| {
                Self::load_series(
                    ctx,
                    query,
                    time_per_pixel,
                    data_result,
                    instruction,
                    &output,
                )
            })
            .collect();

        self.all_series.extend(all_series?.into_iter().flatten());

        Ok(output)
    }
}

impl SeriesPointsSystem {
    fn load_series(
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        time_per_pixel: f64,
        data_result: &re_viewer_context::DataResult,
        instruction: &re_viewer_context::VisualizerInstruction,
        output: &VisualizerExecutionOutput,
    ) -> Result<Vec<PlotSeries>, ViewPropertyQueryError> {
        re_tracing::profile_function!();

        let current_query = ctx.current_query();
        let query_ctx = ctx.query_context(data_result, current_query.clone(), instruction.id);

        let visible_time_range = util::determine_visible_time_range(ctx, data_result);
        let time_range = util::determine_time_range(ctx, visible_time_range)?;

        {
            re_tracing::profile_scope!("primary", &data_result.entity_path.to_string());

            let query = re_chunk_store::RangeQuery::new(view_query.timeline, time_range);

            let mut results = range_with_blueprint_resolved_data(
                ctx,
                None,
                &query,
                data_result,
                archetypes::Scalars::all_component_identifiers()
                    .chain(archetypes::SeriesPoints::all_component_identifiers()),
                instruction,
            );

            // The plot view visualizes scalar data within a specific time range, without any kind
            // of time-alignment / bootstrapping behavior:
            // * For the scalar themselves, this is what you want: if you're trying to plot some
            //   data between t=100 and t=200, you don't want to display a point from t=20 (and
            //   _extended bounds_ will take care of lines crossing the limit).
            // * For the secondary components (colors, radii, names, etc), this is a problem
            //   though: you don't want your plot to change color depending on what the currently
            //   visible time range is! Secondary components have to be bootstrapped.
            //
            // Bootstrapping is now handled automatically by the query system for the components
            // we specified when calling range_with_blueprint_resolved_data.
            results.merge_bootstrapped_data(re_view::latest_at_with_blueprint_resolved_data(
                ctx,
                None,
                &re_chunk_store::LatestAtQuery::new(query.timeline, query.range.min()),
                data_result,
                archetypes::SeriesPoints::optional_components()
                    .iter()
                    .map(|c| c.component),
                Some(instruction),
            ));

            // Wrap results for convenient error-reporting iteration
            let results = re_view::BlueprintResolvedResults::Range(query.clone(), results);
            let results =
                re_view::VisualizerInstructionQueryResults::new(instruction.id, &results, output);

            // If we have no scalars, we can't do anything.
            let scalar_iter =
                results.iter_required(archetypes::Scalars::descriptor_scalars().component);
            let all_scalar_chunks = scalar_iter.chunks();

            // All the default values for a `PlotPoint`, accounting for both overrides and default values.
            // We know there's only a single value fallback for stroke width, so this is fine, albeit a bit hacky in case we add an array fallback later.
            let fallback_size: MarkerSize = typed_fallback_for(
                &query_ctx,
                archetypes::SeriesPoints::descriptor_marker_sizes().component,
            );
            let default_point = PlotPoint {
                time: 0,
                value: 0.0,
                attrs: PlotPointAttrs {
                    // Filled out later.
                    color: egui::Color32::DEBUG_COLOR,
                    // NOTE: arguably, the `MarkerSize` value should be twice the `radius_ui`. We do
                    // stick to the semantics of `MarkerSize` == radius for backward compatibility and
                    // because markers need a decent radius value to be at all legible.
                    radius_ui: **fallback_size,
                    kind: PlotSeriesKind::Scatter(ScatterAttrs {
                        marker: MarkerShape::default(),
                    }),
                },
            };

            let num_series = determine_num_series(all_scalar_chunks);
            let mut points_per_series =
                allocate_plot_points(&query, &default_point, all_scalar_chunks, num_series);

            collect_scalars(all_scalar_chunks, &mut points_per_series);
            collect_colors(
                &query_ctx,
                &query,
                &results,
                all_scalar_chunks,
                &mut points_per_series,
                &archetypes::SeriesPoints::descriptor_colors(),
            );
            collect_radius_ui(
                &query,
                &results,
                all_scalar_chunks,
                &mut points_per_series,
                &archetypes::SeriesPoints::descriptor_marker_sizes(),
                // `marker_size` is a radius, see NOTE above
                1.0,
            );

            // Fill in marker shapes
            {
                re_tracing::profile_scope!("fill marker shapes");

                {
                    let marker_iter = results
                        .iter_optional(archetypes::SeriesPoints::descriptor_markers().component);
                    let all_marker_shapes_chunks = marker_iter.chunks().iter().collect_vec();

                    if all_marker_shapes_chunks.len() == 1
                        && all_marker_shapes_chunks[0].chunk.is_static()
                    {
                        re_tracing::profile_scope!("override/default fast path");

                        if let Some(marker_shapes) = all_marker_shapes_chunks[0]
                            .iter_component::<MarkerShape>()
                            .next()
                        {
                            for (points, marker_shape) in points_per_series
                                .iter_mut()
                                .zip(clamped_or_nothing(marker_shapes.as_slice(), num_series))
                            {
                                for point in points {
                                    point.attrs.kind = PlotSeriesKind::Scatter(ScatterAttrs {
                                        marker: *marker_shape,
                                    });
                                }
                            }
                        }
                    } else if all_marker_shapes_chunks.is_empty() {
                        re_tracing::profile_scope!("fallback markers");

                        let fallback_array = query_ctx
                            .viewer_ctx()
                            .component_fallback_registry
                            .fallback_for(
                                SeriesPoints::descriptor_markers().component,
                                SeriesPoints::descriptor_markers().component_type,
                                &query_ctx,
                            );
                        if let Ok(marker_array) = MarkerShape::from_arrow(&fallback_array) {
                            for (points, marker) in points_per_series
                                .iter_mut()
                                .zip(clamped_or_nothing(&marker_array, num_series))
                            {
                                for p in points {
                                    p.attrs.kind =
                                        PlotSeriesKind::Scatter(ScatterAttrs { marker: *marker });
                                }
                            }
                        }
                    } else {
                        re_tracing::profile_scope!("standard path");

                        let mut all_marker_shapes_iters = all_marker_shapes_chunks
                            .iter()
                            .map(|chunk| chunk.iter_component::<MarkerShape>())
                            .collect_vec();
                        let all_marker_shapes_indexed = {
                            let all_marker_shapes = all_marker_shapes_iters
                                .iter_mut()
                                .flat_map(|it| it.into_iter());
                            let all_marker_shapes_indices = all_marker_shapes_chunks
                                .iter()
                                .flat_map(|chunk| chunk.iter_component_indices(*query.timeline()));
                            itertools::izip!(all_marker_shapes_indices, all_marker_shapes)
                        };

                        let all_frames = re_query::range_zip_1x1(
                            all_scalars_indices(&query, all_scalar_chunks),
                            all_marker_shapes_indexed,
                        )
                        .enumerate();

                        // Simplified path for single series.
                        if num_series == 1 {
                            let points = &mut *points_per_series[0];
                            all_frames.for_each(|(i, (_index, _scalars, marker_shapes))| {
                                if let Some(marker) = marker_shapes
                                    .and_then(|marker_shapes| marker_shapes.first().copied())
                                {
                                    points[i].attrs.kind =
                                        PlotSeriesKind::Scatter(ScatterAttrs { marker });
                                }
                            });
                        } else {
                            all_frames.for_each(|(i, (_index, _scalars, marker_shapes))| {
                                if let Some(marker_shapes) = marker_shapes {
                                    for (points, marker) in points_per_series
                                        .iter_mut()
                                        .zip(clamped_or_nothing(&marker_shapes, num_series))
                                    {
                                        points[i].attrs.kind =
                                            PlotSeriesKind::Scatter(ScatterAttrs {
                                                marker: *marker,
                                            });
                                    }
                                }
                            });
                        }
                    }
                }
            }

            let series_visibility = collect_series_visibility(
                &query_ctx,
                &results,
                num_series,
                archetypes::SeriesPoints::descriptor_visible_series().component,
            );
            let series_names = collect_series_name(
                &query_ctx,
                &results,
                num_series,
                &archetypes::SeriesPoints::descriptor_names(),
            );

            let mut series = Vec::with_capacity(num_series);

            re_log::debug_assert!(
                points_per_series.len() <= series_names.len(),
                "Number of series names {} after processing should be at least the number of series allocated {}",
                series_names.len(),
                points_per_series.len()
            );
            for (instance, (points, label, visible)) in itertools::izip!(
                points_per_series.into_iter(),
                series_names.into_iter(),
                series_visibility.into_iter()
            )
            .enumerate()
            {
                let instance_path = if num_series == 1 {
                    InstancePath::entity_all(data_result.entity_path.clone())
                } else {
                    InstancePath::instance(data_result.entity_path.clone(), instance as u64)
                };

                if let Err(err) = util::points_to_series(
                    instance_path,
                    visible_time_range,
                    time_per_pixel,
                    visible,
                    points,
                    ctx.recording_engine().store(),
                    view_query,
                    label,
                    // Aggregation for points is not supported.
                    re_sdk_types::components::AggregationPolicy::Off,
                    &mut series,
                    instruction.id,
                ) {
                    results.report_error(format!("Failed to create series: {err}"));
                }
            }

            Ok(series)
        }
    }
}
