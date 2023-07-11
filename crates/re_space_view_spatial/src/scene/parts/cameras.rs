use glam::vec3;

use re_components::Pinhole;
use re_data_store::{EntityPath, EntityProperties};
use re_log_types::{Component as _, InstanceKey};
use re_renderer::renderer::LineStripFlags;
use re_viewer_context::{
    ArchetypeDefinition, ScenePart, SceneQuery, SpaceViewHighlights, SpaceViewOutlineMasks,
    ViewerContext,
};

use crate::{
    instance_hash_conversions::picking_layer_id_from_instance_path_hash,
    scene::contexts::{pinhole_camera_view_coordinates, SpatialSceneContext},
    space_camera_3d::SpaceCamera3D,
    SpatialSpaceView,
};

use super::{SpatialScenePartData, SpatialSpaceViewState};

const CAMERA_COLOR: re_renderer::Color32 = re_renderer::Color32::from_rgb(150, 150, 150);

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
        store: &re_arrow_store::DataStore,
        query: &re_arrow_store::LatestAtQuery,
        entity_highlight: &SpaceViewOutlineMasks,
    ) {
        let instance_key = InstanceKey(0);

        // Need to ignore the image plane dependent derived transform we generate for pinhole cameras,
        // otherwise we'd put the frustum lines in front of the camera.
        let Some(world_from_camera) = scene_context.transforms.reference_from_entity_ignore_image_plane_transform(ent_path, store, query) else {
            return;
        };

        let frustum_length = *props.pinhole_image_plane_distance.get();

        let pinhole_view_coordinates = pinhole_camera_view_coordinates(store, query, ent_path);

        // If the camera is our reference, there is nothing for us to display.
        if scene_context.transforms.reference_path() == ent_path {
            self.space_cameras.push(SpaceCamera3D {
                ent_path: ent_path.clone(),
                pinhole_view_coordinates,
                world_from_camera: macaw::IsoTransform::IDENTITY,
                pinhole: Some(pinhole),
                picture_plane_distance: frustum_length,
            });
            return;
        }

        // If this transform is not representable an iso transform we can't use it as a space camera yet.
        // This would happen if the camera is under another camera or under a transform with non-uniform scale.
        let Some(world_from_camera_iso) = macaw::IsoTransform::from_mat4(&world_from_camera.into()) else {
            return;
        };

        debug_assert!(world_from_camera_iso.is_finite());

        self.space_cameras.push(SpaceCamera3D {
            ent_path: ent_path.clone(),
            pinhole_view_coordinates,
            world_from_camera: world_from_camera_iso,
            pinhole: Some(pinhole),
            picture_plane_distance: frustum_length,
        });

        let Some(resolution) = pinhole.resolution else {
            return;
        };

        // Setup a RDF frustum (for non-RDF we apply a transformation matrix later).
        let w = resolution.x();
        let h = resolution.y();
        let z = frustum_length;

        let corners = [
            pinhole.unproject(vec3(0.0, 0.0, z)),
            pinhole.unproject(vec3(0.0, h, z)),
            pinhole.unproject(vec3(w, 0.0, z)),
            pinhole.unproject(vec3(w, h, z)),
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
            re_data_store::InstancePathHash::instance(ent_path, instance_key);
        let instance_layer_id = picking_layer_id_from_instance_path_hash(instance_path_for_picking);

        let mut line_builder = scene_context.shared_render_builders.lines();
        let mut batch = line_builder
            .batch("camera frustum")
            // The frustum is setup as a RDF frustum, but if the view coordinates are not RDF,
            // we need to reorient the displayed frustum so that we indicate the correct orientation in the 3D world space.
            .world_from_obj(
                world_from_camera * glam::Affine3A::from_mat3(pinhole_view_coordinates.from_rdf()),
            )
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
        let latest_at_query = re_arrow_store::LatestAtQuery::new(query.timeline, query.latest_at);
        for (ent_path, props) in query.iter_entities() {
            if let Some(pinhole) =
                store.query_latest_component::<Pinhole>(ent_path, &latest_at_query)
            {
                let entity_highlight = highlights.entity_outline_mask(ent_path.hash());

                self.visit_instance(
                    scene_context,
                    ent_path,
                    &props,
                    pinhole,
                    store,
                    &latest_at_query,
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
