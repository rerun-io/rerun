use glam::vec3;

use re_log_types::Instance;
use re_renderer::renderer::LineStripFlags;
use re_types::{
    Archetype as _,
    archetypes::Pinhole,
    components::{self},
};
use re_view::latest_at_with_blueprint_resolved_data;
use re_viewer_context::{
    DataResult, IdentifiedViewSystem, MaybeVisualizableEntities, ViewContext,
    ViewContextCollection, ViewOutlineMasks, ViewQuery, ViewSystemExecutionError,
    VisualizableEntities, VisualizableFilterContext, VisualizerExecutionOutput,
    VisualizerQueryInfo, VisualizerSystem,
};

use super::{SpatialViewVisualizerData, filter_visualizable_3d_entities};
use crate::{
    contexts::TransformTreeContext, pinhole_wrapper::PinholeWrapper, visualizers::process_radius,
};

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
    image_from_camera: glam::Mat3,
    resolution: glam::Vec2,
    color: egui::Color32,
    line_width: re_renderer::Size,
    camera_xyz: components::ViewCoordinates,
    image_plane_distance: f32,
}

impl CamerasVisualizer {
    fn visit_instance(
        &mut self,
        line_builder: &mut re_renderer::LineDrawableBuilder<'_>,
        transforms: &TransformTreeContext,
        data_result: &DataResult,
        pinhole_properties: &CameraComponentDataWithFallbacks,
        entity_highlight: &ViewOutlineMasks,
    ) -> Result<(), String> {
        // Check for valid resolution.
        let w = pinhole_properties.resolution.x;
        let h = pinhole_properties.resolution.y;
        let z = pinhole_properties.image_plane_distance;
        let color = pinhole_properties.color;
        if !w.is_finite() || !h.is_finite() || w <= 0.0 || h <= 0.0 {
            return Err("Invalid resolution".to_owned());
        }

        let instance = Instance::from(0);
        let ent_path = &data_result.entity_path;
        let frame_id = transforms.transform_frame_id_for(ent_path.hash());

        let pinhole = crate::Pinhole {
            image_from_camera: pinhole_properties.image_from_camera,
            resolution: pinhole_properties.resolution,
            color: Some(pinhole_properties.color),
            line_width: Some(pinhole_properties.line_width),
        };

        // If the camera is the target frame, there is nothing for us to display.
        if transforms.target_frame() == frame_id {
            self.pinhole_cameras.push(PinholeWrapper {
                ent_path: ent_path.clone(),
                pinhole_view_coordinates: pinhole_properties.camera_xyz,
                world_from_camera: macaw::IsoTransform::IDENTITY,
                pinhole,
                picture_plane_distance: pinhole_properties.image_plane_distance,
            });
            return Err("Can't visualize pinholes at the view's origin".to_owned());
        }

        let Some(pinhole_tree_root_info) = transforms.pinhole_tree_root_info(frame_id) else {
            return Err("No valid pinhole present".to_owned());
        };
        let world_from_camera = pinhole_tree_root_info
            .parent_root_from_pinhole_root
            .as_affine3a();

        // If this transform is not representable as an `IsoTransform` we can't display it yet.
        // This would happen if the camera is under another camera or under a transform with non-uniform scale.
        let Some(world_from_camera_iso) = macaw::IsoTransform::from_mat4(&world_from_camera.into())
        else {
            return Err("Can only visualize pinhole under isometric transforms".to_owned());
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
                .color(color)
                .flags(flags)
                .picking_instance_id(instance_layer_id.instance);

            if let Some(outline_mask_ids) = entity_highlight.instances.get(&instance) {
                lines.outline_mask_ids(*outline_mask_ids);
            }
        }

        // world_from_camera is the transform to the pinhole origin
        self.data.add_bounding_box_from_points(
            ent_path.hash(),
            std::iter::once(glam::Vec3::ZERO),
            world_from_camera,
        );

        Ok(())
    }
}

impl VisualizerSystem for CamerasVisualizer {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<Pinhole>()
    }

    fn filter_visualizable_entities(
        &self,
        entities: MaybeVisualizableEntities,
        context: &dyn VisualizableFilterContext,
    ) -> VisualizableEntities {
        re_tracing::profile_function!();
        filter_visualizable_3d_entities(entities, context)
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        query: &ViewQuery<'_>,
        context_systems: &ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        let mut output = VisualizerExecutionOutput::default();

        let transforms = context_systems.get::<TransformTreeContext>()?;

        // Counting all cameras ahead of time is a bit wasteful, but we also don't expect a huge amount,
        // so let re_renderer's allocator internally decide what buffer sizes to pick & grow them as we go.
        let mut line_builder = re_renderer::LineDrawableBuilder::new(ctx.viewer_ctx.render_ctx());
        line_builder.radius_boost_in_ui_points_for_outlines(
            re_view::SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES,
        );

        for data_result in query.iter_visible_data_results(Self::identifier()) {
            let time_query = re_chunk_store::LatestAtQuery::new(query.timeline, query.latest_at);

            let query_shadowed_components = false;
            let query_results = latest_at_with_blueprint_resolved_data(
                ctx,
                None,
                &time_query,
                data_result,
                Pinhole::all_component_identifiers(),
                query_shadowed_components,
            );

            let Some(pinhole_projection) = query_results
                .get_required_mono::<components::PinholeProjection>(
                    Pinhole::descriptor_image_from_camera().component,
                )
            else {
                continue;
            };

            let resolution = query_results.get_mono_with_fallback::<components::Resolution>(
                Pinhole::descriptor_resolution().component,
            );
            let camera_xyz = query_results.get_mono_with_fallback::<components::ViewCoordinates>(
                Pinhole::descriptor_camera_xyz().component,
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
                image_from_camera: pinhole_projection.0.into(),
                resolution: resolution.into(),
                color,
                line_width,
                camera_xyz,
                image_plane_distance: image_plane_distance.into(),
            };

            let entity_highlight = query
                .highlights
                .entity_outline_mask(data_result.entity_path.hash());

            if let Err(err) = self.visit_instance(
                &mut line_builder,
                transforms,
                data_result,
                &component_data,
                entity_highlight,
            ) {
                output.report_error_for(data_result.entity_path.clone(), err);
            }
        }

        Ok(output.with_draw_data([(line_builder.into_draw_data()?.into())]))
    }

    fn data(&self) -> Option<&dyn std::any::Any> {
        Some(self.data.as_any())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
