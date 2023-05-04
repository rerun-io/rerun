use egui::Color32;
use re_data_store::EntityPath;
use re_log_types::component_types::InstanceKey;
use re_renderer::{
    renderer::{DepthClouds, MeshInstance},
    LineStripSeriesBuilder, PointCloudBuilder,
};

use crate::misc::instance_hash_conversions::picking_layer_id_from_instance_path_hash;

use super::MeshSource;

/// Primitives sent off to `re_renderer`.
/// (Some meta information still relevant to ui setup as well)
///
/// TODO(andreas): Right now we're using `re_renderer` data structures for reading (bounding box).
///                 In the future, this will be more limited as we're going to gpu staging data as soon as possible
///                 which is very slow to read. See [#594](https://github.com/rerun-io/rerun/pull/594)
pub struct SceneSpatialPrimitives {
    /// Estimated bounding box of all data in scene coordinates. Accumulated.
    pub(super) bounding_box: macaw::BoundingBox,

    pub images: Vec<super::Image>,
    pub line_strips: LineStripSeriesBuilder,
    pub points: PointCloudBuilder,
    pub meshes: Vec<MeshSource>,
    pub depth_clouds: DepthClouds,

    pub any_outlines: bool,
}

const AXIS_COLOR_X: Color32 = Color32::from_rgb(255, 25, 25);
const AXIS_COLOR_Y: Color32 = Color32::from_rgb(0, 240, 0);
const AXIS_COLOR_Z: Color32 = Color32::from_rgb(80, 80, 255);

const SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES: f32 = 1.5;
const SIZE_BOOST_IN_POINTS_FOR_POINT_OUTLINES: f32 = 2.5;

impl SceneSpatialPrimitives {
    pub fn new(re_ctx: &mut re_renderer::RenderContext) -> Self {
        Self {
            bounding_box: macaw::BoundingBox::nothing(),
            images: Default::default(),
            line_strips: LineStripSeriesBuilder::new(re_ctx)
                .radius_boost_in_ui_points_for_outlines(SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES),
            points: PointCloudBuilder::new(re_ctx)
                .radius_boost_in_ui_points_for_outlines(SIZE_BOOST_IN_POINTS_FOR_POINT_OUTLINES),
            meshes: Default::default(),
            depth_clouds: DepthClouds {
                clouds: Default::default(),
                radius_boost_in_ui_points_for_outlines: SIZE_BOOST_IN_POINTS_FOR_POINT_OUTLINES,
            },
            any_outlines: false,
        }
    }

    /// bounding box covering the rendered scene
    pub fn bounding_box(&self) -> macaw::BoundingBox {
        self.bounding_box
    }

    /// Number of primitives. Rather arbitrary what counts as a primitive so use this only for heuristic purposes!
    pub fn num_primitives(&self) -> usize {
        let Self {
            bounding_box: _,
            images,
            line_strips,
            points,
            meshes,
            depth_clouds,
            any_outlines: _,
        } = &self;

        images.len()
            + line_strips.vertices.len()
            + points.vertices.len()
            + meshes.len()
            + depth_clouds.clouds.len()
    }

    pub fn recalculate_bounding_box(&mut self) {
        crate::profile_function!();

        let Self {
            bounding_box,
            images,
            line_strips,
            points,
            meshes,
            depth_clouds,
            any_outlines: _,
        } = self;

        *bounding_box = macaw::BoundingBox::nothing();

        for image in images {
            let rect = &image.textured_rect;
            bounding_box.extend(rect.top_left_corner_position);
            bounding_box.extend(rect.top_left_corner_position + rect.extent_u);
            bounding_box.extend(rect.top_left_corner_position + rect.extent_v);
            bounding_box.extend(rect.top_left_corner_position + rect.extent_v + rect.extent_u);
        }

        // We don't need a very accurate bounding box, so in order to save some time,
        // we calculate a per batch bounding box for lines and points.
        // TODO(andreas): We should keep these around to speed up picking!
        for (batch, vertex_iter) in points.iter_vertices_by_batch() {
            let batch_bb = macaw::BoundingBox::from_points(vertex_iter.map(|v| v.position));
            *bounding_box = bounding_box.union(batch_bb.transform_affine3(&batch.world_from_obj));
        }
        for (batch, vertex_iter) in line_strips.iter_vertices_by_batch() {
            let batch_bb = macaw::BoundingBox::from_points(vertex_iter.map(|v| v.position));
            *bounding_box = bounding_box.union(batch_bb.transform_affine3(&batch.world_from_obj));
        }

        for mesh in meshes {
            // TODO(jleibs): is this safe for meshes or should we be doing the equivalent of the above?
            *bounding_box =
                bounding_box.union(mesh.mesh.bbox().transform_affine3(&mesh.world_from_mesh));
        }

        for cloud in &depth_clouds.clouds {
            *bounding_box = bounding_box.union(cloud.bbox());
        }
    }

