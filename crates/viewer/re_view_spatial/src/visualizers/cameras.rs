use glam::vec3;
use re_log_types::Instance;
use re_renderer::renderer::LineStripFlags;
use re_types::{
    archetypes::Pinhole,
    components::{self},
    Archetype as _,
};
use re_view::latest_at_with_blueprint_resolved_data;
use re_viewer_context::{
    DataResult, IdentifiedViewSystem, MaybeVisualizableEntities, QueryContext,
    TypedComponentFallbackProvider, ViewContext, ViewContextCollection, ViewOutlineMasks,
    ViewQuery, ViewStateExt as _, ViewSystemExecutionError, VisualizableEntities,
    VisualizableFilterContext, VisualizerQueryInfo, VisualizerSystem,
};

use super::{filter_visualizable_3d_entities, SpatialViewVisualizerData};
use crate::{
    contexts::TransformTreeContext, resolution_of_image_at, space_camera_3d::SpaceCamera3D,
    ui::SpatialViewState,
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

struct CameraComponentDataWithFallbacks {
    pinhole: crate::Pinhole,
    camera_xyz: components::ViewCoordinates,
    image_plane_distance: f32,
}

impl CamerasVisualizer {
    #[allow(clippy::too_many_arguments)]
    fn visit_instance(
        &mut self,
        line_builder: &mut re_renderer::LineDrawableBuilder<'_>,
        transforms: &TransformTreeContext,
        data_result: &DataResult,
        pinhole_properties: &CameraComponentDataWithFallbacks,
        entity_highlight: &ViewOutlineMasks,
    ) {
        // Check for valid resolution.
        let w = pinhole_properties.pinhole.resolution.x;
        let h = pinhole_properties.pinhole.resolution.y;
        let z = pinhole_properties.image_plane_distance;
        if !w.is_finite() || !h.is_finite() || w <= 0.0 || h <= 0.0 {
            return;
        }

        let instance = Instance::from(0);
        let ent_path = &data_result.entity_path;

        // Assuming the fallback provider did the right thing, this value should always be set.
        // If the camera is our reference, there is nothing for us to display.
        if transforms.reference_path() == ent_path {
            self.space_cameras.push(SpaceCamera3D {
                ent_path: ent_path.clone(),
                pinhole_view_coordinates: pinhole_properties.camera_xyz,
                world_from_camera: re_math::IsoTransform::IDENTITY,
                pinhole: Some(pinhole_properties.pinhole),
                picture_plane_distance: pinhole_properties.image_plane_distance,
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
            pinhole_view_coordinates: pinhole_properties.camera_xyz,
            world_from_camera: world_from_camera_iso,
            pinhole: Some(pinhole_properties.pinhole),
            picture_plane_distance: pinhole_properties.image_plane_distance,
        });

        // Setup a RDF frustum (for non-RDF we apply a transformation matrix later).
        let corners = [
            pinhole_properties.pinhole.unproject(vec3(0.0, 0.0, z)),
            pinhole_properties.pinhole.unproject(vec3(0.0, h, z)),
            pinhole_properties.pinhole.unproject(vec3(w, h, z)),
            pinhole_properties.pinhole.unproject(vec3(w, 0.0, z)),
        ];

        let up_triangle = [
            pinhole_properties.pinhole.unproject(vec3(0.4 * w, 0.0, z)),
            pinhole_properties
                .pinhole
                .unproject(vec3(0.5 * w, -0.1 * w, z)),
            pinhole_properties.pinhole.unproject(vec3(0.6 * w, 0.0, z)),
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
                world_from_camera
                    * glam::Affine3A::from_mat3(pinhole_properties.camera_xyz.from_rdf()),
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
    ) -> Result<Vec<re_renderer::QueueableDrawData>, ViewSystemExecutionError> {
        let transforms = context_systems.get::<TransformTreeContext>()?;

        // Counting all cameras ahead of time is a bit wasteful, but we also don't expect a huge amount,
        // so let re_renderer's allocator internally decide what buffer sizes to pick & grow them as we go.
        let mut line_builder = re_renderer::LineDrawableBuilder::new(ctx.viewer_ctx.render_ctx());
        line_builder.radius_boost_in_ui_points_for_outlines(
            re_view::SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES,
        );

        for data_result in query.iter_visible_data_results(Self::identifier()) {
            let latest_at = query.latest_at_query();
            let query_ctx = ctx.query_context(data_result, &latest_at);
            let time_query = re_chunk_store::LatestAtQuery::new(query.timeline, query.latest_at);

            let query_shadowed_components = false;
            let query_results = latest_at_with_blueprint_resolved_data(
                ctx,
                None,
                &time_query,
                data_result,
                Pinhole::all_components()
                    .iter()
                    .map(|desc| desc.component_name),
                query_shadowed_components,
            );

            let Some(pinhole_projection) =
                query_results.get_required_mono::<components::PinholeProjection>()
            else {
                continue;
            };

            let resolution = query_results
                .get_mono::<components::Resolution>()
                .unwrap_or_else(|| self.fallback_for(&query_ctx));
            let camera_xyz = query_results
                .get_mono::<components::ViewCoordinates>()
                .unwrap_or_else(|| self.fallback_for(&query_ctx));
            let image_plane_distance = query_results
                .get_mono::<components::ImagePlaneDistance>()
                .unwrap_or_else(|| self.fallback_for(&query_ctx));

            let component_data = CameraComponentDataWithFallbacks {
                pinhole: crate::Pinhole {
                    image_from_camera: pinhole_projection.0.into(),
                    resolution: resolution.into(),
                },
                camera_xyz,
                image_plane_distance: image_plane_distance.into(),
            };

            let entity_highlight = query
                .highlights
                .entity_outline_mask(data_result.entity_path.hash());

            self.visit_instance(
                &mut line_builder,
                transforms,
                data_result,
                &component_data,
                entity_highlight,
            );
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

impl TypedComponentFallbackProvider<components::ImagePlaneDistance> for CamerasVisualizer {
    fn fallback_for(&self, ctx: &QueryContext<'_>) -> components::ImagePlaneDistance {
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

impl TypedComponentFallbackProvider<components::ViewCoordinates> for CamerasVisualizer {
    fn fallback_for(&self, _ctx: &QueryContext<'_>) -> components::ViewCoordinates {
        Pinhole::DEFAULT_CAMERA_XYZ
    }
}

impl TypedComponentFallbackProvider<components::Resolution> for CamerasVisualizer {
    fn fallback_for(&self, ctx: &QueryContext<'_>) -> components::Resolution {
        // If the Pinhole has no resolution, use the resolution for the image logged at the same path.
        // See https://github.com/rerun-io/rerun/issues/3852
        resolution_of_image_at(ctx.viewer_ctx, ctx.query, ctx.target_entity_path)
            // Zero will be seen as invalid resolution by the visualizer, making it opt out of visualization.
            // TODO(andreas): We should display a warning about this somewhere.
            // Since it's not a required component, logging a warning about this might be too noisy.
            .unwrap_or(components::Resolution::from([0.0, 0.0]))
    }
}

re_viewer_context::impl_component_fallback_provider!(CamerasVisualizer => [
    components::ImagePlaneDistance,
    components::ViewCoordinates,
    components::Resolution
]);
