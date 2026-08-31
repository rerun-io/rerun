//! Rendering of the translucent error bands drawn around measurement series.

use crate::{PlotSeries, PlotSeriesKind};

/// Opacity of the filled error band.
const BAND_ALPHA: f32 = 0.2;

/// Depth of the error bands.
///
/// Lines sit at `0.0` and markers in front of them, so a positive value puts the band behind both.
const BAND_DEPTH: f32 = 1.0;

/// Builds the translucent error band for every series that has variances.
///
/// The band reaches one standard deviation, `sqrt(variance)`, above and below each value.
///
/// One mesh per series, since the tint that colors it is per instance. A translucent tint also puts
/// the mesh in `DrawPhase::Transparent`, which sorts back-to-front, so [`BAND_DEPTH`] keeps a band
/// under its line.
pub(crate) fn build_band_draw_data(
    all_series: &[PlotSeries],
    plot_transform: &egui_plot::PlotTransform,
    time_offset: i64,
    render_ctx: &re_renderer::RenderContext,
) -> Option<re_renderer::QueueableDrawData> {
    re_tracing::profile_function!();

    let to_screen = |[t, v]: [f64; 2]| {
        let screen_pos = plot_transform.position_from_point(&egui_plot::PlotPoint::new(t, v));
        glam::vec2(screen_pos.x, screen_pos.y)
    };

    let mut instances = Vec::new();
    for series in all_series {
        if !series.visible || series.variances.is_empty() || series.points.len() < 2 {
            continue;
        }
        re_log::debug_assert_eq!(series.variances.len(), series.points.len());

        let step_mode = match series.kind {
            PlotSeriesKind::Continuous => None,
            PlotSeriesKind::Stepped(mode) => Some(mode),
            PlotSeriesKind::Scatter(_) | PlotSeriesKind::Clear => continue,
        };

        let (upper, lower): (Vec<_>, Vec<_>) = std::iter::zip(&series.points, &series.variances)
            .map(|(&(time, value), &variance)| {
                // A non-positive variance means "no band", and guards `sqrt` against NaN.
                let offset = if variance > 0.0 {
                    f64::from(variance).sqrt()
                } else {
                    0.0
                };
                let time = (time - time_offset) as f64;
                ([time, value + offset], [time, value - offset])
            })
            .unzip();

        // Step the edges like the line, so the band follows the staircase.
        let step = |points: Vec<[f64; 2]>| match step_mode {
            Some(mode) => crate::view_class::to_stepped_points(&points, mode),
            None => points,
        };
        let (upper, lower) = (step(upper), step(lower));

        // One quad per adjacent pair. Non-finite values get no quad, breaking the band like they
        // break the line.
        let mut builder = re_renderer::ShapeBuilder::default();
        for (upper, lower) in std::iter::zip(upper.windows(2), lower.windows(2)) {
            if !std::iter::chain(upper, lower).all(|[_, v]| v.is_finite()) {
                continue;
            }
            builder.add_convex_polygon(&[
                to_screen(upper[0]),
                to_screen(upper[1]),
                to_screen(lower[1]),
                to_screen(lower[0]),
            ]);
        }
        if builder.is_empty() {
            continue;
        }

        // TODO(isaac): all bands could share a single mesh, which needs per-vertex colors in
        // `ShapeBuilder::into_cpu_mesh` (markers don't want that) and moving the tint from
        // `additive_tint` into `albedo_factor`.
        let cpu_mesh = builder.into_cpu_mesh(format!("band: {}", series.label), render_ctx);
        let gpu_mesh = match re_renderer::mesh::GpuMesh::new(render_ctx, &cpu_mesh) {
            Ok(gpu_mesh) => std::sync::Arc::new(gpu_mesh),
            Err(err) => {
                re_log::error_once!("Failed to build error band mesh: {err}");
                continue;
            }
        };

        instances.push(re_renderer::renderer::GpuMeshInstance {
            gpu_mesh,
            // TODO(andreas): could we use gpu transform instead of `to_screen` earlier? Unlike with the lines we don't have any problems with the line spanning since this is just a raw mesh anyways.
            // The catch is precision: projecting on the cpu happens in `f64` and only screen
            // coordinates reach the `f32` vertex buffer, whereas a gpu transform would put raw
            // plot-space values in there.
            world_from_mesh: glam::Affine3A::from_translation(glam::vec3(0.0, 0.0, BAND_DEPTH)),
            additive_tint: series.color.gamma_multiply(BAND_ALPHA),
            outline_mask_ids: re_renderer::OutlineMaskPreference::NONE,
            picking_layer_id: re_renderer::PickingLayerId::default(),
            cull_mode: None,
        });
    }

    if instances.is_empty() {
        return None;
    }

    match re_renderer::renderer::MeshDrawData::new(render_ctx, &instances) {
        Ok(draw_data) => Some(draw_data.into()),
        Err(err) => {
            re_log::error_once!("Failed to build error band MeshDrawData: {err}");
            None
        }
    }
}
