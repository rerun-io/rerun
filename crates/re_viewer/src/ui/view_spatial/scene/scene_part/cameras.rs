use re_data_store::{EntityPath, EntityProperties};
use re_log_types::{
    component_types::{InstanceKey, Pinhole},
    coordinates::{Handedness, SignedAxis3},
    ViewCoordinates,
};
use re_renderer::renderer::LineStripFlags;
use re_viewer_context::TimeControl;
use re_viewer_context::{SceneQuery, ViewerContext};

use crate::{
    misc::{
        instance_hash_conversions::picking_layer_id_from_instance_path_hash, SpaceViewHighlights,
        SpaceViewOutlineMasks, TransformCache,
    },
    ui::view_spatial::{scene::EntityDepthOffsets, SceneSpatial, SpaceCamera3D},
};

use super::{instance_path_hash_for_picking, ScenePart};

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

pub struct CamerasPart;

impl CamerasPart {
    #[allow(clippy::too_many_arguments)]
    fn visit_instance(
        scene: &mut SceneSpatial,
        ent_path: &EntityPath,
        instance_key: InstanceKey,
        props: &EntityProperties,
        transforms: &TransformCache,
        pinhole: Pinhole,
        view_coordinates: ViewCoordinates,
        entity_highlight: &SpaceViewOutlineMasks,
    ) {
        // The transform *at* this entity path already has the pinhole transformation we got passed in!
        // This makes sense, since if there's an image logged here one would expect that the transform applies.
        // We're however first interested in the rigid transform that led here, so query the parent transform.
        //
        // Note that currently a transform on an object can't have both a pinhole AND a rigid transform,
        // which makes this rather well defined here.
        let parent_path = ent_path
            .parent()
            .expect("root path can't be part of scene query");
        let Some(world_from_parent) = transforms.reference_from_entity(&parent_path) else {
                return;
            };

        let frustum_length = *props.pinhole_image_plane_distance.get();

        // If the camera is our reference, there is nothing for us to display.
        if transforms.reference_path() == ent_path {
            scene.space_cameras.push(SpaceCamera3D {
                ent_path: ent_path.clone(),
                view_coordinates,
                world_from_camera: macaw::IsoTransform::IDENTITY,
                pinhole: Some(pinhole),
                picture_plane_distance: frustum_length,
            });
            return;
        }

        // If this transform is not representable an iso transform transform we can't display it yet.
        // This would happen if the camera is under another camera or under a transform with non-uniform scale.
        let Some(world_from_camera) = macaw::IsoTransform::from_mat4(&world_from_parent.into()) else {
            return;
        };

        scene.space_cameras.push(SpaceCamera3D {
            ent_path: ent_path.clone(),
            view_coordinates,
            world_from_camera,
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
        let color = SceneSpatial::CAMERA_COLOR;
        let num_instances = 1; // There is only ever one instance of `Transform` per entity.
        let instance_path_for_picking = instance_path_hash_for_picking(
            ent_path,
            instance_key,
            num_instances,
            entity_highlight.any_selection_highlight,
        );
        let instance_layer_id = picking_layer_id_from_instance_path_hash(instance_path_for_picking);

        let mut batch = scene
            .primitives
            .line_strips
            .batch("camera frustum")
            .world_from_obj(world_from_parent)
            .outline_mask_ids(entity_highlight.overall)
            .picking_object_id(instance_layer_id.object);
        let lines = batch
            .add_segments(segments.into_iter())
            .radius(radius)
            .color(color)
            .flags(LineStripFlags::FLAG_CAP_END_ROUND | LineStripFlags::FLAG_CAP_START_ROUND)
            .picking_instance_id(instance_layer_id.instance);

        if let Some(outline_mask_ids) = entity_highlight.instances.get(&instance_key) {
            lines.outline_mask_ids(*outline_mask_ids);
        }
    }
}

impl ScenePart for CamerasPart {
    fn load(
        &self,
        scene: &mut SceneSpatial,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        transforms: &TransformCache,
        highlights: &SpaceViewHighlights,
        _depth_offsets: &EntityDepthOffsets,
    ) {
        crate::profile_scope!("CamerasPart");

        let store = &ctx.log_db.entity_db.data_store;
        for (ent_path, props) in query.iter_entities() {
            let query = re_arrow_store::LatestAtQuery::new(query.timeline, query.latest_at);

            if let Some(pinhole) = store.query_latest_component::<Pinhole>(ent_path, &query) {
                let view_coordinates = determine_view_coordinates(
                    &ctx.log_db.entity_db.data_store,
                    &ctx.rec_cfg.time_ctrl,
                    ent_path.clone(),
                );
                let entity_highlight = highlights.entity_outline_mask(ent_path.hash());

                Self::visit_instance(
                    scene,
                    ent_path,
                    InstanceKey(0),
                    &props,
                    transforms,
                    pinhole,
                    view_coordinates,
                    entity_highlight,
                );
            }
        }
    }
}
