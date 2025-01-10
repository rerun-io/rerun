use glam::vec3;
use re_log_types::Instance;
use re_renderer::renderer::LineStripFlags;
use re_types::{
    archetypes::Pinhole,
    components::{ImagePlaneDistance, ViewCoordinates},
};
use re_viewer_context::{
    ApplicableEntities, DataResult, IdentifiedViewSystem, QueryContext,
    TypedComponentFallbackProvider, ViewContext, ViewContextCollection, ViewOutlineMasks,
    ViewQuery, ViewStateExt as _, ViewSystemExecutionError, VisualizableEntities,
    VisualizableFilterContext, VisualizerQueryInfo, VisualizerSystem,
};

use super::{filter_visualizable_3d_entities, SpatialViewVisualizerData};
use crate::{
    contexts::TransformContext, query_pinhole, space_camera_3d::SpaceCamera3D, ui::SpatialViewState,
};

const CAMERA_COLOR: re_renderer::Color32 = re_renderer::Color32::from_rgb(150, 150, 150);

pub struct CamerasVisualizer {
    pub data: SpatialViewVisualizerData,
    pub space_cameras: Vec<SpaceCamera3D>,
}

impl Default for CamerasVisualizer {
    fn default() -> Self {
        Self {
            // Cameras themselves aren't inherently 2D or 3D since they represent intrinsics.
            // (extrinsics, represented by [`transform3d_arrow::Transform3DArrowsPart`] are 3D though)
            data: (SpatialViewVisualizerData::new(None)),
            space_cameras: Vec::new(),
        }
    }
}

impl IdentifiedViewSystem for CamerasVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Cameras".into()
    }
}

impl CamerasVisualizer {
    #[allow(clippy::too_many_arguments)]
    fn visit_instance(
        &mut self,
        line_builder: &mut re_renderer::LineDrawableBuilder<'_>,
        transforms: &TransformContext,
        data_result: &DataResult,
        pinhole: &Pinhole,
        pinhole_view_coordinates: ViewCoordinates,
        entity_highlight: &ViewOutlineMasks,
    ) {
        let instance = Instance::from(0);
        let ent_path = &data_result.entity_path;

        // Assuming the fallback provider did the right thing, this value should always be set.
        let frustum_length = pinhole.image_plane_distance.unwrap_or_default().into();

        // If the camera is our reference, there is nothing for us to display.
        if transforms.reference_path() == ent_path {
            self.space_cameras.push(SpaceCamera3D {
                ent_path: ent_path.clone(),
                pinhole_view_coordinates,
                world_from_camera: re_math::IsoTransform::IDENTITY,
                pinhole: Some(pinhole.clone()),
                picture_plane_distance: frustum_length,
            });
            return;
        }

        // The camera transform does not include the pinhole transform.
        let Some(transform_info) = transforms.transform_info_for_entity(ent_path.hash()) else {
            return;
        };
        let Some(twod_in_threed_info) = &transform_info.twod_in_threed_info else {
            // This implies that the transform context didn't see the pinhole transform.
            // Should be impossible!
            re_log::error_once!("Transform context didn't register the pinhole transform, but `CamerasVisualizer` is trying to display it!");
            return;
        };
        if &twod_in_threed_info.parent_pinhole != ent_path {
            // This implies that the camera is under another camera.
            // This should be reported already as an error at this point.
            return;
        }
        let world_from_camera = twod_in_threed_info.reference_from_pinhole_entity;

        // If this transform is not representable as an `IsoTransform` we can't display it yet.
        // This would happen if the camera is under another camera or under a transform with non-uniform scale.
        let Some(world_from_camera_iso) =
            re_math::IsoTransform::from_mat4(&world_from_camera.into())
        else {
            return;
        };

        debug_assert!(world_from_camera_iso.is_finite());

        self.space_cameras.push(SpaceCamera3D {
            ent_path: ent_path.clone(),
            pinhole_view_coordinates,
            world_from_camera: world_from_camera_iso,
            pinhole: Some(pinhole.clone()),
            picture_plane_distance: frustum_length,
        });

        let Some(resolution) = pinhole.resolution.as_ref() else {
            return;
        };

        // Setup a RDF frustum (for non-RDF we apply a transformation matrix later).
        let w = resolution.x();
        let h = resolution.y();
        let z = frustum_length;

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

        let radius = re_renderer::Size::new_ui_points(1.0);
        let instance_path_for_picking =
            re_entity_db::InstancePathHash::instance(ent_path, instance);
        let instance_layer_id =
            re_view::picking_layer_id_from_instance_path_hash(instance_path_for_picking);

        let mut batch = line_builder
            .batch(ent_path.to_string())
            // The frustum is setup as a RDF frustum, but if the view coordinates are not RDF,
            // we need to reorient the displayed frustum so that we indicate the correct orientation in the 3D world space.
            .world_from_obj(
                world_from_camera * glam::Affine3A::from_mat3(pinhole_view_coordinates.from_rdf()),
            )
            .outline_mask_ids(entity_highlight.overall)
            .picking_object_id(instance_layer_id.object);

        for (strip, flags) in strips {
            let lines = batch
                .add_strip(strip.into_iter())
                .radius(radius)
                .color(CAMERA_COLOR)
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
    }
}

impl VisualizerSystem for CamerasVisualizer {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<Pinhole>()
    }

