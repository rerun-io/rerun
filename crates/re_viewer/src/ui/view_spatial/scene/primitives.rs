use egui::Color32;
use re_data_store::InstanceIdHash;
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
    pub textured_rectangles_ids: Vec<InstanceIdHash>,
    pub textured_rectangles: Vec<re_renderer::renderer::TexturedRect>,

    pub line_strips: LineStripSeriesBuilder<InstanceIdHash>,
    pub points: PointCloudBuilder<InstanceIdHash>,

    pub meshes: Vec<MeshSource>,
}

impl SceneSpatialPrimitives {
    /// bounding box covering the rendered scene
    pub fn bounding_box(&self) -> macaw::BoundingBox {
        self.bounding_box
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
                        additive_tint: mesh.additive_tint.unwrap_or(Color32::TRANSPARENT),
                    })
            })
            .collect()
    }
}
