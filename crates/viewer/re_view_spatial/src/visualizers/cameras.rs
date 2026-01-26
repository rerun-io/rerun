use glam::vec3;
use re_log_types::Instance;
use re_renderer::renderer::LineStripFlags;
use re_sdk_types::Archetype as _;
use re_sdk_types::archetypes::Pinhole;
use re_sdk_types::components;
use re_view::latest_at_with_blueprint_resolved_data;
use re_viewer_context::{
    IdentifiedViewSystem, ViewContext, ViewContextCollection, ViewOutlineMasks, ViewQuery,
    ViewSystemExecutionError, VisualizerExecutionOutput, VisualizerQueryInfo, VisualizerSystem,
    typed_fallback_for,
};

use super::SpatialViewVisualizerData;
use crate::contexts::TransformTreeContext;
use crate::pinhole_wrapper::PinholeWrapper;
use crate::view_kind::SpatialViewKind;
use crate::visualizers::process_radius;
use crate::visualizers::utilities::spatial_view_kind_from_view_class;

pub struct CamerasVisualizer {
    pub data: SpatialViewVisualizerData,
    pub pinhole_cameras: Vec<PinholeWrapper>,
}

impl Default for CamerasVisualizer {
    fn default() -> Self {
        Self {
            // Cameras themselves aren't inherently 2D or 3D since they represent intrinsics.
            // (extrinsics, represented by [`transform3d_arrow::Transform3DArrowsPart`] are 3D though)
            data: (SpatialViewVisualizerData::new(None)),
            pinhole_cameras: Vec::new(),
        }
    }
}

impl IdentifiedViewSystem for CamerasVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Cameras".into()
    }
}

struct CameraComponentDataWithFallbacks {
    child_frame: components::TransformFrameId,

    color: egui::Color32,
    line_width: re_renderer::Size,
    camera_xyz: components::ViewCoordinates,
    image_plane_distance: f32,
}