    pub fn mesh_instances(&self) -> Vec<MeshInstance> {
        crate::profile_function!();
        self.meshes
            .iter()
            .flat_map(|mesh| {
                let (scale, rotation, translation) =
                    mesh.world_from_mesh.to_scale_rotation_translation();
                // TODO(andreas): The renderer should make it easy to apply a transform to a bunch of meshes
                let base_transform =
                    glam::Affine3A::from_scale_rotation_translation(scale, rotation, translation);
                mesh.mesh
                    .mesh_instances
                    .iter()
                    .map(move |mesh_instance| MeshInstance {
                        gpu_mesh: mesh_instance.gpu_mesh.clone(),
                        world_from_mesh: base_transform * mesh_instance.world_from_mesh,
                        outline_mask_ids: mesh.outline_mask_ids,
                        picking_layer_id: picking_layer_id_from_instance_path_hash(
                            mesh.picking_instance_hash,
                        ),
                        ..Default::default()
                    })
            })
            .collect()
    }

    pub fn add_axis_lines(
        &mut self,
        transform: macaw::IsoTransform,
        ent_path: Option<&EntityPath>,
        axis_length: f32,
    ) {
        use re_renderer::renderer::LineStripFlags;

        // TODO(andreas): It would be nice if could display the semantics (left/right/up) as a tooltip on hover.
        let line_radius = re_renderer::Size::new_scene(axis_length * 0.05);
        let origin = transform.translation();

        let mut line_batch = self.line_strips.batch("origin axis").picking_object_id(
            re_renderer::PickingLayerObjectId(ent_path.map_or(0, |p| p.hash64())),
        );
        let picking_instance_id = re_renderer::PickingLayerInstanceId(InstanceKey::SPLAT.0);

        line_batch
            .add_segment(
                origin,
                origin + transform.transform_vector3(glam::Vec3::X) * axis_length,
            )
            .radius(line_radius)
            .color(AXIS_COLOR_X)
            .flags(
                LineStripFlags::FLAG_COLOR_GRADIENT
                    | LineStripFlags::FLAG_CAP_END_TRIANGLE
                    | LineStripFlags::FLAG_CAP_START_ROUND,
            )
            .picking_instance_id(picking_instance_id);
        line_batch
            .add_segment(
                origin,
                origin + transform.transform_vector3(glam::Vec3::Y) * axis_length,
            )
            .radius(line_radius)
            .color(AXIS_COLOR_Y)
            .flags(
                LineStripFlags::FLAG_COLOR_GRADIENT
                    | LineStripFlags::FLAG_CAP_END_TRIANGLE
                    | LineStripFlags::FLAG_CAP_START_ROUND,
            )
            .picking_instance_id(picking_instance_id);
        line_batch
            .add_segment(
                origin,
                origin + transform.transform_vector3(glam::Vec3::Z) * axis_length,
            )
            .radius(line_radius)
            .color(AXIS_COLOR_Z)
            .flags(
                LineStripFlags::FLAG_COLOR_GRADIENT
                    | LineStripFlags::FLAG_CAP_END_TRIANGLE
                    | LineStripFlags::FLAG_CAP_START_ROUND,
            )
            .picking_instance_id(picking_instance_id);
    }
}
