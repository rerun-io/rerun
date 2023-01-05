use itertools::Itertools as _;

use re_data_store::InstanceIdHash;

use super::{SceneSpatialPrimitives, SceneSpatialUiData};
use crate::{
    math::{line_segment_distance_sq_to_point_2d, ray_closest_t_line_segment},
    ui::view_spatial::eye::Eye,
};

pub enum AdditionalPickingInfo {
    /// No additional picking information.
    None,
    /// The hit was a textured rect at the given uv coordinates (ranging from 0 to 1)
    TexturedRect(glam::Vec2),
    /// We hit a egui ui element, meaning that depth information is not usable.
    GuiOverlay,
}

pub struct PickingRayHit {
    /// What object got hit by the picking ray.
    ///
    /// The ray hit position may not actually be on this object, as we allow snapping to closest object!
    pub instance_hash: InstanceIdHash,

    /// Where along the picking ray the hit occurred.
    pub ray_t: f32,

    pub depth_offset: re_renderer::DepthOffset,

    /// Any additional information about the picking hit.
    pub info: AdditionalPickingInfo,
}

impl PickingRayHit {
    fn from_instance_and_t(instance_hash: InstanceIdHash, t: f32) -> Self {
        Self {
            instance_hash,
            ray_t: t,
            info: AdditionalPickingInfo::None,
            depth_offset: 0,
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
            let mut num_hidden = 0;
            for (i, transparent_hit) in self.transparent_hits.iter().enumerate() {
                if transparent_hit.ray_t <= opaque_hit.ray_t {
                    break;
                }
                num_hidden = i + 1;
            }
            self.transparent_hits.drain(0..num_hidden);
        }
    }
}

const RAY_T_EPSILON: f32 = f32::EPSILON;

pub fn picking(
    pointer_in_ui: glam::Vec2,
    ui_rect: &egui::Rect,
    eye: &Eye,
    primitives: &SceneSpatialPrimitives,
    ui_data: &SceneSpatialUiData,
    ui_interaction_radius: f32,
) -> PickingResult {
    crate::profile_function!();

    let ui_from_world = eye.ui_from_world(ui_rect);
    let ray_in_world = eye.picking_ray(ui_rect, pointer_in_ui);

    let SceneSpatialPrimitives {
        bounding_box: _,
        textured_rectangles,
        textured_rectangles_ids,
        line_strips,
        points,
        meshes,
    } = primitives;

    // in ui points
    let max_side_ui_dist_sq = ui_interaction_radius * ui_interaction_radius; // TODO(emilk): interaction radius from egui
    let mut closest_opaque_side_ui_dist_sq = max_side_ui_dist_sq;
    let mut closest_opaque_pick = PickingRayHit {
        instance_hash: InstanceIdHash::NONE,
        ray_t: f32::INFINITY,
        info: AdditionalPickingInfo::None,
        depth_offset: 0,
    };
    let mut transparent_hits = Vec::new(); // Combined, sorted (and partially "hidden") by opaque results later.

    let mut check_hit = |side_ui_dist_sq, ray_hit: PickingRayHit, transparent| {
        let gap_to_closest_opaque = closest_opaque_pick.ray_t - ray_hit.ray_t;

        // Use depth offset if very close to each other in relative distance.
        if gap_to_closest_opaque.abs()
            < closest_opaque_pick.ray_t.max(ray_hit.ray_t) * RAY_T_EPSILON
        {
            if ray_hit.depth_offset < closest_opaque_pick.depth_offset {
                return;
            }
        } else if gap_to_closest_opaque < 0.0 {
            return;
        }

        if side_ui_dist_sq <= closest_opaque_side_ui_dist_sq {
            if transparent {
                transparent_hits.push(ray_hit);
            } else {
                closest_opaque_pick = ray_hit;
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
                    let t = ray_in_world
                        .closest_t_to_point(batch.world_from_obj.transform_point3(point.position));
                    check_hit(
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
                    let t = ray_closest_t_line_segment(&ray_in_world, [start_world, end_world]);

                    check_hit(
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
                check_hit(
                    side_ui_dist_sq,
                    PickingRayHit::from_instance_and_t(mesh.instance_hash, t),
                    false,
                );
            }
        }
    }

    {
        crate::profile_scope!("textured rectangles");
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
            let (intersect, t) = rect_plane.intersect_ray(ray_in_world.origin, ray_in_world.dir);
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
                    depth_offset: rect.depth_offset,
                };
                check_hit(0.0, picking_hit, rect.multiplicative_tint.a() < 1.0);
            }
        }
    }

    {
        crate::profile_scope!("ui rectangles");
        let egui_pos = egui::pos2(pointer_in_ui.x, pointer_in_ui.y);

        for (bbox, instance_hash) in &ui_data.pickable_ui_rects {
            let side_ui_dist_sq = bbox.distance_sq_to_pos(egui_pos);
            check_hit(
                side_ui_dist_sq,
                PickingRayHit {
                    instance_hash: *instance_hash,
                    ray_t: 0.0,
                    info: AdditionalPickingInfo::GuiOverlay,
                    depth_offset: 0,
                },
                false,
            );
        }
    }

    let mut result = PickingResult {
        opaque_hit: closest_opaque_pick
            .instance_hash
            .is_some()
            .then_some(closest_opaque_pick),
        transparent_hits,
        picking_ray: ray_in_world,
    };
    result.sort_and_remove_hidden_transparent();
    result
}
