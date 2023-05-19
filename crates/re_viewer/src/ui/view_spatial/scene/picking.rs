//! Handles picking in 2D & 3D spaces.

use ahash::HashSet;
use re_data_store::InstancePathHash;
use re_log_types::component_types::InstanceKey;
use re_renderer::PickingLayerProcessor;

use super::{Image, SceneSpatialPrimitives, SceneSpatialUiData};
use crate::{
    misc::instance_hash_conversions::instance_path_hash_from_picking_layer_id,
    ui::view_spatial::eye::Eye,
};

#[derive(Clone, PartialEq, Eq)]
pub enum PickingHitType {
    /// The hit was a textured rect.
    TexturedRect,

    /// The result came from GPU based picking.
    GpuPickingResult,

    /// We hit a egui ui element, meaning that depth information is not usable.
    GuiOverlay,
}

#[derive(Clone, PartialEq)]
pub struct PickingRayHit {
    /// What entity or instance got hit by the picking ray.
    ///
    /// The ray hit position may not actually be on this entity, as we allow snapping to closest entity!
    pub instance_path_hash: InstancePathHash,

    /// Where the ray hit the entity.
    pub space_position: glam::Vec3,

    pub depth_offset: re_renderer::DepthOffset,

    /// Any additional information about the picking hit.
    pub hit_type: PickingHitType,
}

#[derive(Clone, PartialEq)]
pub struct PickingResult {
    /// Picking ray hits. NOT sorted by distance but rather by source of picking.
    ///
    /// Typically there is only one hit, but there might be several if there are transparent objects
    /// or "aggressive" objects like 2D images which we always want to pick, even if they're in the background.
    /// (This is very useful for 2D scenes and so far we keep this behavior in 3D for simplicity)
    pub hits: Vec<PickingRayHit>,
}

impl PickingResult {
    pub fn space_position(&self) -> Option<glam::Vec3> {
        // Use gpu hit if available as they are usually the position one expects.
        // (other picking sources might be in here even if hidden!)
        self.hits
            .iter()
            .find(|h| h.hit_type == PickingHitType::GpuPickingResult)
            .or_else(|| self.hits.first())
            .map(|hit| hit.space_position)
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

        // Gather picking results from different sources.
        let gpu_pick = picking_gpu(
            render_ctx,
            gpu_readback_identifier,
            self,
            previous_picking_result,
        );
        let mut rect_hits = picking_textured_rects(self, &primitives.images);
        rect_hits.sort_by(|a, b| b.depth_offset.cmp(&a.depth_offset));
        let ui_rect_hits = picking_ui_rects(self, ui_data);

        let mut hits = Vec::new();

        // Start with gpu based picking as baseline. This is our prime source of picking information.
        //
        // ..unless the same object got also picked as part of a textured rect.
        // Textured rect picks also know where on the rect, making this the better source!
        // Note that whenever this happens, it means that the same object path has a textured rect and something else
        // e.g. a camera.
        if let Some(gpu_pick) = gpu_pick {
            if rect_hits.iter().all(|rect_hit| {
                rect_hit.instance_path_hash.entity_path_hash
                    != gpu_pick.instance_path_hash.entity_path_hash
            }) {
                hits.push(gpu_pick);
            }
        }

        // We never throw away any textured rects, even if they're behind other objects.
        hits.extend(rect_hits);

        // UI rects are overlaid on top, but we don't let them hide other picking results either.
        // Give any other previous hits precedence.
        let previously_hit_objects: HashSet<_> = hits
            .iter()
            .map(|prev_hit| prev_hit.instance_path_hash)
            .collect();
        hits.extend(
            ui_rect_hits
                .into_iter()
                .filter(|ui_hit| !previously_hit_objects.contains(&ui_hit.instance_path_hash)),
        );

        PickingResult { hits }
    }
}

fn picking_gpu(
    render_ctx: &re_renderer::RenderContext,
    gpu_readback_identifier: u64,
    context: &PickingContext,
    previous_picking_result: &Option<PickingResult>,
) -> Option<PickingRayHit> {
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
            context.pointer_in_pixel - gpu_picking_result.rect.min.as_vec2();
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
            return None;
        }

        let picked_world_position =
            gpu_picking_result.picked_world_position(picked_on_picking_rect.as_uvec2());

        Some(PickingRayHit {
            instance_path_hash: instance_path_hash_from_picking_layer_id(picked_id),
            space_position: picked_world_position,
            depth_offset: 1,
            hit_type: PickingHitType::GpuPickingResult,
        })
    } else {
        // It is possible that some frames we don't get a picking result and the frame after we get several.
        // We need to cache the last picking result and use it until we get a new one or the mouse leaves the screen.
        // (Andreas: On my mac this *actually* happens in very simple scenes, I get occasional frames with 0 and then with 2 picking results!)
        if let Some(PickingResult { hits }) = previous_picking_result {
            for previous_opaque_hit in hits.iter() {
                if matches!(
                    previous_opaque_hit.hit_type,
                    PickingHitType::GpuPickingResult
                ) {
                    return Some(previous_opaque_hit.clone());
                }
            }
        }
        None
    }
}

fn picking_textured_rects(context: &PickingContext, images: &[Image]) -> Vec<PickingRayHit> {
    crate::profile_function!();

    let mut hits = Vec::new();

    for image in images {
        let rect = &image.textured_rect;
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
        let intersection_world = context.ray_in_world.point_along(t);
        let dir_from_rect_top_left = intersection_world - rect.top_left_corner_position;
        let u = dir_from_rect_top_left.dot(rect.extent_u) / rect.extent_u.length_squared();
        let v = dir_from_rect_top_left.dot(rect.extent_v) / rect.extent_v.length_squared();

        if (0.0..=1.0).contains(&u) && (0.0..=1.0).contains(&v) {
            hits.push(PickingRayHit {
                instance_path_hash: InstancePathHash {
                    entity_path_hash: image.ent_path.hash(),
                    instance_key: InstanceKey::from_2d_image_coordinate(
                        [
                            (u * rect.colormapped_texture.texture.width() as f32) as u32,
                            (v * rect.colormapped_texture.texture.height() as f32) as u32,
                        ],
                        rect.colormapped_texture.texture.width() as u64,
                    ),
                },
                space_position: intersection_world,
                hit_type: PickingHitType::TexturedRect,
                depth_offset: rect.options.depth_offset,
            });
        }
    }

    hits
}

fn picking_ui_rects(
    context: &PickingContext,
    ui_data: &SceneSpatialUiData,
) -> Option<PickingRayHit> {
    crate::profile_function!();

    let egui_pos = egui::pos2(context.pointer_in_space2d.x, context.pointer_in_space2d.y);
    for (bbox, instance_hash) in &ui_data.pickable_ui_rects {
        if bbox.contains(egui_pos) {
            // Handle only a single ui rectangle (exit right away, ignore potential overlaps)
            return Some(PickingRayHit {
                instance_path_hash: *instance_hash,
                space_position: context.ray_in_world.origin,
                hit_type: PickingHitType::GuiOverlay,
                depth_offset: 0,
            });
        }
    }
    None
}
