use re_components::{
    coordinates::{Handedness, SignedAxis3},
    Component, InstanceKey, Pinhole, Transform3D, ViewCoordinates,
};
use re_data_store::{EntityPath, EntityProperties};
use re_renderer::renderer::LineStripFlags;
use re_viewer_context::{ArchetypeDefinition, ScenePart, TimeControl};
use re_viewer_context::{SceneQuery, ViewerContext};
use re_viewer_context::{SpaceViewHighlights, SpaceViewOutlineMasks};

use crate::{
    instance_hash_conversions::picking_layer_id_from_instance_path_hash,
    scene::contexts::SpatialSceneContext, space_camera_3d::SpaceCamera3D, SpatialSpaceView,
};

use super::{SpatialScenePartData, SpatialSpaceViewState};

const CAMERA_COLOR: re_renderer::Color32 = re_renderer::Color32::from_rgb(150, 150, 150);

/// Determine the view coordinates (i.e.) the axis semantics.
///
/// The recommended way to log this is on the object holding the extrinsic camera properties
/// (i.e. the last rigid transform from here)
/// But for ease of use allow it everywhere along the path.
///
/// TODO(andreas): Doing a search upwards here isn't great. Maybe this can be part of the transform cache or similar?
fn determine_view_coordinates(
    store: &re_arrow_store::DataStore,
    time_ctrl: &TimeControl,
    mut entity_path: EntityPath,
) -> ViewCoordinates {
    loop {
        if let Some(view_coordinates) =
            store.query_latest_component(&entity_path, &time_ctrl.current_query())
        {
            return view_coordinates;
        }

        if let Some(parent) = entity_path.parent() {
            entity_path = parent;
        } else {
            // Keep in mind, there is no universal convention for any of this!
            // https://twitter.com/freyaholmer/status/1325556229410861056
            return ViewCoordinates::from_up_and_handedness(
                SignedAxis3::POSITIVE_Y,
                Handedness::Right,
            );
        }
    }
}

#[derive(Default)]
pub struct CamerasPart {
    pub data: SpatialScenePartData,
    pub space_cameras: Vec<SpaceCamera3D>,
}

