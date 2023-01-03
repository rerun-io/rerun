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

pub struct PickingRayHit {
    /// What object got hit by the picking ray.
    ///
    /// The ray hit position may not actually be on this object, as we allow snapping to closest object!
    pub instance_hash: InstanceIdHash,

    /// Where along the picking ray the hit occurred.
    pub ray_t: f32,

    /// Any additional information about the picking hit.
    pub info: AdditionalPickingInfo,
}

impl PickingRayHit {
    fn from_instance_and_t(instance_hash: InstanceIdHash, t: f32) -> Self {
        Self {
            instance_hash,
            ray_t: t,
            info: AdditionalPickingInfo::None,
        }
    }
}

pub struct PickingResult {
    /// Picking ray hit for an opaque object (if any).
    pub opaque_hit: Option<PickingRayHit>,

    /// Picking ray hits for transparent objects, sorted from far to near.
    /// If there is an opaque hit, all of them are in front of the opaque hit.
    pub transparent_hits: Vec<PickingRayHit>,

    /// The picking ray used. Given in the coordinates of the space the picking is performed in.
    picking_ray: macaw::Ray3,
}

impl PickingResult {
    /// The space position of a given hit.
    pub fn space_position(&self, hit: &PickingRayHit) -> glam::Vec3 {
        self.picking_ray.origin + self.picking_ray.dir * hit.ray_t
    }

    /// Iterates over all hits from far to close.
    pub fn iter_hits(&self) -> impl Iterator<Item = &PickingRayHit> {
        self.opaque_hit.iter().chain(self.transparent_hits.iter())
    }

    fn sort_and_remove_hidden_transparent(&mut self) {
        // Sort from far to close
        self.transparent_hits
            .sort_by(|a, b| b.ray_t.partial_cmp(&a.ray_t).unwrap());

        // Delete subset that is behind opaque hit.
        if let Some(opaque_hit) = &self.opaque_hit {
            for (i, transparent_hit) in self.transparent_hits.iter().enumerate() {
                if transparent_hit.ray_t >= opaque_hit.ray_t {
                    if i > 0 {
                        self.transparent_hits.drain(0..i);
                    }
                    break;
                }
            }
        }
    }
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

    /// Performs object picking in the scene.
    ///
    /// `pointer_in_ui`:  Position of the cursor in 2d screen coordinates, with [0,0] as top-left and ui point units.
    pub fn picking(
        &self,
        pointer_in_ui: glam::Vec2,
        rect: &egui::Rect,
        eye: &Eye,
        // TODO(andreas): Workaround for supporting labels in 2d spaces. Need to unify label and label picking handling!
        rects_2d: &[(egui::Rect, InstanceIdHash)],
    ) -> PickingResult {
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
        let mut closest_opaque_side_ui_dist_sq = max_side_ui_dist_sq;
        let mut closest_opqaue_pick = PickingRayHit {
            instance_hash: InstanceIdHash::NONE,
            ray_t: f32::INFINITY,
            info: AdditionalPickingInfo::None,
        };
        let mut transparent_hits = Vec::new(); // Combined, sorted (and partially "hidden") by opaque results later.

        let mut add_hit = |side_ui_dist_sq, ray_hit: PickingRayHit, transparent| {
            if ray_hit.ray_t < closest_opqaue_pick.ray_t
                && side_ui_dist_sq <= closest_opaque_side_ui_dist_sq
            {
                if transparent {
                    transparent_hits.push(ray_hit);
                } else {
                    closest_opqaue_pick = ray_hit;
                    closest_opaque_side_ui_dist_sq = side_ui_dist_sq;
                }
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
                    if dist_sq <= max_side_ui_dist_sq {
                        let t = ray_in_world.closest_t_to_point(
                            batch.world_from_obj.transform_point3(point.position),
                        );
                        add_hit(
                            dist_sq,
                            PickingRayHit::from_instance_and_t(*instance_hash, t),
                            false,
                        );
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

                        add_hit(
                            side_ui_dist_sq,
                            PickingRayHit::from_instance_and_t(instance_hash, t),
                            false,
                        );
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
                    add_hit(
                        side_ui_dist_sq,
                        PickingRayHit::from_instance_and_t(mesh.instance_hash, t),
                        false,
                    );
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

                if (0.0..=1.0).contains(&u) && (0.0..=1.0).contains(&v) {
                    let picking_hit = PickingRayHit {
                        instance_hash: *id,
                        ray_t: t,
                        info: AdditionalPickingInfo::TexturedRect(glam::vec2(u, v)),
                    };
                    add_hit(0.0, picking_hit, rect.multiplicative_tint.a() < 1.0);
                }
            }
        }

        // TODO(andreas): This doesn't make sense anymore in this framework as it is limited to pure 2d views.
        {
            crate::profile_scope!("2d rectangles");
            let mut check_hovering = |instance_hash, dist_sq: f32| {
                if dist_sq <= closest_opaque_side_ui_dist_sq {
                    closest_opaque_side_ui_dist_sq = dist_sq;
                    closest_opqaue_pick = PickingRayHit {
                        instance_hash,
                        ray_t: 0.0,
                        info: AdditionalPickingInfo::None,
                    };
                }
            };
            let pointer_pos2d = ray_in_world.origin.truncate();
            let pointer_pos2d = egui::pos2(pointer_pos2d.x, pointer_pos2d.y);

            for (bbox, instance_hash) in rects_2d {
                check_hovering(*instance_hash, bbox.distance_sq_to_pos(pointer_pos2d));
            }
        }

        let mut result = PickingResult {
            opaque_hit: closest_opqaue_pick
                .instance_hash
                .is_some()
                .then_some(closest_opqaue_pick),
            transparent_hits,
            picking_ray: ray_in_world,
        };
        result.sort_and_remove_hidden_transparent();
        result
    }
}
