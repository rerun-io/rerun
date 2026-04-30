use itertools::Itertools as _;
use rayon::prelude::*;
use re_chunk_store::{LatestAtQuery, RangeQuery, RowId};
use re_log_types::{EntityPath, TimeInt};
use re_sdk_types::components::{self, AggregationPolicy, InterpolationMode, StrokeWidth};
use re_sdk_types::reflection::Enum as _;
use re_sdk_types::{Archetype as _, archetypes};
use re_view::{ChunksWithComponent, range_with_blueprint_resolved_data};
use re_viewer_context::external::re_entity_db::InstancePath;
use re_viewer_context::{
    IdentifiedViewSystem, SingleRequiredComponentConstraint, ViewContext, ViewQuery,
    ViewStateExt as _, ViewSystemExecutionError, VisualizerExecutionOutput, VisualizerQueryInfo,
    VisualizerReportSeverity, VisualizerSystem, typed_fallback_for,
};

use crate::series_query::{
    allocate_plot_points, collect_colors, collect_radius_ui, collect_scalars, collect_series_name,
    collect_series_visibility, determine_num_series,
};
use crate::{PlotPoint, PlotPointAttrs, PlotSeries, PlotSeriesKind, util};

/// Output data from [`SeriesLinesSystem`].
pub struct SeriesLinesOutput {
    pub all_series: Vec<PlotSeries>,
}

/// The system for rendering [`archetypes::SeriesLines`] archetypes.
#[derive(Default, Debug)]
pub struct SeriesLinesSystem;

impl IdentifiedViewSystem for SeriesLinesSystem {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "SeriesLines".into()
    }
}

impl VisualizerSystem for SeriesLinesSystem {
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo {
            relevant_archetype: archetypes::SeriesLines::name().into(),
            constraints: SingleRequiredComponentConstraint::new::<components::Scalar>(
                &archetypes::Scalars::descriptor_scalars(),
            )
            .with_additional_physical_types(util::series_supported_datatypes())
            .with_allow_static_data(false)
            .into(),

            queried: archetypes::Scalars::all_components()
                .iter()
                .chain(archetypes::SeriesLines::all_components().iter())
                .cloned()
                .collect(),
        }
    }

    fn execute(
        &self,
        ctx: &ViewContext<'_>,
        query: &ViewQuery<'_>,
        _context: &re_viewer_context::ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        re_tracing::profile_function!();

        let output = VisualizerExecutionOutput::default();

        let time_per_pixel = ctx
            .view_state
            .downcast_ref::<crate::view_class::TimeSeriesViewState>()
            .map_or(1.0, |state| state.time_per_pixel);

        let data_results: Vec<_> = query
            .iter_visualizer_instruction_for(Self::identifier())
            .collect();

        let all_series: Vec<_> = data_results
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

        let mut all_series_flat = Vec::new();
        all_series_flat.extend(all_series.into_iter().flatten());

        // Build re_renderer line draw data from the collected series.
        let draw_data = Self::build_draw_data(ctx, query, &all_series_flat)?;

        Ok(output
            .with_draw_data(draw_data)
            .with_visualizer_data(SeriesLinesOutput {
                all_series: all_series_flat,
            }))
    }
}

