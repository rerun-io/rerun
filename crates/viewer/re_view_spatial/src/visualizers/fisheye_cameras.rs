use glam::vec3;
use re_renderer::renderer::LineStripFlags;
use re_sdk_types::Archetype as _;
use re_sdk_types::archetypes::Fisheye;
use re_sdk_types::components;
use re_view::latest_at_with_blueprint_resolved_data;
use re_viewer_context::{
    IdentifiedViewSystem, ViewContext, ViewContextCollection, ViewQuery, ViewSystemExecutionError,
    VisualizerExecutionOutput, VisualizerQueryInfo, VisualizerReportSeverity, VisualizerSystem,
};

use super::SpatialViewVisualizerData;
use super::cameras::{CameraComponentDataWithFallbacks, visit_camera_instance};
use crate::contexts::TransformTreeContext;
use crate::pinhole_wrapper::PinholeWrapper;
use crate::visualizers::process_radius;
use crate::visualizers::utilities::spatial_view_kind_from_view_class;

pub struct FisheyeCamerasVisualizer {
    pub data: SpatialViewVisualizerData,
    pub pinhole_cameras: Vec<PinholeWrapper>,
}

impl Default for FisheyeCamerasVisualizer {
    fn default() -> Self {
        Self {
            data: (SpatialViewVisualizerData::new(None)),
            pinhole_cameras: Vec::new(),
        }
    }
}

impl IdentifiedViewSystem for FisheyeCamerasVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "FisheyeCameras".into()
    }
}

/// Build frustum line strips for a fisheye camera with curved edges.
///
/// Uses the equidistant model: `theta_d = theta * (1 + k1*theta^2 + k2*theta^4 + k3*theta^6 + k4*theta^8)`
fn fisheye_frustum_strips(
    pinhole: &crate::Pinhole,
    coeffs: [f32; 4],
    w: f32,
    h: f32,
    z: f32,
) -> Vec<(Vec<glam::Vec3>, LineStripFlags)> {
    const NUM_SEGMENTS: usize = 24;

    let unproject = |u: f32, v: f32| -> glam::Vec3 { fisheye_unproject(pinhole, coeffs, u, v, z) };

    // Sample many points along an image-space edge and unproject each.
    let sample_edge = |u0: f32, v0: f32, u1: f32, v1: f32| -> Vec<glam::Vec3> {
        (0..=NUM_SEGMENTS)
            .map(|i| {
                let t = i as f32 / NUM_SEGMENTS as f32;
                unproject(u0 + t * (u1 - u0), v0 + t * (v1 - v0))
            })
            .collect()
    };

    let corners = [
        unproject(0.0, 0.0),
        unproject(0.0, h),
        unproject(w, h),
        unproject(w, 0.0),
    ];

    let flags = LineStripFlags::FLAGS_OUTWARD_EXTENDING_ROUND_CAPS;
    let mut strips = vec![
        // Rectangle edges (curved).
        (sample_edge(0.0, 0.0, 0.0, h), flags), // left
        (sample_edge(0.0, h, w, h), flags),     // bottom
        (sample_edge(w, h, w, 0.0), flags),     // right
        (sample_edge(0.0, 0.0, w, 0.0), flags), // top
        // Up triangle (curved edges).
        (
            sample_edge(0.4 * w, 0.0, 0.5 * w, -0.1 * w),
            LineStripFlags::empty(),
        ),
        (
            sample_edge(0.5 * w, -0.1 * w, 0.6 * w, 0.0),
            LineStripFlags::empty(),
        ),
    ];

    // Rays from origin to each corner (always straight).
    for &corner in &corners {
        strips.push((vec![glam::Vec3::ZERO, corner], flags));
    }

    strips
}

