//! Handles picking in 2D & 3D spaces.

use re_data_store::InstancePathHash;
use re_renderer::PickingLayerProcessor;

use super::{SceneSpatialPrimitives, SceneSpatialUiData};
use crate::{
    misc::instance_hash_conversions::instance_path_hash_from_picking_layer_id,
    ui::view_spatial::eye::Eye,
};

#[derive(Clone)]
pub enum AdditionalPickingInfo {
    /// No additional picking information.
    None,

    /// The hit was a textured rect at the given uv coordinates (ranging from 0 to 1)
    TexturedRect(glam::Vec2),

    /// The result came from GPU based picking.
    GpuPickingResult,

    /// We hit a egui ui element, meaning that depth information is not usable.
    GuiOverlay,
}

#[derive(Clone)]
pub struct PickingRayHit {
    /// What entity or instance got hit by the picking ray.
    ///
    /// The ray hit position may not actually be on this entity, as we allow snapping to closest entity!
    pub instance_path_hash: InstancePathHash,

    /// Where along the picking ray the hit occurred.
    pub ray_t: f32,

    pub depth_offset: re_renderer::DepthOffset,

    /// Any additional information about the picking hit.
    pub info: AdditionalPickingInfo,
}

impl PickingRayHit {
    pub fn space_position(&self, ray_in_world: &macaw::Ray3) -> glam::Vec3 {
        ray_in_world.origin + ray_in_world.dir * self.ray_t
    }
}

#[derive(Clone)]
pub struct PickingResult {
    /// Picking ray hit for an opaque object (if any).
    pub opaque_hit: Option<PickingRayHit>,

    /// Picking ray hits for transparent objects, sorted from far to near.
    /// If there is an opaque hit, all of them are in front of the opaque hit.
    pub transparent_hits: Vec<PickingRayHit>,
}

impl PickingResult {
    /// Iterates over all hits from far to close.
    pub fn iter_hits(&self) -> impl Iterator<Item = &PickingRayHit> {
        self.opaque_hit.iter().chain(self.transparent_hits.iter())
    }

    pub fn space_position(&self, ray_in_world: &macaw::Ray3) -> Option<glam::Vec3> {
        self.opaque_hit
            .as_ref()
            .or_else(|| self.transparent_hits.last())
            .map(|hit| hit.space_position(ray_in_world))
    }
}

const RAY_T_EPSILON: f32 = f32::EPSILON;

/// State used to build up picking results.
struct PickingState {
    closest_opaque_side_ui_dist_sq: f32,
    closest_opaque_pick: PickingRayHit,
    transparent_hits: Vec<PickingRayHit>,
}

impl PickingState {
    fn check_hit(&mut self, side_ui_dist_sq: f32, ray_hit: PickingRayHit, transparent: bool) {
        let gap_to_closest_opaque = self.closest_opaque_pick.ray_t - ray_hit.ray_t;

        // Use depth offset if very close to each other in relative distance.
        if gap_to_closest_opaque.abs()
            < self.closest_opaque_pick.ray_t.max(ray_hit.ray_t) * RAY_T_EPSILON
        {
            if ray_hit.depth_offset < self.closest_opaque_pick.depth_offset {
                return;
            }
        } else if gap_to_closest_opaque < 0.0 {
            return;
        }

        if side_ui_dist_sq <= self.closest_opaque_side_ui_dist_sq {
            if transparent {
                self.transparent_hits.push(ray_hit);
            } else {
                self.closest_opaque_pick = ray_hit;
                self.closest_opaque_side_ui_dist_sq = side_ui_dist_sq;
            }
        }
    }

    fn sort_and_remove_hidden_transparent(&mut self) {
        // Sort from far to close
        self.transparent_hits
            .sort_by(|a, b| b.ray_t.partial_cmp(&a.ray_t).unwrap());

        // Delete subset that is behind opaque hit.
        if self.closest_opaque_pick.ray_t.is_finite() {
            let mut num_hidden = 0;
            for (i, transparent_hit) in self.transparent_hits.iter().enumerate() {
                if transparent_hit.ray_t <= self.closest_opaque_pick.ray_t {
                    break;
                }
                num_hidden = i + 1;
            }
            self.transparent_hits.drain(0..num_hidden);
        }
    }
}

/// Picking context in which picking is performed.
pub struct PickingContext {
    /// Cursor position in the UI coordinate system.
    pub pointer_in_ui: glam::Vec2,

    /// Cursor position on the renderer canvas in pixels.
    pub pointer_in_pixel: glam::Vec2,

