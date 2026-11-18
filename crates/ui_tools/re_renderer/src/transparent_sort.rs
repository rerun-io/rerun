//! Shared CPU back-to-front sorting for transparent, index-lookup based renderers
//! (point clouds and gaussian splats).
//!
//! Primitives within a batch are sorted on the CPU each frame (during drawable collection) and the
//! resulting order is uploaded as an `R32Uint` lookup texture. The vertex shader redirects each
//! primitive through this texture so they are painted farthest-from-camera first, letting
//! premultiplied-alpha blending composite correctly. Sorting is only done within a single batch,
//! not against other batches or primitives.

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;

use crate::RenderContext;
use crate::allocator::DataTextureSource;
use crate::renderer::DrawableCollectionViewInfo;
use crate::wgpu_resources::{GpuBindGroup, GpuTexture};

/// Previous back-to-front ordering for each view rendering the same batch.
///
/// Keyed by [`crate::ViewBuilderId`] so several views can display the same primitives from
/// different angles without their orderings clobbering each other. Reused across frames: the eye
/// usually moves only a little, so last frame's ordering is a near-sorted starting point.
#[derive(Clone, Default, re_byte_size::SizeBytes)]
pub struct SortOrderCache {
    sort_orders: Arc<Mutex<HashMap<crate::ViewBuilderId, Vec<u32>>>>,
}

/// Per-batch data needed to sort its primitives back-to-front on the CPU.
#[derive(Clone)]
pub struct TransparentSort {
    /// Object-space center of each primitive in this batch, used as the sort key.
    pub object_positions: Arc<Vec<glam::Vec3>>,

    /// Transforms the camera into the same space as `object_positions` once per view.
    pub object_from_world: glam::Affine3A,

    /// Caller-owned scratch buffers holding each view's previous ordering.
    ///
    /// `None` disables cross-frame reuse, so the batch is sorted from scratch every frame.
    pub sort_order_cache: Option<SortOrderCache>,
}

/// Builds a lookup texture giving the batch-local back-to-front (farthest-first) primitive order,
/// so premultiplied-alpha blending composites correctly.
///
/// The texture holds batch-local indices (`0..num_primitives`); the shader adds the batch's first
/// primitive index. Returns `None` if the batch is empty or the upload failed.
pub fn build_back_to_front_lookup_texture(
    ctx: &RenderContext,
    sort: &TransparentSort,
    view_info: &DrawableCollectionViewInfo,
) -> Option<GpuTexture> {
    re_tracing::profile_function!();

    let object_positions = &sort.object_positions;
    let num_primitives = object_positions.len();
    if num_primitives == 0 {
        return None;
    }

    // Start from this view's previous ordering when we have one: the eye usually moves only a
    // little between frames, so the ordering is nearly correct already and re-sorting it is much
    // cheaper than sorting `0..n` from scratch (Rust's sort detects already-ordered runs).
    let mut order = sort
        .sort_order_cache
        .as_ref()
        .and_then(|cache| cache.sort_orders.lock().remove(&view_info.view_id))
        .filter(|order| order.len() == num_primitives)
        .unwrap_or_else(|| (0..num_primitives as u32).collect());

    let eye_object_position = sort
        .object_from_world
        .transform_point3(view_info.camera_world_position.into());
    {
        re_tracing::profile_scope!("sort");
        // `sort_by_cached_key` computes the key once per primitive (`O(n)` distance computations)
        // instead of recomputing it on every comparison like `sort_by` would.
        // The squared distance is non-negative and finite, so its `f32` bit pattern is monotonic
        // and usable as an integer sort key; `Reverse` gives us farthest-first.
        order.sort_by_cached_key(|&i| {
            // Sorting by radial distance keeps the ordering stable when the camera rotates.
            let distance_squared =
                object_positions[i as usize].distance_squared(eye_object_position);
            std::cmp::Reverse(distance_squared.to_bits())
        });
    }

    // Stash this frame's ordering to seed next frame's sort for this view.
    if let Some(cache) = &sort.sort_order_cache {
        cache
            .sort_orders
            .lock()
            .insert(view_info.view_id, order.clone());
    }

    let mut lookup_texture = DataTextureSource::new(ctx);
    if let Err(err) = lookup_texture.extend_from_slice(&order) {
        re_log::error_once!("Failed to upload index lookup texture: {err}");
        return None;
    }
    match lookup_texture.finish(wgpu::TextureFormat::R32Uint, "back_to_front_lookup_texture") {
        Ok(texture) => Some(texture),
        Err(err) => {
            re_log::error_once!("Failed to upload index lookup texture: {err}");
            None
        }
    }
}

/// Resources selected for a single batch in a single view.
pub struct SortedDrawable {
    pub batch_index: usize,

    /// The batch's back-to-front lookup texture bind group for this view, if it is sorted.
    pub lookup_bind_group: Option<GpuBindGroup>,
}

/// Per-frame, per-view sorted drawables for a draw data.
///
/// Entries from the previous frame are discarded when the draw data is reused in a new frame.
#[derive(Default)]
pub struct SortedDrawables {
    frame_index: u64,
    entries: Vec<SortedDrawable>,
}

impl SortedDrawables {
    /// Records a drawable for the current frame, clearing stale entries from previous frames.
    ///
    /// Returns the index of the pushed drawable, to be used as the draw-data payload.
    pub fn push_for_frame(&mut self, frame_index: u64, drawable: SortedDrawable) -> usize {
        if self.frame_index != frame_index {
            self.frame_index = frame_index;
            self.entries.clear();
        }
        self.entries.push(drawable);
        self.entries.len() - 1
    }

    /// The drawable previously recorded at `index` (a draw-data payload).
    pub fn get(&self, index: usize) -> &SortedDrawable {
        &self.entries[index]
    }
}