impl SeriesLinesSystem {
    fn build_draw_data(
        ctx: &ViewContext<'_>,
        query: &ViewQuery<'_>,
        all_series: &[PlotSeries],
    ) -> Result<Vec<re_renderer::QueueableDrawData>, ViewSystemExecutionError> {
        re_tracing::profile_function!();

        let render_ctx = ctx.viewer_ctx.render_ctx();

        let view_state = ctx
            .view_state
            .as_any()
            .downcast_ref::<crate::view_class::TimeSeriesViewState>();

        let time_offset = view_state.map_or(0, |state| state.time_offset);

        let plot_transform = view_state.and_then(|state| state.plot_transform);
        let Some(plot_transform) = plot_transform else {
            // First frame: no transform available yet.
            return Ok(Vec::new());
        };

        let mut num_strips = 0;
        let mut num_vertices = 0;

        for s in all_series {
            match s.kind {
                PlotSeriesKind::Continuous => {
                    num_strips += 1;
                    num_vertices += s.points.len();
                }
                PlotSeriesKind::Stepped(mode) => {
                    num_strips += 1;

                    let series_vertices = if s.points.len() < 2 {
                        s.points.len()
                    } else {
                        match mode {
                            crate::StepMode::After | crate::StepMode::Before => {
                                s.points.len() * 2 - 1
                            }
                            crate::StepMode::Mid => s.points.len() * 3 - 2,
                        }
                    };
                    num_vertices += series_vertices;
                }
                PlotSeriesKind::Clear => {}
                PlotSeriesKind::Scatter(_) => {
                    re_log::debug_panic!(
                        "Self::load_series produced an unexpected PlotSeriesKind: Scatter"
                    );
                }
            }
        }

        if num_strips == 0 {
            return Ok(Vec::new());
        }

        let mut line_builder = re_renderer::LineDrawableBuilder::new(render_ctx);
        // Plots render in screen space and don't benefit from MSAA-driven anti-aliasing of lines;
        // the default opaque pipeline relies on alpha-to-coverage which produces dithered edges
        // that look bad at typical plot line widths.
        line_builder.enable_alpha_blending();
        line_builder.reserve_strips(num_strips)?;
        line_builder.reserve_vertices(num_vertices)?;

        // Below 1.5 physical pixels width, we widen the line and fade its color
        // to keep sub-pixel strokes visible without aliasing.
        let pixels_per_point = ctx.viewer_ctx.egui_ctx().pixels_per_point();
        let min_line_radius_ui = 0.75 / pixels_per_point;

        for series in all_series {
            if !series.visible || series.points.is_empty() {
                continue;
            }

            let mut color = series.color;

            // Highlighted (hovered/selected) series get rendered with a thicker stroke
            let mut radius_ui = if crate::series_query::is_series_highlighted(query, series) {
                series.radius_ui + crate::markers::HIGHLIGHT_RADIUS_EXPANSION
            } else {
                series.radius_ui
            };

            // Lines below 1.5 physical px width look terrible, so instead reduce the opacity to fade them.
            if radius_ui < min_line_radius_ui {
                color = color.gamma_multiply(radius_ui / min_line_radius_ui);
                radius_ui = min_line_radius_ui;
            }

            // We don't do gpu transforms since that would transform the shape of things, and we
            // only want to transform the center position.
            let to_screen = |t: f64, v: f64| {
                let screen_pos =
                    plot_transform.position_from_point(&egui_plot::PlotPoint::new(t, v));
                glam::Vec2::new(screen_pos.x, screen_pos.y)
            };

            let screen_points: Vec<glam::Vec2> = match series.kind {
                PlotSeriesKind::Continuous => series
                    .points
                    .iter()
                    .map(|&(time, value)| to_screen((time - time_offset) as f64, value))
                    .collect(),
                PlotSeriesKind::Stepped(mode) => {
                    let raw_points: Vec<[f64; 2]> = series
                        .points
                        .iter()
                        .map(|&(time, value)| [(time - time_offset) as f64, value])
                        .collect();
                    crate::view_class::to_stepped_points(&raw_points, mode)
                        .iter()
                        .map(|p| to_screen(p[0], p[1]))
                        .collect()
                }
                PlotSeriesKind::Scatter(_) | PlotSeriesKind::Clear => continue,
            };

            let mut batch = line_builder.batch(series.label.clone()).picking_object_id(
                re_renderer::PickingLayerObjectId(series.instance_path.entity_path.hash64()),
            );

            batch
                .add_strip_2d(screen_points.into_iter())
                .color(color)
                .radius(re_renderer::Size::new_ui_points(radius_ui))
                .flags(
                    re_renderer::renderer::LineStripFlags::STRIP_FLAGS_OUTWARD_EXTENDING_ROUND_CAPS,
                );
        }

        // Single finite values surrounded by non-finite (NaN/±inf) neighbors get dropped by
        // the line builder (a strip needs ≥2 points). Render them as Circle markers so they
        // stay visible.
        let nan_island_draw_data = build_nan_island_marker_draw_data(
            ctx,
            query,
            all_series,
            &plot_transform,
            time_offset,
            render_ctx,
        );

        let mut draw_data: Vec<re_renderer::QueueableDrawData> =
            vec![line_builder.into_draw_data()?.into()];
        draw_data.extend(nan_island_draw_data);
        Ok(draw_data)
    }
}