    /// Cursor position in the 2D space coordinate system.
    ///
    /// For 3D spaces this is equal to the cursor position in pixel coordinate system.
    pub pointer_in_space2d: glam::Vec2,

    /// The picking ray used. Given in the coordinates of the space the picking is performed in.
    pub ray_in_world: macaw::Ray3,

    /// Multiply with this to convert to pixels from points.
    pixels_from_points: f32,
}

impl PickingContext {
    /// Radius in which cursor interactions may snap to the nearest object even if the cursor
    /// does not hover it directly.
    ///
    /// Note that this needs to be scaled when zooming is applied by the virtual->visible ui rect transform.
    pub const UI_INTERACTION_RADIUS: f32 = 5.0;

    pub fn new(
        pointer_in_ui: egui::Pos2,
        space2d_from_ui: eframe::emath::RectTransform,
        ui_clip_rect: egui::Rect,
        pixels_from_points: f32,
        eye: &Eye,
    ) -> PickingContext {
        let pointer_in_space2d = space2d_from_ui.transform_pos(pointer_in_ui);
        let pointer_in_space2d = glam::vec2(pointer_in_space2d.x, pointer_in_space2d.y);
        let pointer_in_pixel = (pointer_in_ui - ui_clip_rect.left_top()) * pixels_from_points;

        PickingContext {
            pointer_in_space2d,
            pointer_in_pixel: glam::vec2(pointer_in_pixel.x, pointer_in_pixel.y),
            pointer_in_ui: glam::vec2(pointer_in_ui.x, pointer_in_ui.y),
            ray_in_world: eye.picking_ray(*space2d_from_ui.to(), pointer_in_space2d),
            pixels_from_points,
        }
    }

    /// Performs picking for a given scene.
    pub fn pick(
        &self,
        render_ctx: &re_renderer::RenderContext,
        gpu_readback_identifier: re_renderer::GpuReadbackIdentifier,
        previous_picking_result: &Option<PickingResult>,
        primitives: &SceneSpatialPrimitives,
        ui_data: &SceneSpatialUiData,
    ) -> PickingResult {
        crate::profile_function!();

        let max_side_ui_dist_sq = Self::UI_INTERACTION_RADIUS * Self::UI_INTERACTION_RADIUS;

        let mut state = PickingState {
            closest_opaque_side_ui_dist_sq: max_side_ui_dist_sq,
            closest_opaque_pick: PickingRayHit {
                instance_path_hash: InstancePathHash::NONE,
                ray_t: f32::INFINITY,
                info: AdditionalPickingInfo::None,
                depth_offset: 0,
            },
            // Combined, sorted (and partially "hidden") by opaque results later.
            transparent_hits: Vec::new(),
        };

        picking_gpu(
            render_ctx,
            gpu_readback_identifier,
            &mut state,
            self,
            previous_picking_result,
        );
        picking_textured_rects(
            self,
            &mut state,
            &primitives.textured_rectangles,
            &primitives.textured_rectangles_ids,
        );
        picking_ui_rects(self, &mut state, ui_data);

        state.sort_and_remove_hidden_transparent();

        PickingResult {
            opaque_hit: state
                .closest_opaque_pick
                .instance_path_hash
                .is_some()
                .then_some(state.closest_opaque_pick),
            transparent_hits: state.transparent_hits,
        }
    }
}

