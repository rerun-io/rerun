use rayon::prelude::*;
use re_sdk_types::components;
use re_sdk_types::{Archetype as _, archetypes};
use re_viewer_context::{
    IdentifiedViewSystem, SingleRequiredComponentConstraint, ViewContext, ViewQuery,
    ViewStateExt as _, ViewSystemExecutionError, VisualizerExecutionOutput, VisualizerQueryInfo,
    VisualizerSystem,
};

use crate::error_bands::build_band_draw_data;
use crate::line_series_loader::{
    LineSeriesSource, LineSeriesStyling, load_line_series_with_styling,
};
use crate::{PlotSeries, PlotSeriesKind, util};

/// Output data from [`SeriesLinesSystem`].
#[derive(Default, Clone)]
pub struct SeriesLinesOutput {
    pub all_series: Vec<PlotSeries>,
}

/// The system for rendering [`archetypes::SeriesLines`] archetypes.
#[derive(Default, Debug)]
pub struct SeriesLinesSystem;

impl IdentifiedViewSystem for SeriesLinesSystem {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        re_viewer_context::external::re_string_interner::intern_static!(
            re_viewer_context::ViewSystemIdentifier,
            "SeriesLines"
        )
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
            .with_additional_physical_types(util::series_supported_encodings())
            .with_allow_static_data(false)
            .into(),

            queried: std::iter::chain(
                archetypes::Scalars::all_components().iter(),
                archetypes::SeriesLines::all_components().iter(),
            )
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
        let draw_data = build_line_draw_data(ctx, query, &all_series_flat)?;

        Ok(output
            .with_draw_data(draw_data)
            .with_visualizer_data(SeriesLinesOutput {
                all_series: all_series_flat,
            }))
    }
}

/// Builds GPU-rendered line strips (and surrounding-NaN markers) for a slice of [`PlotSeries`].
pub(crate) fn build_line_draw_data(
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
                        crate::StepMode::After | crate::StepMode::Before => s.points.len() * 2 - 1,
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
            let screen_pos = plot_transform.position_from_point(&egui_plot::PlotPoint::new(t, v));
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
            .flags(re_renderer::renderer::LineStripFlags::STRIP_FLAGS_OUTWARD_EXTENDING_ROUND_CAPS);
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

    let band_draw_data = build_band_draw_data(all_series, &plot_transform, time_offset, render_ctx);

    let mut draw_data: Vec<re_renderer::QueueableDrawData> =
        vec![line_builder.into_draw_data()?.into()];
    draw_data.extend(nan_island_draw_data);
    draw_data.extend(band_draw_data);
    draw_data.extend(build_always_on_marker_draw_data(
        ctx,
        query,
        all_series,
        &plot_transform,
        time_offset,
        render_ctx,
    ));
    Ok(draw_data)
}

fn build_always_on_marker_draw_data(
    ctx: &ViewContext<'_>,
    query: &ViewQuery<'_>,
    all_series: &[PlotSeries],
    plot_transform: &egui_plot::PlotTransform,
    time_offset: i64,
    render_ctx: &re_renderer::RenderContext,
) -> Option<re_renderer::QueueableDrawData> {
    if !ctx
        .view_state
        .downcast_ref::<crate::view_class::TimeSeriesViewState>()
        .is_ok_and(|state| state.always_show_line_data_markers)
    {
        return None;
    }

    let marker_meshes = ctx
        .viewer_ctx
        .store_context
        .memoizer(|cache: &mut crate::markers::MarkerMeshCache| cache.get_or_build(render_ctx))?;
    let circle_mesh = marker_meshes.for_shape(re_sdk_types::components::MarkerShape::Circle);

    let mut instances = Vec::new();
    for series in all_series {
        if !series.visible
            || series.is_aggregated()
            || !matches!(
                series.kind,
                PlotSeriesKind::Continuous | PlotSeriesKind::Stepped(_)
            )
        {
            continue;
        }

        let mut radius = crate::markers::ALWAYS_ON_LINE_MARKER_MIN_RADIUS_UI
            .max(series.radius_ui * crate::markers::ALWAYS_ON_LINE_MARKER_RADIUS_MULTIPLIER);
        if crate::series_query::is_series_highlighted(query, series) {
            radius += crate::markers::HIGHLIGHT_RADIUS_EXPANSION;
        }

        let sample_points = series
            .points
            .iter()
            .skip(usize::from(series.first_point_is_continuity_bridge));
        for &(time, value) in sample_points {
            if !value.is_finite() {
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
            re_log::error_once!("Failed to build always-on marker MeshDrawData: {err}");
            None
        }
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
        load_line_series_with_styling(
            ctx,
            view_query,
            time_per_pixel,
            data_result,
            instruction,
            output,
            &LineSeriesSource {
                value_descriptor: archetypes::Scalars::descriptor_scalars(),
                queried_components: std::iter::chain(
                    archetypes::Scalars::all_component_identifiers(),
                    archetypes::SeriesLines::all_component_identifiers(),
                )
                .collect(),
                styling: LineSeriesStyling::series_lines(),
                variance_descriptor: None,
                unit_descriptor: None,
            },
        )
    }
}
