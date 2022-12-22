use egui::Color32;
use itertools::Itertools as _;
use re_data_store::InstanceIdHash;
use re_renderer::{renderer::MeshInstance, LineStripSeriesBuilder, PointCloudBuilder};

use crate::{math::line_segment_distance_sq_to_point_2d, ui::view_spatial::eye::Eye};

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

    /// TODO(andreas): Need to decide of this should be used for hovering as well. If so add another builder with meta-data?
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

    pub fn picking(
        &self,
        pointer_in_ui: glam::Vec2,
        rect: &egui::Rect,
        eye: &Eye,
    ) -> Option<(InstanceIdHash, glam::Vec3)> {
        crate::profile_function!();

        let ui_from_world = eye.ui_from_world(rect);
        let world_from_ui = eye.world_from_ui(rect);
        let ray_in_world = eye.picking_ray(rect, pointer_in_ui);

        let Self {
            bounding_box: _,
            textured_rectangles: _, // TODO(andreas): Should be able to pick 2d rectangles!
            line_strips,
            points,
            meshes,
        } = &self;

        // in points
        let max_side_dist_sq = 5.0 * 5.0; // TODO(emilk): interaction radius from egui

        let mut closest_z = f32::INFINITY;
        // in points
        let mut closest_side_dist_sq = max_side_dist_sq;
        let mut closest_instance_id = None;

        {
            crate::profile_scope!("points_3d");

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
                    if pos_in_ui.z < 0.0 {
                        continue; // TODO(emilk): don't we expect negative Z!? RHS etc
                    }
                    let dist_sq = pos_in_ui.truncate().distance_squared(pointer_in_ui);
                    if dist_sq < max_side_dist_sq {
                        let t = pos_in_ui.z.abs();
                        if t < closest_z || dist_sq < closest_side_dist_sq {
                            closest_z = t;
                            closest_side_dist_sq = dist_sq;
                            closest_instance_id = Some(*instance_hash);
                        }
                    }
                }
            }
        }

        {
            crate::profile_scope!("line_segments_3d");

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
                    let dist_sq = line_segment_distance_sq_to_point_2d(
                        [a.truncate(), b.truncate()],
                        pointer_in_ui,
                    );

                    if dist_sq < max_side_dist_sq {
                        let t = a.z.abs(); // not very accurate
                        if t < closest_z || dist_sq < closest_side_dist_sq {
                            closest_z = t;
                            closest_side_dist_sq = dist_sq;
                            closest_instance_id = Some(instance_hash);
                        }
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

                if t < f32::INFINITY {
                    let dist_sq = 0.0;
                    if t < closest_z || dist_sq < closest_side_dist_sq {
                        closest_z = t; // TODO(emilk): I think this is wrong
                        closest_side_dist_sq = dist_sq;
                        closest_instance_id = Some(mesh.instance_hash);
                    }
                }
            }
        }

        // TODO: Rectangles

        if let Some(closest_instance_id) = closest_instance_id {
            let closest_point = world_from_ui.project_point3(glam::Vec3::new(
                pointer_in_ui.x,
                pointer_in_ui.y,
                closest_z,
            ));
            Some((closest_instance_id, closest_point))
        } else {
            None
        }
    }
}