impl CamerasVisualizer {
    fn visit_instance(
        &mut self,
        ctx: &re_viewer_context::QueryContext<'_>,
        line_builder: &mut re_renderer::LineDrawableBuilder<'_>,
        transforms: &TransformTreeContext,
        pinhole_properties: &CameraComponentDataWithFallbacks,
        entity_highlight: &ViewOutlineMasks,
        view_kind: SpatialViewKind,
    ) -> Result<(), String> {
        let instance = Instance::from(0);
        let ent_path = ctx.target_entity_path;

        // We're currently NOT using `CoordinateFrame` component for this visualization but instead `Pinhole::child_frame`.
        // Otherwise, you'd need to log a redundant `CoordinateFrame` to see the camera frustum, which can be unintuitive.
        //
        // Note that `child_frame` defaults to the entity's implicit frame, so if no frames are set, it doesn't make a difference.
        //
        // In theory, `CoordinateFrame::frame_id` and `Pinhole::child_frame` could disagree, making it unclear what to show.
        // Sticking with the semantics of `CoordinateFrame::frame_id`, we should give it precedence,
        // but this implies ignoring `CoordinateFrame::frame_id`'s fallback in all other cases, which is arguably
        // even more confusing. So instead, we rely _solely_ on `Pinhole::child_frame` for now.
        let pinhole_frame_id = re_tf::TransformFrameIdHash::new(&pinhole_properties.child_frame);

        // Query the pinhole from the transform tree since it uses atomic-latest-at.
        let Some(pinhole_tree_root_info) = transforms.pinhole_tree_root_info(pinhole_frame_id)
        else {
            // This implies that the transform context didn't see the pinhole transform.
            // This can happen with various frame id mismatches. TODO(andreas): When exactly does this happen? Can we add a unit test and improve the message?
            return Err(format!(
                "The pinhole's child frame {:?} does not form the root of a 2D subspace. Ensure you're transform tree is valid.",
                transforms.format_frame(pinhole_frame_id)
            ));
        };
        let resolved_pinhole = &pinhole_tree_root_info.pinhole_projection;

        // Check for a valid resolution.
        let resolution = resolved_pinhole
            .resolution
            .unwrap_or_else(|| typed_fallback_for(ctx, Pinhole::descriptor_resolution().component));
        let w = resolution.x();
        let h = resolution.y();
        let z = pinhole_properties.image_plane_distance;
        if !w.is_finite() || !h.is_finite() || w <= 0.0 || h <= 0.0 {
            return Err("Invalid pinhole resolution.".to_owned());
        }

        let pinhole = crate::Pinhole {
            image_from_camera: (*resolved_pinhole.image_from_camera).into(),
            resolution: glam::vec2(w, h),
        };

        // If the camera is the target frame of a 2D view, there is nothing for us to display.
        if transforms.target_frame() == pinhole_frame_id && view_kind == SpatialViewKind::TwoD {
            self.pinhole_cameras.push(PinholeWrapper {
                ent_path: ent_path.clone(),
                pinhole_view_coordinates: pinhole_properties.camera_xyz,
                world_from_camera: macaw::IsoTransform::IDENTITY,
                pinhole,
                picture_plane_distance: pinhole_properties.image_plane_distance,
            });
            return Err("Can't visualize pinholes at the view's origin".to_owned());
        }

        // If this transform is not representable as an `IsoTransform` we can't display it yet.
        // This would happen if the camera is under another camera or under a transform with non-uniform scale.
        let Some(world_from_camera) = transforms.target_from_pinhole_root(pinhole_frame_id) else {
            return Err("Pinhole is not connected to the view's target frame.".to_owned());
        };
        let world_from_camera = world_from_camera.as_affine3a();
        let Some(world_from_camera_iso) = macaw::IsoTransform::from_mat4(&world_from_camera.into())
        else {
            return Err("Can only visualize pinhole under isometric transforms.".to_owned());
        };

        debug_assert!(world_from_camera_iso.is_finite());

        self.pinhole_cameras.push(PinholeWrapper {
            ent_path: ent_path.clone(),
            pinhole_view_coordinates: pinhole_properties.camera_xyz,
            world_from_camera: world_from_camera_iso,
            pinhole,
            picture_plane_distance: pinhole_properties.image_plane_distance,
        });

        // Setup a RDF frustum (for non-RDF we apply a transformation matrix later).
        let corners = [
            pinhole.unproject(vec3(0.0, 0.0, z)),
            pinhole.unproject(vec3(0.0, h, z)),
            pinhole.unproject(vec3(w, h, z)),
            pinhole.unproject(vec3(w, 0.0, z)),
        ];

        let up_triangle = [
            pinhole.unproject(vec3(0.4 * w, 0.0, z)),
            pinhole.unproject(vec3(0.5 * w, -0.1 * w, z)),
            pinhole.unproject(vec3(0.6 * w, 0.0, z)),
        ];

        let strips = vec![
            // Frustum rectangle, connected with zero point.
            (
                vec![
                    corners[0],
                    corners[1],
                    glam::Vec3::ZERO,
                    corners[2],
                    corners[3],
                    glam::Vec3::ZERO,
                    corners[0],
                    corners[3],
                    glam::Vec3::ZERO,
                ],
                LineStripFlags::FLAGS_OUTWARD_EXTENDING_ROUND_CAPS,
            ),
            // Missing piece of the rectangle at the far plane.
            (
                vec![corners[1], corners[2]],
                LineStripFlags::FLAGS_OUTWARD_EXTENDING_ROUND_CAPS,
            ),
            // Triangle indicating up direction.
            // Don't extend round caps here, this would reach into the frustum otherwise.
            (
                vec![up_triangle[0], up_triangle[1], up_triangle[2]],
                LineStripFlags::empty(),
            ),
        ];

        let radius = pinhole_properties.line_width;
        let instance_path_for_picking =
            re_entity_db::InstancePathHash::instance(ent_path, instance);
        let instance_layer_id =
            re_view::picking_layer_id_from_instance_path_hash(instance_path_for_picking);

        let mut batch = line_builder
            .batch(ent_path.to_string())
            // The frustum is setup as a RDF frustum, but if the view coordinates are not RDF,
            // we need to reorient the displayed frustum so that we indicate the correct orientation in the 3D world space.
            .world_from_obj(
                world_from_camera
                    * glam::Affine3A::from_mat3(pinhole_properties.camera_xyz.from_rdf()),
            )
            .outline_mask_ids(entity_highlight.overall)
            .picking_object_id(instance_layer_id.object);

        for (strip, flags) in strips {
            let lines = batch
                .add_strip(strip.into_iter())
                .radius(radius)
                .color(pinhole_properties.color)
                .flags(flags)
                .picking_instance_id(instance_layer_id.instance);

            if let Some(outline_mask_ids) = entity_highlight.instances.get(&instance) {
                lines.outline_mask_ids(*outline_mask_ids);
            }
        }

        // world_from_camera is the transform to the pinhole origin.
        self.data
            .add_bounding_box(ent_path.hash(), macaw::BoundingBox::ZERO, world_from_camera);

        Ok(())
    }
}