fn build_nan_island_marker_draw_data(
    ctx: &ViewContext<'_>,
    query: &ViewQuery<'_>,
    all_series: &[PlotSeries],
    plot_transform: &egui_plot::PlotTransform,
    time_offset: i64,
    render_ctx: &re_renderer::RenderContext,
) -> Option<re_renderer::QueueableDrawData> {
    let marker_meshes = ctx
        .viewer_ctx
        .store_context
        .memoizer(|cache: &mut crate::markers::MarkerMeshCache| cache.get_or_build(render_ctx))?;

    let circle_mesh = marker_meshes.for_shape(re_sdk_types::components::MarkerShape::Circle);

    let mut instances = Vec::new();
    for series in all_series {
        if !series.visible || series.points.is_empty() {
            continue;
        }
        if !matches!(
            series.kind,
            PlotSeriesKind::Continuous | PlotSeriesKind::Stepped(_)
        ) {
            continue;
        }

        let mut radius = series.radius_ui;
        if crate::series_query::is_series_highlighted(query, series) {
            radius += crate::markers::HIGHLIGHT_RADIUS_EXPANSION;
        }

        let pts = &series.points;
        for i in 0..pts.len() {
            let (time, value) = pts[i];
            if !value.is_finite() {
                continue;
            }
            let prev_finite = i > 0 && pts[i - 1].1.is_finite();
            let next_finite = i + 1 < pts.len() && pts[i + 1].1.is_finite();
            if prev_finite || next_finite {
                continue;
            }
            let center = plot_transform.position_from_point(&egui_plot::PlotPoint::new(
                (time.saturating_sub(time_offset)) as f64,
                value,
            ));
            instances.push(crate::markers::marker_instance(
                circle_mesh.clone(),
                glam::vec2(center.x, center.y),
                radius,
                series.color,
            ));
        }
    }

    if instances.is_empty() {
        return None;
    }

    match re_renderer::renderer::MeshDrawData::new(render_ctx, &instances) {
        Ok(draw_data) => Some(draw_data.into()),
        Err(err) => {
            re_log::error_once!("Failed to build NaN-island marker MeshDrawData: {err}");
            None
        }
    }
}

impl SeriesLinesSystem {
    fn load_series(
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        time_per_pixel: f64,
        data_result: &re_viewer_context::DataResult,
        instruction: &re_viewer_context::VisualizerInstruction,
        output: &re_viewer_context::VisualizerExecutionOutput,
    ) -> Vec<PlotSeries> {
        re_tracing::profile_function!(data_result.entity_path.to_string());

        let current_query = ctx.current_query();
        let query_ctx = ctx.query_context(data_result, current_query.clone(), instruction.id);

        let data_time_range =
            util::data_result_time_range(ctx.viewer_ctx, data_result, view_query.timeline);
        let query_range = match util::determine_query_range(ctx, data_time_range) {
            Ok(range) => range,
            Err(err) => {
                output.report_unspecified_source(
                    instruction.id,
                    VisualizerReportSeverity::Error,
                    format!("Failed to determine query range: {err}"),
                );
                return Vec::new();
            }
        };
        let query = re_chunk_store::RangeQuery::new(view_query.timeline, query_range)
            // We must fetch data with extended bounds, otherwise the query clamping would
            // cut-off the data early at the edge of the view.
            .include_extended_bounds(true);

        let mut results = range_with_blueprint_resolved_data(
            ctx,
            None,
            &query,
            data_result,
            archetypes::Scalars::all_component_identifiers()
                .chain(archetypes::SeriesLines::all_component_identifiers()),
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
            archetypes::SeriesLines::optional_components()
                .iter()
                .map(|c| c.component),
            Some(instruction),
        ));

        // Wrap results for convenient error-reporting iteration
        let results = re_view::BlueprintResolvedResults::Range(query.clone(), results);
        let results =
            re_view::VisualizerInstructionQueryResults::new(instruction, &results, output);

        // If we have no scalars, we can't do anything.
        let scalar_component = archetypes::Scalars::descriptor_scalars().component;
        let scalar_iter = results.iter_required(scalar_component);
        let all_scalar_chunks = scalar_iter.chunks();