/// Unproject a pixel coordinate using the fisheye equidistant model.
///
/// Given pixel `(u, v)` and image plane distance `dist`, returns the 3D point in camera space.
/// Unlike pinhole unprojection, the distance is interpreted as radial distance from the camera
/// origin (on a hemisphere), not depth along the optical axis. This avoids the singularity at
/// theta = pi/2 and produces a natural dome-shaped frustum for wide-angle fisheye cameras.
pub(crate) fn fisheye_unproject(
    pinhole: &crate::Pinhole,
    coeffs: [f32; 4],
    u: f32,
    v: f32,
    dist: f32,
) -> glam::Vec3 {
    let pp = pinhole.principal_point();
    let fl = pinhole.focal_length_in_pixels();

    // Normalized image coordinates.
    let x_prime = (u - pp.x) / fl.x;
    let y_prime = (v - pp.y) / fl.y;
    let theta_d = x_prime.hypot(y_prime);

    if theta_d < 1e-8 {
        return vec3(0.0, 0.0, dist);
    }

    // Newton's method: solve theta_d = theta * (1 + k1*theta^2 + k2*theta^4 + k3*theta^6 + k4*theta^8)
    let [k1, k2, k3, k4] = coeffs;
    let mut theta = theta_d;
    for _ in 0..10 {
        let theta2 = theta * theta;
        let theta4 = theta2 * theta2;
        let theta6 = theta4 * theta2;
        let theta8 = theta4 * theta4;
        let f_val = theta * (1.0 + k1 * theta2 + k2 * theta4 + k3 * theta6 + k4 * theta8) - theta_d;
        let f_deriv =
            1.0 + 3.0 * k1 * theta2 + 5.0 * k2 * theta4 + 7.0 * k3 * theta6 + 9.0 * k4 * theta8;
        if f_deriv.abs() < 1e-12 {
            break;
        }
        theta -= f_val / f_deriv;
    }

    // Clamp: rays beyond pi go behind the camera and make no sense for visualization.
    theta = theta.clamp(0.0, std::f32::consts::PI * 0.95);

    // 3D ray direction from the equidistant model (already a unit vector).
    let s = theta.sin() / theta_d;
    let dir = vec3(s * x_prime, s * y_prime, theta.cos());

    // Scale to radial distance `dist` from the camera origin.
    dir * dist
}

impl VisualizerSystem for FisheyeCamerasVisualizer {
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<Fisheye>()
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        query: &ViewQuery<'_>,
        context_systems: &ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        let output = VisualizerExecutionOutput::default();

        let transforms = context_systems.get::<TransformTreeContext>(&output)?;
        let view_kind = spatial_view_kind_from_view_class(ctx.view_class_identifier);

        let mut line_builder = re_renderer::LineDrawableBuilder::new(ctx.viewer_ctx.render_ctx());
        line_builder.radius_boost_in_ui_points_for_outlines(
            re_view::SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES,
        );

        for (data_result, instruction) in query.iter_visualizer_instruction_for(Self::identifier())
        {
            let time_query = re_chunk_store::LatestAtQuery::new(query.timeline, query.latest_at);

            let query_results = latest_at_with_blueprint_resolved_data(
                ctx,
                None,
                &time_query,
                data_result,
                Fisheye::all_component_identifiers().collect::<Vec<_>>(),
                Some(instruction),
            );

            // `image_from_camera` _is_ the required component, but we don't process it further since we rely on the
            // pinhole information from the transform tree instead, which already has this and other properties queried.
            if query_results
                .get_mono::<components::PinholeProjection>(
                    Fisheye::descriptor_image_from_camera().component,
                )
                .is_none()
            {
                continue;
            }

            let camera_xyz = query_results.get_mono_with_fallback::<components::ViewCoordinates>(
                Fisheye::descriptor_camera_xyz().component,
            );
            let child_frame = query_results.get_mono_with_fallback::<components::TransformFrameId>(
                Fisheye::descriptor_child_frame().component,
            );
            let image_plane_distance = query_results
                .get_mono_with_fallback::<components::ImagePlaneDistance>(
                    Fisheye::descriptor_image_plane_distance().component,
                );
            let color = query_results
                .get_mono_with_fallback::<components::Color>(Fisheye::descriptor_color().component)
                .into();
            let line_width = process_radius(
                &data_result.entity_path,
                query_results.get_mono_with_fallback::<components::Radius>(
                    Fisheye::descriptor_line_width().component,
                ),
            );

            let coeffs = query_results
                .get_mono::<components::FisheyeCoefficients>(
                    Fisheye::descriptor_distortion_coefficients().component,
                )
                .unwrap_or_default();
            let coeffs = [coeffs.k1(), coeffs.k2(), coeffs.k3(), coeffs.k4()];

            let component_data = CameraComponentDataWithFallbacks {
                child_frame,
                color,
                line_width,
                camera_xyz,
                image_plane_distance: image_plane_distance.into(),
            };

            let entity_highlight = query
                .highlights
                .entity_outline_mask(data_result.entity_path.hash());

            if let Err(err) = visit_camera_instance(
                &mut self.data,
                &mut self.pinhole_cameras,
                &ctx.query_context(data_result, query.latest_at_query(), instruction.id),
                &mut line_builder,
                transforms,
                &component_data,
                entity_highlight,
                view_kind,
                |pinhole, w, h, z| fisheye_frustum_strips(pinhole, coeffs, w, h, z),
            ) {
                output.report_unspecified_source(
                    instruction.id,
                    VisualizerReportSeverity::Error,
                    err,
                );
            }
        }

        Ok(output.with_draw_data([(line_builder.into_draw_data()?.into())]))
    }

    fn data(&self) -> Option<&dyn std::any::Any> {
        Some(self.data.as_any())
    }
}