    fn filter_visualizable_entities(
        &self,
        entities: ApplicableEntities,
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
    ) -> Result<Vec<re_renderer::QueueableDrawData>, ViewSystemExecutionError> {
        let transforms = context_systems.get::<TransformContext>()?;

        // Counting all cameras ahead of time is a bit wasteful, but we also don't expect a huge amount,
        // so let re_renderer's allocator internally decide what buffer sizes to pick & grow them as we go.
        let mut line_builder = re_renderer::LineDrawableBuilder::new(ctx.viewer_ctx.render_ctx);
        line_builder.radius_boost_in_ui_points_for_outlines(
            re_view::SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES,
        );

        for data_result in query.iter_visible_data_results(ctx, Self::identifier()) {
            let time_query = re_chunk_store::LatestAtQuery::new(query.timeline, query.latest_at);

            if let Some(pinhole) = query_pinhole(ctx, &time_query, data_result) {
                let entity_highlight = query
                    .highlights
                    .entity_outline_mask(data_result.entity_path.hash());

                self.visit_instance(
                    &mut line_builder,
                    transforms,
                    data_result,
                    &pinhole,
                    pinhole.camera_xyz.unwrap_or(ViewCoordinates::RDF), // TODO(#2641): This should come from archetype
                    entity_highlight,
                );
            }
        }

        Ok(vec![(line_builder.into_draw_data()?.into())])
    }

    fn data(&self) -> Option<&dyn std::any::Any> {
        Some(self.data.as_any())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn fallback_provider(&self) -> &dyn re_viewer_context::ComponentFallbackProvider {
        self
    }
}

impl TypedComponentFallbackProvider<ImagePlaneDistance> for CamerasVisualizer {
    fn fallback_for(&self, ctx: &QueryContext<'_>) -> ImagePlaneDistance {
        let Ok(state) = ctx.view_state.downcast_ref::<SpatialViewState>() else {
            return Default::default();
        };

        let scene_size = state.bounding_boxes.smoothed.size().length();

        if scene_size.is_finite() && scene_size > 0.0 {
            // Works pretty well for `examples/python/open_photogrammetry_format/open_photogrammetry_format.py --no-frames`
            scene_size * 0.02
        } else {
            // This value somewhat arbitrary. In almost all cases where the scene has defined bounds
            // the heuristic will change it or it will be user edited. In the case of non-defined bounds
            // this value works better with the default camera setup.
            0.3
        }
        .into()
    }
}

re_viewer_context::impl_component_fallback_provider!(CamerasVisualizer => [ImagePlaneDistance]);