impl VisualizerSystem for CamerasVisualizer {
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<Pinhole>()
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        query: &ViewQuery<'_>,
        context_systems: &ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        let mut output = VisualizerExecutionOutput::default();

        let transforms = context_systems.get::<TransformTreeContext>()?;
        let view_kind = spatial_view_kind_from_view_class(ctx.view_class_identifier);

        // Counting all cameras ahead of time is a bit wasteful, but we also don't expect a huge amount,
        // so let re_renderer's allocator internally decide what buffer sizes to pick & grow them as we go.
        let mut line_builder = re_renderer::LineDrawableBuilder::new(ctx.viewer_ctx.render_ctx());
        line_builder.radius_boost_in_ui_points_for_outlines(
            re_view::SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES,
        );

        for (data_result, instruction) in query.iter_visualizer_instruction_for(Self::identifier())
        {
            let time_query = re_chunk_store::LatestAtQuery::new(query.timeline, query.latest_at);

            let query_shadowed_components = false;
            let query_results = latest_at_with_blueprint_resolved_data(
                ctx,
                None,
                &time_query,
                data_result,
                Pinhole::all_component_identifiers(),
                query_shadowed_components,
                Some(instruction),
            );

            // `image_from_camera` _is_ the required component, but we don't process it further since we rely on the
            // pinhole information from the transform tree instead, which already has this and other properties queried.
            if query_results
                .get_required_mono::<components::PinholeProjection>(
                    Pinhole::descriptor_image_from_camera().component,
                )
                .is_none()
            {
                continue;
            }

            let camera_xyz = query_results.get_mono_with_fallback::<components::ViewCoordinates>(
                Pinhole::descriptor_camera_xyz().component,
            );
            let child_frame = query_results.get_mono_with_fallback::<components::TransformFrameId>(
                Pinhole::descriptor_child_frame().component,
            );
            let image_plane_distance = query_results
                .get_mono_with_fallback::<components::ImagePlaneDistance>(
                    Pinhole::descriptor_image_plane_distance().component,
                );
            let color = query_results
                .get_mono_with_fallback::<components::Color>(Pinhole::descriptor_color().component)
                .into();
            let line_width = process_radius(
                &data_result.entity_path,
                query_results.get_mono_with_fallback::<components::Radius>(
                    Pinhole::descriptor_line_width().component,
                ),
            );

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

            if let Err(err) = self.visit_instance(
                &ctx.query_context(data_result, &query.latest_at_query()),
                &mut line_builder,
                transforms,
                &component_data,
                entity_highlight,
                view_kind,
            ) {
                output.report_error_for(data_result.entity_path.clone(), err);
            }
        }

        Ok(output.with_draw_data([(line_builder.into_draw_data()?.into())]))
    }

    fn data(&self) -> Option<&dyn std::any::Any> {
        Some(self.data.as_any())
    }
}
