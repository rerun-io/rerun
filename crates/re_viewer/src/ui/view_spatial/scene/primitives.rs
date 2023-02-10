use egui::{Color32, NumExt};
use macaw::BoundingBox;
use re_data_store::InstancePathHash;
use re_renderer::{renderer::MeshInstance, LineStripSeriesBuilder, PointCloudBuilder};

use crate::ui::view_spatial::SpatialNavigationMode;

use super::MeshSource;

/// Primitives sent off to `re_renderer`.
/// (Some meta information still relevant to ui setup as well)
///
/// TODO(andreas): Right now we're using `re_renderer` data structures for reading (bounding box & picking).
///                 In the future, this will be more limited as we're going to gpu staging data as soon as possible
///                 which is very slow to read. See [#594](https://github.com/rerun-io/rerun/pull/594)
#[derive(Default)]
pub struct SceneSpatialPrimitives {
    /// Estimated bounding box of all data in scene coordinates. Accumulated.
    pub(super) bounding_box_2d: macaw::BoundingBox,
    pub(super) bounding_box_3d: macaw::BoundingBox,

    // TODO(andreas): Storing extra data like so is unsafe and not future proof either
    //                (see also above comment on the need to separate cpu-readable data)
    pub textured_rectangles_ids: Vec<InstancePathHash>,
    pub textured_rectangles: Vec<re_renderer::renderer::TexturedRect>,

    pub line_strips: LineStripSeriesBuilder<InstancePathHash>,
    pub points: PointCloudBuilder<InstancePathHash>,

    pub meshes: Vec<MeshSource>,
}

const AXIS_COLOR_X: Color32 = Color32::from_rgb(255, 25, 25);
const AXIS_COLOR_Y: Color32 = Color32::from_rgb(0, 240, 0);
const AXIS_COLOR_Z: Color32 = Color32::from_rgb(80, 80, 255);

impl SceneSpatialPrimitives {
    /// bounding box covering the rendered scene
    pub fn bounding_box(&self, nav_mode: SpatialNavigationMode) -> macaw::BoundingBox {
        match nav_mode {
            SpatialNavigationMode::TwoD => self.bounding_box_2d,
            SpatialNavigationMode::ThreeD => self.bounding_box_3d,
        }
    }

    /// Number of primitives. Rather arbitrary what counts as a primitive so use this only for heuristic purposes!
    pub fn num_primitives(&self) -> usize {
        let Self {
            bounding_box_2d: _,
            bounding_box_3d: _,
            textured_rectangles,
            textured_rectangles_ids: _,
            line_strips,
            points,
            meshes,
        } = &self;

        textured_rectangles.len()
            + line_strips.vertices.len()
            + points.vertices.len()
            + meshes.len()
    }

    pub fn recalculate_bounding_boxes(&mut self) {
        crate::profile_function!();

        self.bounding_box_2d = macaw::BoundingBox::nothing();
        self.bounding_box_3d = macaw::BoundingBox::nothing();

        for bb in [&mut self.bounding_box_2d, &mut self.bounding_box_3d] {
            for rect in &self.textured_rectangles {
                bb.extend(rect.top_left_corner_position);
                bb.extend(rect.top_left_corner_position + rect.extent_u);
                bb.extend(rect.top_left_corner_position + rect.extent_v);
                bb.extend(rect.top_left_corner_position + rect.extent_v + rect.extent_u);
            }
        }

        // We don't need a very accurate bounding box, so in order to save some time,
        // we calculate a per batch bounding box for lines and points.
        // TODO(andreas): We should keep these around to speed up picking!
        for (batch, vertex_iter) in self.points.iter_vertices_by_batch() {
            let proj = &glam::Affine3A::from_mat4(batch.world_from_obj);

            let batch_bb = macaw::BoundingBox::from_points(vertex_iter.map(|v| v.position));

            let bb_3d = batch_bb.transform_affine3(proj);

            let bb_2d = macaw::BoundingBox::from_points(batch_bb.corners().iter().map(|c| {
                let pt = proj.transform_point3(*c);
                let norm = pt.z.signum() * pt.z.abs().at_least(1.0);
                pt / norm
            }));

            self.bounding_box_2d = self.bounding_box_2d.union(bb_2d);
            self.bounding_box_3d = self.bounding_box_3d.union(bb_3d);
        }
        for (batch, vertex_iter) in self.line_strips.iter_vertices_by_batch() {
            let batch_bb = macaw::BoundingBox::from_points(vertex_iter.map(|v| v.position));
            self.bounding_box_3d = self.bounding_box_3d.union(
                batch_bb.transform_affine3(&glam::Affine3A::from_mat4(batch.world_from_obj)),
            );
        }

        for mesh in &self.meshes {
            self.bounding_box_3d = self
                .bounding_box_3d
                .union(mesh.mesh.bbox().transform_affine3(&mesh.world_from_mesh));
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
                        mesh: None, // Don't care.
                        world_from_mesh: base_transform * mesh_instance.world_from_mesh,
                        additive_tint: mesh.additive_tint,
                    })
            })
            .collect()
    }

    pub fn add_axis_lines(
        &mut self,
        transform: macaw::IsoTransform,
        instance_path_hash: InstancePathHash,
        axis_length: f32,
    ) {
        use re_renderer::renderer::LineStripFlags;

        // TODO(andreas): It would be nice if could display the semantics (left/right/up) as a tooltip on hover.
        let line_radius = re_renderer::Size::new_scene(axis_length * 0.05);
        let origin = transform.translation();

        let mut line_batch = self.line_strips.batch("origin axis");
        line_batch
            .add_segment(
                origin,
                origin + transform.transform_vector3(glam::Vec3::X) * axis_length,
            )
            .radius(line_radius)
            .color(AXIS_COLOR_X)
            .flags(LineStripFlags::CAP_END_TRIANGLE | LineStripFlags::CAP_START_ROUND)
            .user_data(instance_path_hash);
        line_batch
            .add_segment(
                origin,
                origin + transform.transform_vector3(glam::Vec3::Y) * axis_length,
            )
            .radius(line_radius)
            .color(AXIS_COLOR_Y)
            .flags(LineStripFlags::CAP_END_TRIANGLE | LineStripFlags::CAP_START_ROUND)
            .user_data(instance_path_hash);
        line_batch
            .add_segment(
                origin,
                origin + transform.transform_vector3(glam::Vec3::Z) * axis_length,
            )
            .radius(line_radius)
            .color(AXIS_COLOR_Z)
            .flags(LineStripFlags::CAP_END_TRIANGLE | LineStripFlags::CAP_START_ROUND)
            .user_data(instance_path_hash);
    }
}
