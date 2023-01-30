use egui::Color32;
use re_data_store::InstancePathHash;
use re_renderer::{renderer::MeshInstance, LineStripSeriesBuilder, PointCloudBuilder};

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
    pub(super) bounding_box: macaw::BoundingBox,

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
    pub fn bounding_box(&self) -> macaw::BoundingBox {
        self.bounding_box
    }

    /// Number of primitives. Rather arbitrary what counts as a primitive so use this only for heuristic purposes!
    pub fn num_primitives(&self) -> usize {
        let Self {
            bounding_box: _,
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

    pub fn recalculate_bounding_box(&mut self) {
        crate::profile_function!();

        self.bounding_box = macaw::BoundingBox::nothing();

        for rect in &self.textured_rectangles {
            self.bounding_box.extend(rect.top_left_corner_position);
            self.bounding_box
                .extend(rect.top_left_corner_position + rect.extent_u);
            self.bounding_box
                .extend(rect.top_left_corner_position + rect.extent_v);
            self.bounding_box
                .extend(rect.top_left_corner_position + rect.extent_v + rect.extent_u);
        }

        // We don't need a very accurate bounding box, so in order to save some time,
        // we calculate a per batch bounding box for lines and points.
        // TODO(andreas): We should keep these around to speed up picking!
        for (batch, vertex_iter) in self.points.iter_vertices_by_batch() {
            let batch_bb = macaw::BoundingBox::from_points(vertex_iter.map(|v| v.position));
            self.bounding_box = self.bounding_box.union(
                batch_bb.transform_affine3(&glam::Affine3A::from_mat4(batch.world_from_obj)),
            );
        }
        for (batch, vertex_iter) in self.line_strips.iter_vertices_by_batch() {
            let batch_bb = macaw::BoundingBox::from_points(vertex_iter.map(|v| v.position));
            self.bounding_box = self.bounding_box.union(
                batch_bb.transform_affine3(&glam::Affine3A::from_mat4(batch.world_from_obj)),
            );
        }

        for mesh in &self.meshes {
            self.bounding_box = self
                .bounding_box
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
                    .map(move |instance| MeshInstance {
                        gpu_mesh: instance.gpu_mesh.clone(),
                        mesh: None, // Don't care.
                        world_from_mesh: base_transform * instance.world_from_mesh,
                        additive_tint: mesh.additive_tint,
                    })
            })
            .collect()
    }

    pub fn add_axis_lines(
        &mut self,
        transform: macaw::IsoTransform,
        instance: InstancePathHash,
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
            .user_data(instance);
        line_batch
            .add_segment(
                origin,
                origin + transform.transform_vector3(glam::Vec3::Y) * axis_length,
            )
            .radius(line_radius)
            .color(AXIS_COLOR_Y)
            .flags(LineStripFlags::CAP_END_TRIANGLE | LineStripFlags::CAP_START_ROUND)
            .user_data(instance);
        line_batch
            .add_segment(
                origin,
                origin + transform.transform_vector3(glam::Vec3::Z) * axis_length,
            )
            .radius(line_radius)
            .color(AXIS_COLOR_Z)
            .flags(LineStripFlags::CAP_END_TRIANGLE | LineStripFlags::CAP_START_ROUND)
            .user_data(instance);
    }
}