        // Filter out static times if any slipped in.
        // It's enough to check the first one chunk since an entire column has to be either temporal or static.
        let empty_chunks;
        let all_scalar_chunks = if let Some(chunk) = all_scalar_chunks.chunks.first()
            && chunk.is_static()
        {
            results.report_for_component(scalar_component, VisualizerReportSeverity::Error, "Can't plot data that was logged statically in a time series since there's no temporal dimension");
            empty_chunks = ChunksWithComponent::empty(scalar_component);
            &empty_chunks // Proceed with empty data so we catch other errors as well.
        } else {
            all_scalar_chunks
        };

        // All the default values for a `PlotPoint`, accounting for both overrides and default values.
        // We know there's only a single value fallback for stroke width, so this is fine, albeit a bit hacky in case we add an array fallback later.
        let fallback_stroke_width: StrokeWidth = typed_fallback_for(
            &query_ctx,
            archetypes::SeriesLines::descriptor_widths().component,
        );

        let interpolation_mode = results
            .iter_optional(archetypes::SeriesLines::descriptor_interpolation_mode().component)
            .slice::<u8>()
            .next()
            .and_then(|(_, s)| InterpolationMode::from_integer_slice(s).next()?)
            .unwrap_or_default();

        let plot_kind = match interpolation_mode {
            InterpolationMode::Linear => PlotSeriesKind::Continuous,
            InterpolationMode::StepAfter => PlotSeriesKind::Stepped(crate::StepMode::After),
            InterpolationMode::StepBefore => PlotSeriesKind::Stepped(crate::StepMode::Before),
            InterpolationMode::StepMid => PlotSeriesKind::Stepped(crate::StepMode::Mid),
        };

        let default_point = PlotPoint {
            time: 0,
            value: 0.0,
            attrs: PlotPointAttrs {
                // Filled out later.
                color: egui::Color32::DEBUG_COLOR,
                radius_ui: 0.5 * *fallback_stroke_width.0,
                kind: plot_kind,
            },
        };

        let num_series = determine_num_series(all_scalar_chunks, &results);
        let mut points_per_series =
            allocate_plot_points(&query, &default_point, all_scalar_chunks, num_series);

        collect_scalars(all_scalar_chunks, &mut points_per_series);

        collect_colors(
            &query,
            &results,
            all_scalar_chunks,
            &mut points_per_series,
            &archetypes::SeriesLines::descriptor_colors(),
        );
        collect_radius_ui(
            &query,
            &results,
            all_scalar_chunks,
            &mut points_per_series,
            &archetypes::SeriesLines::descriptor_widths(),
            0.5,
        );

        // Now convert the `PlotPoints` into `Vec<PlotSeries>`
        let aggregator = results
            .iter_optional(archetypes::SeriesLines::descriptor_aggregation_policy().component)
            .slice::<u8>()
            .next()
            .and_then(|(_, s)| AggregationPolicy::from_integer_slice(s).next()?)
            // TODO(andreas): Relying on the default==placeholder here instead of going through a fallback provider.
            //                This is fine, because we know there's no `TypedFallbackProvider`, but wrong if one were to be added.
            .unwrap_or_default();

        // NOTE: The chunks themselves are already sorted as best as possible (hint: overlap)
        // by the query engine.
        let all_chunks_sorted_and_not_overlapped =
            all_scalar_chunks.iter().tuple_windows().all(|(lhs, rhs)| {
                let lhs_time_max = lhs
                    .chunk
                    .timelines()
                    .get(query.timeline())
                    .map_or(TimeInt::MAX, |time_column| time_column.time_range().max());
                let rhs_time_min = rhs
                    .chunk
                    .timelines()
                    .get(query.timeline())
                    .map_or(TimeInt::MIN, |time_column| time_column.time_range().min());
                lhs_time_max <= rhs_time_min
            });