fn picking_gpu(
    render_ctx: &re_renderer::RenderContext,
    gpu_readback_identifier: u64,
    state: &mut PickingState,
    context: &PickingContext,
    previous_picking_result: &Option<PickingResult>,
) {
    crate::profile_function!();

    // Only look at newest available result, discard everything else.
    let mut gpu_picking_result = None;
    while let Some(picking_result) =
        PickingLayerProcessor::next_readback_result::<()>(render_ctx, gpu_readback_identifier)
    {
        gpu_picking_result = Some(picking_result);
    }

    if let Some(gpu_picking_result) = gpu_picking_result {
        // First, figure out where on the rect the cursor is by now.
        // (for simplicity, we assume the screen hasn't been resized)
        let pointer_on_picking_rect =
            context.pointer_in_pixel - gpu_picking_result.rect.left_top.as_vec2();
        // The cursor might have moved outside of the rect. Clamp it back in.
        let pointer_on_picking_rect = pointer_on_picking_rect.clamp(
            glam::Vec2::ZERO,
            (gpu_picking_result.rect.extent - glam::UVec2::ONE).as_vec2(),
        );

        // Find closest non-zero pixel to the cursor.
        let mut picked_id = re_renderer::PickingLayerId::default();
        let mut picked_on_picking_rect = glam::Vec2::ZERO;
        let mut closest_rect_distance_sq = f32::INFINITY;

        for (i, id) in gpu_picking_result.picking_id_data.iter().enumerate() {
            if id.object.0 != 0 {
                let current_pos_on_picking_rect = glam::uvec2(
                    i as u32 % gpu_picking_result.rect.extent.x,
                    i as u32 / gpu_picking_result.rect.extent.x,
                )
                .as_vec2()
                    + glam::vec2(0.5, 0.5); // Use pixel center for distances.
                let distance_sq =
                    current_pos_on_picking_rect.distance_squared(pointer_on_picking_rect);
                if distance_sq < closest_rect_distance_sq {
                    picked_on_picking_rect = current_pos_on_picking_rect;
                    closest_rect_distance_sq = distance_sq;
                    picked_id = *id;
                }
            }
        }
        if picked_id == re_renderer::PickingLayerId::default() {
            // Nothing found.
            return;
        }

        let ui_distance_sq = picked_on_picking_rect.distance_squared(pointer_on_picking_rect)
            / (context.pixels_from_points * context.pixels_from_points);
        let picked_world_position =
            gpu_picking_result.picked_world_position(picked_on_picking_rect.as_uvec2());
        state.check_hit(
            ui_distance_sq,
            PickingRayHit {
                instance_path_hash: instance_path_hash_from_picking_layer_id(picked_id),
                // TODO(andreas): Once this is the primary path we should not awkwardly reconstruct the ray_t here. It's not entirely correct either!
                ray_t: picked_world_position.distance(context.ray_in_world.origin),
                depth_offset: 0,
                info: AdditionalPickingInfo::GpuPickingResult,
            },
            false,
        );
    } else {
        // It is possible that some frames we don't get a picking result and the frame after we get several.
        // We need to cache the last picking result and use it until we get a new one or the mouse leaves the screen.
        // (Andreas: On my mac this *actually* happens in very simple scenes, I get occasional frames with 0 and then with 2 picking results!)
        if let Some(PickingResult {
            opaque_hit: Some(previous_opaque_hit),
            ..
        }) = previous_picking_result
        {
            if matches!(
                previous_opaque_hit.info,
                AdditionalPickingInfo::GpuPickingResult
            ) {
                state.closest_opaque_pick = previous_opaque_hit.clone();
            }
        }
    }
}

fn picking_textured_rects(
    context: &PickingContext,
    state: &mut PickingState,
    textured_rectangles: &[re_renderer::renderer::TexturedRect],
    textured_rectangles_ids: &[InstancePathHash],
) {
    crate::profile_function!();

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
            rect_plane.intersect_ray(context.ray_in_world.origin, context.ray_in_world.dir);
        if !intersect {
            continue;
        }
        let intersection_world = context.ray_in_world.origin + context.ray_in_world.dir * t;
        let dir_from_rect_top_left = intersection_world - rect.top_left_corner_position;
        let u = dir_from_rect_top_left.dot(rect.extent_u) / rect.extent_u.length_squared();
        let v = dir_from_rect_top_left.dot(rect.extent_v) / rect.extent_v.length_squared();

        if (0.0..=1.0).contains(&u) && (0.0..=1.0).contains(&v) {
            let picking_hit = PickingRayHit {
                instance_path_hash: *id,
                ray_t: t,
                info: AdditionalPickingInfo::TexturedRect(glam::vec2(u, v)),
                depth_offset: rect.depth_offset,
            };
            state.check_hit(0.0, picking_hit, rect.multiplicative_tint.a() < 1.0);
        }
    }
}

fn picking_ui_rects(
    context: &PickingContext,
    state: &mut PickingState,
    ui_data: &SceneSpatialUiData,
) {
    crate::profile_function!();

    let egui_pos = egui::pos2(context.pointer_in_space2d.x, context.pointer_in_space2d.y);
    for (bbox, instance_hash) in &ui_data.pickable_ui_rects {
        let side_ui_dist_sq = bbox.distance_sq_to_pos(egui_pos);
        state.check_hit(
            side_ui_dist_sq,
            PickingRayHit {
                instance_path_hash: *instance_hash,
                ray_t: 0.0,
                info: AdditionalPickingInfo::GuiOverlay,
                depth_offset: 0,
            },
            false,
        );
    }
}
