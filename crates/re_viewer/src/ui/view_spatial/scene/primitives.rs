use egui::Color32;
use itertools::Itertools as _;
use re_data_store::InstanceIdHash;
use re_renderer::{renderer::MeshInstance, LineStripSeriesBuilder, PointCloudBuilder};

use crate::{
    math::{line_segment_distance_sq_to_point_2d, ray_closet_t_line_segment},
    ui::view_spatial::eye::Eye,
};

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
    bounding_box: macaw::BoundingBox,

    // TODO: should we store metadata on renderer? probably not future proof for upcoming changes.
    pub textured_rectangles: Vec<re_renderer::renderer::TexturedRect>,
    pub textured_rectangles_ids: Vec<InstanceIdHash>,

    pub line_strips: LineStripSeriesBuilder<InstanceIdHash>,
    pub points: PointCloudBuilder<InstanceIdHash>,

    pub meshes: Vec<MeshSource>,
}

pub enum AdditionalPickingInfo {
    /// No additional picking information.
    None,
    /// The hit was a textured rect at the given uv coordinates (ranging from 0 to 1)
    TexturedRect(glam::Vec2),
}

pub struct PickingResult {
    /// What object got hit by the picking ray.
    pub instance_hash: InstanceIdHash,

    /// Hit position in the coordinate system of the current space.
    /// I.e. the renderer's world position.
    pub space_position: glam::Vec3,

    /// Any additional information about the picking hit.
    pub info: AdditionalPickingInfo,
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

    pub fn picking(
        &self,
        pointer_in_ui: glam::Vec2,
        rect: &egui::Rect,
        eye: &Eye,
    ) -> Vec<PickingResult> {
        crate::profile_function!();

        let ui_from_world = eye.ui_from_world(rect);
        let ray_in_world = eye.picking_ray(rect, pointer_in_ui);

        let Self {
            bounding_box: _,
            textured_rectangles,
            textured_rectangles_ids,
            line_strips,
            points,
            meshes,
        } = &self;

        // in ui points
        let max_side_ui_dist_sq = 5.0 * 5.0; // TODO(emilk): interaction radius from egui
        let mut closest_opaque_ray_t = f32::INFINITY;
        let mut closest_opaque_side_ui_dist_sq = max_side_ui_dist_sq;
        let mut closest_opaque_instance_hash = None;
        let mut closest_opaque_picking_info = AdditionalPickingInfo::None;

        let mut update_closest = |t, side_ui_dist_sq, instance_hash: &InstanceIdHash| {
            if side_ui_dist_sq <= max_side_ui_dist_sq
                && t <= closest_opaque_ray_t
                && side_ui_dist_sq <= closest_opaque_side_ui_dist_sq
            {
                closest_opaque_ray_t = t;
                closest_opaque_side_ui_dist_sq = side_ui_dist_sq;
                closest_opaque_instance_hash = Some(*instance_hash);
                true
            } else {
                false
            }
        };

        {
            crate::profile_scope!("points");

            for (batch, vertex_iter) in points.iter_vertices_and_userdata_by_batch() {
                // For getting the closest point we could transform the mouse ray into the "batch space".
                // However, we want to determine the closest point in *screen space*, meaning that we need to project all points.
                let ui_from_batch = ui_from_world * batch.world_from_obj;

                for (point, instance_hash) in vertex_iter {
                    if instance_hash.is_none() {
                        continue;
                    }

                    // TODO(emilk): take point radius into account
                    let pos_in_ui = ui_from_batch.project_point3(point.position);
                    let dist_sq = pos_in_ui.truncate().distance_squared(pointer_in_ui);
                    if dist_sq < max_side_ui_dist_sq {
                        let t = ray_in_world.closest_t_to_point(
                            batch.world_from_obj.transform_point3(point.position),
                        );
                        update_closest(t, dist_sq, instance_hash);
                    }
                }
            }
        }

        {
            crate::profile_scope!("line_segments");

            for (batch, vertices) in line_strips.iter_vertices_by_batch() {
                // For getting the closest point we could transform the mouse ray into the "batch space".
                // However, we want to determine the closest point in *screen space*, meaning that we need to project all points.
                let ui_from_batch = ui_from_world * batch.world_from_obj;

                for (start, end) in vertices.tuple_windows() {
                    // Skip unconnected tuples.
                    if start.strip_index != end.strip_index {
                        continue;
                    }

                    let instance_hash = line_strips.strip_user_data[start.strip_index as usize];
                    if instance_hash.is_none() {
                        continue;
                    }

                    // TODO(emilk): take line segment radius into account
                    let a = ui_from_batch.project_point3(start.position);
                    let b = ui_from_batch.project_point3(end.position);
                    let side_ui_dist_sq = line_segment_distance_sq_to_point_2d(
                        [a.truncate(), b.truncate()],
                        pointer_in_ui,
                    );

                    if side_ui_dist_sq < max_side_ui_dist_sq {
                        let start_world = batch.world_from_obj.transform_point3(start.position);
                        let end_world = batch.world_from_obj.transform_point3(end.position);
                        let t = ray_closet_t_line_segment(&ray_in_world, [start_world, end_world]);

                        update_closest(t, side_ui_dist_sq, &instance_hash);
                    }
                }
            }
        }

        {
            crate::profile_scope!("meshes");
            for mesh in meshes {
                if !mesh.instance_hash.is_some() {
                    continue;
                }
                let ray_in_mesh = (mesh.world_from_mesh.inverse() * ray_in_world).normalize();
                let t = crate::math::ray_bbox_intersect(&ray_in_mesh, mesh.mesh.bbox());

                if t < 0.0 {
                    let side_ui_dist_sq = 0.0;
                    update_closest(t, side_ui_dist_sq, &mesh.instance_hash);
                }
            }
        }

        {
            crate::profile_scope!("rectangles");
            for (rect, id) in textured_rectangles
                .iter()
                .zip(textured_rectangles_ids.iter())
            {
                if !id.is_some() {
                    continue;
                }

                let rect_plane = macaw::Plane3::from_normal_point(
                    rect.extent_u.cross(rect.extent_v).normalize(),
                    rect.top_left_corner_position,
                );

                // TODO(andreas): Interaction radius is currently ignored for rects.
                let (intersect, t) =
                    rect_plane.intersect_ray(ray_in_world.origin, ray_in_world.dir);
                if !intersect {
                    continue;
                }

                let intersection_world = ray_in_world.origin + ray_in_world.dir * t;
                let dir_from_rect_top_left = intersection_world - rect.top_left_corner_position;
                let u = dir_from_rect_top_left.dot(rect.extent_u) / rect.extent_u.length_squared();
                let v = dir_from_rect_top_left.dot(rect.extent_v) / rect.extent_v.length_squared();

                // TODO: multi intersect / transparent rects
                if (0.0..=1.0).contains(&u)
                    && (0.0..=1.0).contains(&v)
                    && update_closest(t, 0.0, id)
                {
                    closest_opaque_picking_info =
                        AdditionalPickingInfo::TexturedRect(glam::vec2(u, v));
                }
            }
        }

        if let Some(closest_opaque_instance_hash) = closest_opaque_instance_hash {
            let space_position = ray_in_world.origin + ray_in_world.dir * closest_opaque_ray_t;
            vec![PickingResult {
                instance_hash: closest_opaque_instance_hash,
                space_position,
                info: closest_opaque_picking_info,
            }]
        } else {
            Vec::new()
        }
    }
}
