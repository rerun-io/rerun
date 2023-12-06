use glam::vec3;
use re_data_store::{EntityPath, EntityProperties};
use re_renderer::renderer::LineStripFlags;
use re_types::{
    archetypes::Pinhole,
    components::{InstanceKey, Transform3D, ViewCoordinates},
    Archetype as _, ComponentNameSet,
};
use re_viewer_context::{
    IdentifiedViewSystem, SpaceViewOutlineMasks, SpaceViewSystemExecutionError,
    ViewContextCollection, ViewPartSystem, ViewQuery, ViewerContext,
};

use super::SpatialViewPartData;
use crate::{
    contexts::{SharedRenderBuilders, TransformContext},
    instance_hash_conversions::picking_layer_id_from_instance_path_hash,
    query_pinhole,
    space_camera_3d::SpaceCamera3D,
};

const CAMERA_COLOR: re_renderer::Color32 = re_renderer::Color32::from_rgb(150, 150, 150);

pub struct CamerasPart {
    pub data: SpatialViewPartData,
    pub space_cameras: Vec<SpaceCamera3D>,
}

impl Default for CamerasPart {
    fn default() -> Self {
        Self {
            // Cameras themselves aren't inherently 2D or 3D since they represent intrinsics.
            // (extrinsics, represented by [`transform3d_arrow::Transform3DArrowsPart`] are 3D though)
            data: (SpatialViewPartData::new(None)),
            space_cameras: Vec::new(),
        }
    }
}

impl IdentifiedViewSystem for CamerasPart {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Cameras".into()
    }
}

impl CamerasPart {
    #[allow(clippy::too_many_arguments)]
    fn visit_instance(
        &mut self,
        transforms: &TransformContext,
        shared_render_builders: &SharedRenderBuilders,
        ent_path: &EntityPath,
        props: &EntityProperties,
        pinhole: &Pinhole,
        transform_at_entity: Option<Transform3D>,
        pinhole_view_coordinates: ViewCoordinates,
        entity_highlight: &SpaceViewOutlineMasks,
    ) {
        let instance_key = InstanceKey(0);

        // The transform *at* this entity path already has the pinhole transformation we got passed in!
        // This makes sense, since if there's an image logged here one would expect that the transform applies.
        // We're however first interested in the rigid transform that led here, so query the parent transform.
        let parent_path = ent_path
            .parent()
            .expect("root path can't be part of scene query");
        let Some(mut world_from_camera) = transforms.reference_from_entity(&parent_path) else {
            return;
        };

        let frustum_length = *props.pinhole_image_plane_distance;

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

        // There's one wrinkle with using the parent transform though:
        // The entity itself may have a 3D transform which (by convention!) we apply *before* the pinhole camera.
        // Let's add that if it exists.
        if let Some(transform_at_entity) = transform_at_entity.clone() {
            world_from_camera =
                world_from_camera * transform_at_entity.into_parent_from_child_transform();
        }

        // If this transform is not representable as an `IsoTransform` we can't display it yet.
        // This would happen if the camera is under another camera or under a transform with non-uniform scale.
        let Some(world_from_camera_iso) = macaw::IsoTransform::from_mat4(&world_from_camera.into())
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

        let mut line_builder = shared_render_builders.lines();
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

        self.data.extend_bounding_box_with_points(
            std::iter::once(glam::Vec3::ZERO),
            transform_at_entity
                .unwrap_or(Transform3D::IDENTITY)
                .into_parent_from_child_transform(),
        );
    }
}

impl ViewPartSystem for CamerasPart {
    fn required_components(&self) -> ComponentNameSet {
        re_types::archetypes::Pinhole::required_components()
            .iter()
            .map(ToOwned::to_owned)
            .collect()
    }

    fn indicator_components(&self) -> ComponentNameSet {
        std::iter::once(re_types::archetypes::Pinhole::indicator().name()).collect()
    }

    fn execute(
        &mut self,
        ctx: &ViewerContext<'_>,
        query: &ViewQuery<'_>,
        view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        let transforms = view_ctx.get::<TransformContext>()?;
        let shared_render_builders = view_ctx.get::<SharedRenderBuilders>()?;

        let store = ctx.store_db.store();

        for data_result in query.iter_visible_data_results(Self::identifier()) {
            let time_query = re_arrow_store::LatestAtQuery::new(query.timeline, query.latest_at);

            if let Some(pinhole) = query_pinhole(store, &time_query, &data_result.entity_path) {
                let entity_highlight = query
                    .highlights
                    .entity_outline_mask(data_result.entity_path.hash());

                self.visit_instance(
                    transforms,
                    shared_render_builders,
                    &data_result.entity_path,
                    &data_result.resolved_properties,
                    &pinhole,
                    store
                        .query_latest_component::<Transform3D>(
                            &data_result.entity_path,
                            &time_query,
                        )
                        .map(|c| c.value),
                    pinhole.camera_xyz.unwrap_or(ViewCoordinates::RDF), // TODO(#2641): This should come from archetype
                    entity_highlight,
                );
            }
        }

        Ok(Vec::new())
    }

    fn data(&self) -> Option<&dyn std::any::Any> {
        Some(self.data.as_any())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
