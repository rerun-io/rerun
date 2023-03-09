use egui::Color32;
use re_data_store::InstancePathHash;
use re_renderer::{
    renderer::{DepthCloud, MeshInstance},
    LineStripSeriesBuilder, PointCloudBuilder,
};

use super::MeshSource;

/// Primitives sent off to `re_renderer`.
/// (Some meta information still relevant to ui setup as well)
///
/// TODO(andreas): Right now we're using `re_renderer` data structures for reading (bounding box & picking).
///                 In the future, this will be more limited as we're going to gpu staging data as soon as possible
///                 which is very slow to read. See [#594](https://github.com/rerun-io/rerun/pull/594)
pub struct SceneSpatialPrimitives {
    /// Estimated bounding box of all data in scene coordinates. Accumulated.
    pub(super) bounding_box: macaw::BoundingBox,

    // TODO(andreas): Storing extra data like so is unsafe and not future proof either
    //                (see also above comment on the need to separate cpu-readable data)
    pub textured_rectangles_ids: Vec<InstancePathHash>,
    pub textured_rectangles: Vec<re_renderer::renderer::TexturedRect>,

    pub line_strips: LineStripSeriesBuilder<InstancePathHash>,
    pub line_strips_outline_only: LineStripSeriesBuilder<()>,
    pub points: PointCloudBuilder<InstancePathHash>,

    pub meshes: Vec<MeshSource>,
    pub depth_clouds: Vec<DepthCloud>,

    pub any_outlines: bool,
}

const AXIS_COLOR_X: Color32 = Color32::from_rgb(255, 25, 25);
const AXIS_COLOR_Y: Color32 = Color32::from_rgb(0, 240, 0);
const AXIS_COLOR_Z: Color32 = Color32::from_rgb(80, 80, 255);

impl SceneSpatialPrimitives {
    pub fn new(re_ctx: &mut re_renderer::RenderContext) -> Self {
        Self {
            bounding_box: macaw::BoundingBox::nothing(),
            textured_rectangles_ids: Default::default(),
            textured_rectangles: Default::default(),
            line_strips: Default::default(),
            line_strips_outline_only: Default::default(),
            points: PointCloudBuilder::new(re_ctx),
            meshes: Default::default(),
            depth_clouds: Default::default(),
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
            textured_rectangles,
            textured_rectangles_ids: _,
            line_strips,
            line_strips_outline_only: _,
            points,
            meshes,
            depth_clouds,
            any_outlines: _,
        } = &self;

        textured_rectangles.len()
            + line_strips.vertices.len()
            + points.vertices.len()
            + meshes.len()
            + depth_clouds.len()
    }

    pub fn recalculate_bounding_box(&mut self) {
        crate::profile_function!();

        let Self {
            bounding_box,
            textured_rectangles_ids: _,
            textured_rectangles,
            line_strips,
            line_strips_outline_only: _,
            points,
            meshes,
            depth_clouds: _, // no bbox for depth clouds
            any_outlines: _,
        } = self;

        *bounding_box = macaw::BoundingBox::nothing();

        for rect in textured_rectangles {
            bounding_box.extend(rect.top_left_corner_position);
            bounding_box.extend(rect.top_left_corner_position + rect.extent_u);
            bounding_box.extend(rect.top_left_corner_position + rect.extent_v);
            bounding_box.extend(rect.top_left_corner_position + rect.extent_v + rect.extent_u);
        }

        // We don't need a very accurate bounding box, so in order to save some time,
        // we calculate a per batch bounding box for lines and points.
        // TODO(andreas): We should keep these around to speed up picking!
        for (batch, vertex_iter) in points.iter_vertices_by_batch() {
            // Only use points which are an IsoTransform to update the bounding box
            // This prevents crazy bounds-increases when projecting 3d to 2d
            // See: https://github.com/rerun-io/rerun/issues/1203
            if let Some(transform) = macaw::IsoTransform::from_mat4(&batch.world_from_obj) {
                let batch_bb = macaw::BoundingBox::from_points(vertex_iter.map(|v| v.position));
                *bounding_box = bounding_box.union(batch_bb.transform_affine3(&transform.into()));
            }
        }
        for (batch, vertex_iter) in line_strips.iter_vertices_by_batch() {
            // Only use points which are an IsoTransform to update the bounding box
            // This prevents crazy bounds-increases when projecting 3d to 2d
            // See: https://github.com/rerun-io/rerun/issues/1203
            if let Some(transform) = macaw::IsoTransform::from_mat4(&batch.world_from_obj) {
                let batch_bb = macaw::BoundingBox::from_points(vertex_iter.map(|v| v.position));
                *bounding_box = bounding_box.union(batch_bb.transform_affine3(&transform.into()));
            }
        }

        for mesh in meshes {
            // TODO(jleibs): is this safe for meshes or should we be doing the equivalent of the above?
            *bounding_box =
                bounding_box.union(mesh.mesh.bbox().transform_affine3(&mesh.world_from_mesh));
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
                        outline_mask: mesh.outline_mask,
                        ..Default::default()
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
