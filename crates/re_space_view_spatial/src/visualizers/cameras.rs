use glam::vec3;
use re_log_types::Instance;
use re_renderer::renderer::LineStripFlags;
use re_types::{
    archetypes::Pinhole,
    components::{ImagePlaneDistance, Transform3D, ViewCoordinates},
};
use re_viewer_context::{
    ApplicableEntities, DataResult, IdentifiedViewSystem, QueryContext, SpaceViewOutlineMasks,
    SpaceViewStateExt as _, SpaceViewSystemExecutionError, TypedComponentFallbackProvider,
    ViewContext, ViewContextCollection, ViewQuery, VisualizableEntities, VisualizableFilterContext,
    VisualizerQueryInfo, VisualizerSystem,
};

use super::{filter_visualizable_3d_entities, SpatialViewVisualizerData};
use crate::{
    contexts::TransformContext,
    instance_hash_conversions::picking_layer_id_from_instance_path_hash, query_pinhole,
    space_camera_3d::SpaceCamera3D, ui::SpatialSpaceViewState,
    visualizers::SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES,
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
        transform_at_entity: Option<Transform3D>,
        pinhole_view_coordinates: ViewCoordinates,
        entity_highlight: &SpaceViewOutlineMasks,
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
                world_from_camera: macaw::IsoTransform::IDENTITY,
                pinhole: Some(pinhole.clone()),
                picture_plane_distance: frustum_length,
            });
            return;
        }

        // We need special handling to find the 3D transform for drawing the
        // frustum itself. The transform that would otherwise be in the
        // transform context might include both a rigid transform and a pinhole. This
        // makes sense, since if there's an image logged here one would expect
        // both the rigid and the pinhole to apply, but here we're only interested
        // in the rigid transform at this entity path, excluding the pinhole
        // portion (we handle the pinhole separately later).
        let world_from_camera_rigid = {
            // Start with the transform to the entity parent, if it exists
            let world_from_parent = ent_path
                .parent()
                .and_then(|parent_path| transforms.reference_from_entity(&parent_path))
                .unwrap_or(macaw::Affine3A::IDENTITY);

            // Then combine it with the transform at the entity itself, if there is one.
            if let Some(transform_at_entity) = transform_at_entity {
                world_from_parent * transform_at_entity.into_parent_from_child_transform()
            } else {
                world_from_parent
            }
        };

        // If this transform is not representable as an `IsoTransform` we can't display it yet.
        // This would happen if the camera is under another camera or under a transform with non-uniform scale.
        let Some(world_from_camera_rigid_iso) =
            macaw::IsoTransform::from_mat4(&world_from_camera_rigid.into())
        else {
            return;
        };

        debug_assert!(world_from_camera_rigid_iso.is_finite());

        self.space_cameras.push(SpaceCamera3D {
            ent_path: ent_path.clone(),
            pinhole_view_coordinates,
            world_from_camera: world_from_camera_rigid_iso,
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
            pinhole.unproject(vec3(0.6 * w, 0.0, z)),
            pinhole.unproject(vec3(0.5 * w, -0.1 * w, z)),
        ];

        let segments = [
            // Frustum corners:
            (glam::Vec3::ZERO, corners[0]),
            (glam::Vec3::ZERO, corners[1]),
            (glam::Vec3::ZERO, corners[2]),
            (glam::Vec3::ZERO, corners[3]),
            // Rectangle around "far plane":
            (corners[0], corners[1]),
            (corners[1], corners[2]),
            (corners[2], corners[3]),
            (corners[3], corners[0]),
            // Triangle indicating up direction:
            (up_triangle[0], up_triangle[1]),
            (up_triangle[1], up_triangle[2]),
            (up_triangle[2], up_triangle[0]),
        ];

        let radius = re_renderer::Size::new_points(1.0);
        let instance_path_for_picking =
            re_entity_db::InstancePathHash::instance(ent_path, instance);
        let instance_layer_id = picking_layer_id_from_instance_path_hash(instance_path_for_picking);

        let mut batch = line_builder
            .batch(ent_path.to_string())
            // The frustum is setup as a RDF frustum, but if the view coordinates are not RDF,
            // we need to reorient the displayed frustum so that we indicate the correct orientation in the 3D world space.
            .world_from_obj(
                world_from_camera_rigid
                    * glam::Affine3A::from_mat3(pinhole_view_coordinates.from_rdf()),
            )
            .outline_mask_ids(entity_highlight.overall)
            .picking_object_id(instance_layer_id.object);
        let lines = batch
            .add_segments(segments.into_iter())
            .radius(radius)
            .color(CAMERA_COLOR)
            .flags(LineStripFlags::FLAG_CAP_END_ROUND | LineStripFlags::FLAG_CAP_START_ROUND)
            .picking_instance_id(instance_layer_id.instance);

        if let Some(outline_mask_ids) = entity_highlight.instances.get(&instance) {
            lines.outline_mask_ids(*outline_mask_ids);
        }

        // world_from_camera is the transform to the pinhole origin
        self.data.add_bounding_box_from_points(
            ent_path.hash(),
            std::iter::once(glam::Vec3::ZERO),
            world_from_camera_rigid,
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
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        let Some(render_ctx) = ctx.viewer_ctx.render_ctx else {
            return Err(SpaceViewSystemExecutionError::NoRenderContextError);
        };

        let transforms = context_systems.get::<TransformContext>()?;

        // Counting all cameras ahead of time is a bit wasteful, but we also don't expect a huge amount,
        // so let re_renderer's allocator internally decide what buffer sizes to pick & grow them as we go.
        let mut line_builder = re_renderer::LineDrawableBuilder::new(render_ctx);
        line_builder.radius_boost_in_ui_points_for_outlines(SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES);

        for data_result in query.iter_visible_data_results(ctx, Self::identifier()) {
            let time_query = re_data_store::LatestAtQuery::new(query.timeline, query.latest_at);

            if let Some(pinhole) = query_pinhole(ctx, &time_query, data_result) {
                let entity_highlight = query
                    .highlights
                    .entity_outline_mask(data_result.entity_path.hash());

                self.visit_instance(
                    &mut line_builder,
                    transforms,
                    data_result,
                    &pinhole,
                    // TODO(#5607): what should happen if the promise is still pending?
                    ctx.recording()
                        .latest_at_component::<Transform3D>(&data_result.entity_path, &time_query)
                        .map(|c| c.value),
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

    fn as_fallback_provider(&self) -> &dyn re_viewer_context::ComponentFallbackProvider {
        self
    }
}

impl TypedComponentFallbackProvider<ImagePlaneDistance> for CamerasVisualizer {
    fn fallback_for(&self, ctx: &QueryContext<'_>) -> ImagePlaneDistance {
        let Ok(state) = ctx
            .view_ctx
            .view_state
            .downcast_ref::<SpatialSpaceViewState>()
        else {
            return Default::default();
        };

        let scene_size = state.bounding_boxes.accumulated.size().length();

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