impl CamerasPart {
    #[allow(clippy::too_many_arguments)]
    fn visit_instance(
        &mut self,
        scene_context: &SpatialSceneContext,
        ent_path: &EntityPath,
        props: &EntityProperties,
        pinhole: Pinhole,
        transform_at_entity: Option<Transform3D>,
        view_coordinates: ViewCoordinates,
        entity_highlight: &SpaceViewOutlineMasks,
    ) {
        let instance_key = InstanceKey(0);

        // The transform *at* this entity path already has the pinhole transformation we got passed in!
        // This makes sense, since if there's an image logged here one would expect that the transform applies.
        // We're however first interested in the rigid transform that led here, so query the parent transform.
        let parent_path = ent_path
            .parent()
            .expect("root path can't be part of scene query");
        let Some(mut world_from_camera) = scene_context.transforms.reference_from_entity(&parent_path) else {
                return;
            };

        let frustum_length = *props.pinhole_image_plane_distance.get();

        // If the camera is our reference, there is nothing for us to display.
        if scene_context.transforms.reference_path() == ent_path {
            self.space_cameras.push(SpaceCamera3D {
                ent_path: ent_path.clone(),
                view_coordinates,
                world_from_camera: macaw::IsoTransform::IDENTITY,
                pinhole: Some(pinhole),
                picture_plane_distance: frustum_length,
            });
            return;
        }

        // There's one wrinkle with using the parent transform though:
        // The entity itself may have a 3D transform which (by convention!) we apply *before* the pinhole camera.
        // Let's add that if it exists.
        if let Some(transform_at_entity) = transform_at_entity {
            world_from_camera =
                world_from_camera * transform_at_entity.to_parent_from_child_transform();
        }

        // If this transform is not representable an iso transform transform we can't display it yet.
        // This would happen if the camera is under another camera or under a transform with non-uniform scale.
        let Some(world_from_camera_iso) = macaw::IsoTransform::from_mat4(&world_from_camera.into()) else {
            return;
        };

        self.space_cameras.push(SpaceCamera3D {
            ent_path: ent_path.clone(),
            view_coordinates,
            world_from_camera: world_from_camera_iso,
            pinhole: Some(pinhole),
            picture_plane_distance: frustum_length,
        });

        // TODO(andreas): FOV fallback doesn't make much sense. What does pinhole without fov mean?
        let fov_y = pinhole.fov_y().unwrap_or(std::f32::consts::FRAC_PI_2);
        let fy = (fov_y * 0.5).tan() * frustum_length;
        let fx = fy * pinhole.aspect_ratio().unwrap_or(1.0);

        let image_center_pixel = pinhole.resolution().unwrap_or(glam::Vec2::ZERO) * 0.5;
        let principal_point_offset_pixel = image_center_pixel - pinhole.principal_point();
        let principal_point_offset =
            principal_point_offset_pixel / pinhole.resolution().unwrap_or(glam::Vec2::ONE);
        // Don't multiply with (fx,fy) because that would multiply the aspect ratio twice!
        // Times two since fy is the half screen size (extending from -fy to fy!).
        let offset = principal_point_offset * (fy * 2.0);

        let corners = [
            (offset + glam::vec2(fx, -fy)).extend(frustum_length),
            (offset + glam::vec2(fx, fy)).extend(frustum_length),
            (offset + glam::vec2(-fx, fy)).extend(frustum_length),
            (offset + glam::vec2(-fx, -fy)).extend(frustum_length),
        ];
        let triangle_frustum_offset = fy * 1.05;
        let up_triangle = [
            // Use only fx for with and height of the triangle, so that the aspect ratio of the triangle is always the same.
            (offset + glam::vec2(-fx * 0.25, -triangle_frustum_offset)).extend(frustum_length),
            (offset + glam::vec2(0.0, -fx * 0.25 - triangle_frustum_offset)).extend(frustum_length),
            (offset + glam::vec2(fx * 0.25, -triangle_frustum_offset)).extend(frustum_length),
        ];

        let segments = [
            // Frustum corners
            (glam::Vec3::ZERO, corners[0]),
            (glam::Vec3::ZERO, corners[1]),
            (glam::Vec3::ZERO, corners[2]),
            (glam::Vec3::ZERO, corners[3]),
            // rectangle around "far plane"
            (corners[0], corners[1]),
            (corners[1], corners[2]),
            (corners[2], corners[3]),
            (corners[3], corners[0]),
            // triangle indicating up direction
            (up_triangle[0], up_triangle[1]),
            (up_triangle[1], up_triangle[2]),
            (up_triangle[2], up_triangle[0]),
        ];

        let radius = re_renderer::Size::new_points(1.0);
        let instance_path_for_picking =
            re_data_store::InstancePathHash::instance(ent_path, instance_key);
        let instance_layer_id = picking_layer_id_from_instance_path_hash(instance_path_for_picking);

        let mut line_builder = scene_context.shared_render_builders.lines();
        let mut batch = line_builder
            .batch("camera frustum")
            .world_from_obj(world_from_camera)
            .outline_mask_ids(entity_highlight.overall)
            .picking_object_id(instance_layer_id.object);
        let lines = batch
            .add_segments(segments.into_iter())
            .radius(radius)
            .color(CAMERA_COLOR)
            .flags(LineStripFlags::FLAG_CAP_END_ROUND | LineStripFlags::FLAG_CAP_START_ROUND)
            .picking_instance_id(instance_layer_id.instance);

        if let Some(outline_mask_ids) = entity_highlight.instances.get(&instance_key) {
            lines.outline_mask_ids(*outline_mask_ids);
        }

        scene_context
            .num_3d_primitives
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}

impl ScenePart<SpatialSpaceView> for CamerasPart {
    fn archetype(&self) -> ArchetypeDefinition {
        vec1::vec1![Pinhole::name(),]
    }

    fn populate(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        _space_view_state: &SpatialSpaceViewState,
        scene_context: &SpatialSceneContext,
        highlights: &SpaceViewHighlights,
    ) -> Vec<re_renderer::QueueableDrawData> {
        re_tracing::profile_scope!("CamerasPart");

        let store = &ctx.store_db.entity_db.data_store;
        for (ent_path, props) in query.iter_entities() {
            let query = re_arrow_store::LatestAtQuery::new(query.timeline, query.latest_at);

            if let Some(pinhole) = store.query_latest_component::<Pinhole>(ent_path, &query) {
                let view_coordinates = determine_view_coordinates(
                    &ctx.store_db.entity_db.data_store,
                    &ctx.rec_cfg.time_ctrl,
                    ent_path.clone(),
                );
                let entity_highlight = highlights.entity_outline_mask(ent_path.hash());

                self.visit_instance(
                    scene_context,
                    ent_path,
                    &props,
                    pinhole,
                    store.query_latest_component::<Transform3D>(ent_path, &query),
                    view_coordinates,
                    entity_highlight,
                );
            }
        }

        Vec::new()
    }

    fn data(&self) -> Option<&SpatialScenePartData> {
        Some(&self.data)
    }
}