        let has_discontinuities = {
            // Find all clears that may apply, in order to render discontinuities properly.

            re_tracing::profile_scope!("discontinuities");

            let cleared_indices = collect_recursive_clears(ctx, &query, &data_result.entity_path);
            let has_discontinuities = !cleared_indices.is_empty();

            for points in &mut points_per_series {
                points.extend(cleared_indices.iter().map(|(data_time, _)| PlotPoint {
                    time: data_time.as_i64(),
                    value: 0.0,
                    attrs: PlotPointAttrs {
                        color: egui::Color32::TRANSPARENT,
                        radius_ui: 0.0,
                        kind: PlotSeriesKind::Clear,
                    },
                }));
            }

            has_discontinuities
        };

        // This is _almost_ sorted already: all the individual chunks are sorted, but we still
        // have to deal with overlapped chunks, or discontinuities introduced by query-time clears.
        if !all_chunks_sorted_and_not_overlapped || has_discontinuities {
            re_tracing::profile_scope!("sort");
            for points in &mut points_per_series {
                re_tracing::profile_scope!("sort_by_key", points.len().to_string());
                points.sort_by_key(|p| p.time);
            }
        }

        let series_visibility = collect_series_visibility(
            &results,
            num_series,
            &archetypes::SeriesLines::descriptor_visible_series(),
        );
        let series_names = collect_series_name(
            &results,
            num_series,
            &archetypes::SeriesLines::descriptor_names(),
        );

        let mut series = Vec::with_capacity(num_series);

        re_log::debug_assert!(
            points_per_series.len() <= series_names.len(),
            "Number of series names {} after processing should be at least the number of series allocated {}",
            series_names.len(),
            points_per_series.len()
        );
        for (instance, (points, label, visible)) in
            itertools::izip!(points_per_series, series_names, series_visibility).enumerate()
        {
            let instance_path = if num_series == 1 {
                InstancePath::entity_all(data_result.entity_path.clone())
            } else {
                InstancePath::instance(data_result.entity_path.clone(), instance as u64)
            };

            util::points_to_series(
                instance_path,
                time_per_pixel,
                visible,
                points,
                ctx.recording_engine().store(),
                view_query,
                label,
                aggregator,
                &mut series,
                instruction.id,
            );
        }

        series
    }
}

fn collect_recursive_clears(
    ctx: &ViewContext<'_>,
    query: &RangeQuery,
    entity_path: &EntityPath,
) -> Vec<(TimeInt, RowId)> {
    re_tracing::profile_function!();

    let mut cleared_indices = Vec::new();

    let mut clear_entity_path = entity_path.clone();
    let clear_descriptor = archetypes::Clear::descriptor_is_recursive();

    // Bootstrap in case there's a pending clear out of the visible time range.
    {
        let results = ctx.recording_engine().cache().latest_at(
            &LatestAtQuery::new(query.timeline, query.range.min()),
            &clear_entity_path,
            [clear_descriptor.component],
        );

        cleared_indices.extend(
            results
                .get(clear_descriptor.component)
                .iter()
                .flat_map(|chunk| {
                    itertools::izip!(
                        chunk.iter_component_indices(*query.timeline(), clear_descriptor.component),
                        chunk.iter_slices::<bool>(clear_descriptor.component)
                    )
                })
                .filter_map(|(index, is_recursive_buffer)| {
                    let is_recursive =
                        !is_recursive_buffer.is_empty() && is_recursive_buffer.value(0);
                    (is_recursive || clear_entity_path == *entity_path).then_some(index)
                }),
        );
    }

    loop {
        let results = ctx.recording_engine().cache().range(
            query,
            &clear_entity_path,
            [clear_descriptor.component],
        );

        cleared_indices.extend(
            results
                .get(clear_descriptor.component)
                .unwrap_or_default()
                .iter()
                .flat_map(|chunk| {
                    itertools::izip!(
                        chunk.iter_component_indices(*query.timeline(), clear_descriptor.component),
                        chunk.iter_slices::<bool>(clear_descriptor.component)
                    )
                })
                .filter_map(|(index, is_recursive_buffer)| {
                    let is_recursive =
                        !is_recursive_buffer.is_empty() && is_recursive_buffer.value(0);
                    (is_recursive || clear_entity_path == *entity_path).then_some(index)
                }),
        );

        let Some(parent_entity_path) = clear_entity_path.parent() else {
            break;
        };

        clear_entity_path = parent_entity_path;
    }

    cleared_indices
}
