//! Point renderer for efficient rendering of point clouds.
//!
//!
//! How it works:
//! =================
//! Points are rendered as quads and stenciled out by a fragment shader.
//! Quad spanning happens in the vertex shader, uploaded are only the data for the actual points (no vertex buffer!).
//!
//! Like with the `super::lines::LineRenderer`, we're rendering as all quads in a single triangle list draw call.
//! (Rationale for this can be found in the [`crate::renderer::lines`]'s documentation)
//!
//! For WebGL compatibility, data is uploaded as textures. Color is stored in a separate srgb texture, meaning
//! that srgb->linear conversion happens on texture load.
//!

use std::collections::HashMap;
use std::num::NonZeroU64;
use std::ops::Range;
use std::sync::Arc;

use bitflags::bitflags;
use enumset::{EnumSet, enum_set};
use itertools::Itertools as _;
use parking_lot::Mutex;
use smallvec::smallvec;

use super::{DrawData, DrawError, RenderContext, Renderer};
use crate::allocator::{DataTextureSource, create_and_fill_uniform_buffer_batch};
use crate::draw_phases::{
    DrawPhase, OutlineMaskProcessor, PickingLayerObjectId, PickingLayerProcessor,
};
use crate::renderer::{DrawDataDrawable, DrawInstruction, DrawableCollectionViewInfo};
use crate::view_builder::ViewBuilder;
use crate::wgpu_resources::{
    BindGroupDesc, BindGroupEntry, BindGroupLayoutDesc, GpuBindGroup, GpuBindGroupLayoutHandle,
    GpuRenderPipelineHandle, GpuRenderPipelinePoolAccessor, GpuTexture, PipelineLayoutDesc,
    RenderPipelineDesc,
};
use crate::{
    DepthOffset, DrawableCollector, Label, OutlineMaskPreference, PointCloudBuilder,
    include_shader_module,
};

bitflags! {
    /// Property flags for a point batch
    ///
    /// Needs to be kept in sync with `point_cloud.wgsl`
    #[repr(C)]
    #[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct PointCloudBatchFlags : u32 {
        /// If true, we shade all points in the batch like spheres.
        const FLAG_ENABLE_SHADING = 0b0001;

        /// If true, draw 2D camera facing circles instead of spheres.
        const FLAG_DRAW_AS_CIRCLES = 0b0010;

        /// If true, the batch is rendered with premultiplied-alpha blending in the
        /// [`DrawPhase::Transparent`] phase instead of opaque alpha-to-coverage.
        ///
        /// This controls compositing independently of [`Self::FLAG_ENABLE_INDEX_LOOKUP`], since
        /// alpha-blended batches such as coplanar 2D points may not need sorting.
        const FLAG_PREMULTIPLIED_ALPHA = 0b0100;

        /// Enables point-index redirection through the batch's lookup texture.
        ///
        /// This controls vertex data access, not compositing, and is set internally only for
        /// batches that opt into back-to-front sorting.
        const FLAG_ENABLE_INDEX_LOOKUP = 0b1000;
    }
}

pub mod gpu_data {
    use crate::draw_phases::PickingLayerObjectId;
    use crate::{Size, wgpu_buffer_types};

    // Don't use `wgsl_buffer_types` since this data doesn't go into a buffer, so alignment rules don't apply like on buffers..

    /// Position and radius.
    #[repr(C, packed)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct PositionRadius {
        pub pos: glam::Vec3,

        /// Radius of the point in world space
        pub radius: Size, // Might use a f16 here to free memory for more data!
    }
    static_assertions::assert_eq_size!(PositionRadius, glam::Vec4);

    impl re_byte_size::SizeBytes for PositionRadius {
        // Plain-old-data, so nothing lives on the heap.
        const IS_POD: bool = true;

        #[inline]
        fn heap_size_bytes(&self) -> u64 {
            0
        }
    }

    impl PositionRadius {
        /// If there are fewer radii than positions,
        /// the last radius will be repeated for the remaining positions
        /// (clamp to edge).
        pub fn from_many(positions: &[glam::Vec3], radii: &[Size]) -> Vec<Self> {
            use itertools::izip;

            re_tracing::profile_function_if!(10_0000 < positions.len());
            if positions.len() == radii.len() {
                // Optimize common-case with simpler iterators.
                re_tracing::profile_scope_if!(10_000 < positions.len(), "zipped");
                izip!(positions.iter().copied(), radii.iter().copied())
                    .map(|(pos, radius)| Self { pos, radius })
                    .collect()
            } else {
                re_tracing::profile_scope_if!(10_000 < positions.len(), "extended-radius");
                izip!(
                    positions.iter().copied(),
                    std::iter::chain(
                        radii.iter().copied(),
                        std::iter::repeat(*radii.last().unwrap_or(&Size::ONE_UI_POINT))
                    )
                )
                .map(|(pos, radius)| Self { pos, radius })
                .collect()
            }
        }
    }

    /// Uniform buffer that changes once per draw data rendering.
    #[repr(C)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct DrawDataUniformBuffer {
        pub radius_boost_in_ui_points: wgpu_buffer_types::F32RowPadded,
        pub end_padding: [wgpu_buffer_types::PaddingRow; 16 - 1],
    }

    /// Uniform buffer that changes for every batch of points.
    #[repr(C)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct BatchUniformBuffer {
        pub world_from_obj: wgpu_buffer_types::Mat4,

        pub flags: u32, // PointCloudBatchFlags
        pub depth_offset: f32,

        /// Index of this batch's first point in the shared point-data textures.
        pub first_point_index: u32,
        pub _row_padding: u32,

        pub outline_mask_ids: wgpu_buffer_types::UVec2,
        pub picking_object_id: PickingLayerObjectId,

        pub end_padding: [wgpu_buffer_types::PaddingRow; 16 - 6],
    }
}

/// Per-frame back-to-front sorting data for a transparent point cloud batch.
///
/// Points within a transparent batch are sorted on the CPU each frame (in
/// [`PointCloudDrawData::collect_drawables`]) so they can be painted back-to-front.
/// Sorting is only done within a single batch, not against other batches or primitives.
#[derive(Clone)]
struct TransparentSort {
    /// Object-space center of each point in this batch, used as the sort key.
    object_positions: Arc<Vec<glam::Vec3>>,

    /// Transforms the camera into the same space as `object_positions` once per view.
    object_from_world: glam::Affine3A,

    /// Caller-owned scratch buffers holding each view's previous back-to-front ordering.
    ///
    /// `None` disables cross-frame reuse, so the cloud is sorted from scratch every frame.
    sort_order_cache: Option<PointCloudSortOrderCache>,
}

/// Previous point ordering for each view rendering the same point cloud batch.
#[derive(Clone, Default, re_byte_size::SizeBytes)]
pub struct PointCloudSortOrderCache {
    sort_orders: Arc<Mutex<HashMap<crate::ViewBuilderId, Vec<u32>>>>,
}

/// Builds a lookup texture that draws points back-to-front (farthest from the camera first),
/// allowing premultiplied-alpha blending to composite correctly.
fn build_back_to_front_lookup_texture(
    ctx: &RenderContext,
    sort: &TransparentSort,
    view_info: &DrawableCollectionViewInfo,
) -> Option<GpuTexture> {
    re_tracing::profile_function!();

    let object_positions = &sort.object_positions;
    let num_points = object_positions.len();
    if num_points == 0 {
        return None;
    }

    // Start from the previous frame's ordering when we have one: the eye usually moves only
    // a little between frames, so the ordering is nearly correct already and re-sorting it is
    // much cheaper than sorting `0..n` from scratch (Rust's sort detects already-ordered runs).
    let mut point_order = sort
        .sort_order_cache
        .as_ref()
        .and_then(|cache| cache.sort_orders.lock().remove(&view_info.view_id))
        .filter(|order| order.len() == num_points)
        .unwrap_or_else(|| (0..num_points as u32).collect());

    let eye_object_position = sort
        .object_from_world
        .transform_point3(view_info.camera_world_position.into());
    {
        re_tracing::profile_scope!("sort");
        // `sort_by_cached_key` computes the key once per point (`O(n)` distance computations)
        // instead of recomputing it on every comparison like `sort_by` would.
        // The squared distance is non-negative and finite, so its `f32` bit pattern is monotonic
        // and usable as an integer sort key; `Reverse` gives us farthest-first.
        point_order.sort_by_cached_key(|&i| {
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
            .insert(view_info.view_id, point_order.clone());
    }

    let mut lookup_texture = DataTextureSource::new(ctx);
    if let Err(err) = lookup_texture.extend_from_slice(&point_order) {
        re_log::error_once!("Failed to upload point index lookup texture: {err}");
        return None;
    }
    match lookup_texture.finish(
        wgpu::TextureFormat::R32Uint,
        "PointCloudDrawData::point_index_lookup_texture",
    ) {
        Ok(texture) => Some(texture),
        Err(err) => {
            re_log::error_once!("Failed to upload point index lookup texture: {err}");
            None
        }
    }
}

/// Internal, ready to draw representation of [`PointCloudBatchInfo`]
#[derive(Clone)]
struct PointCloudBatch {
    bind_group: GpuBindGroup,
    vertex_range: Range<u32>,
    center_world_position: glam::Vec3,
    active_phases: EnumSet<DrawPhase>,

    /// Set for transparent batches that should be painted back-to-front.
    sort: Option<TransparentSort>,
}

/// Resources selected for a single batch in a single view.
struct PointCloudDrawable {
    batch_index: usize,
    lookup_bind_group: Option<GpuBindGroup>,
}

#[derive(Default)]
struct PointCloudDrawables {
    frame_index: u64, // TODO(andreas): having to hand-implement this mechanism here seems brittle, surely per-frame drawable data will happen more often in the future.
    entries: Vec<PointCloudDrawable>,
}

/// A point cloud drawing operation.
/// Expected to be recreated every frame.
#[derive(Clone)]
pub struct PointCloudDrawData {
    bind_group_all_points: Option<GpuBindGroup>,
    bind_group_all_points_outline_mask: Option<GpuBindGroup>,
    batches: Vec<PointCloudBatch>,

    /// Appended during drawable collection so each view retains its own sorted lookup texture.
    /// Entries from the previous frame are discarded when this draw data is reused.
    drawables: Arc<Mutex<PointCloudDrawables>>,
}

impl DrawData for PointCloudDrawData {
    type Renderer = PointCloudRenderer;

    fn collect_drawables(
        &self,
        view_info: &DrawableCollectionViewInfo,
        collector: &mut DrawableCollector<'_>,
    ) {
        // TODO(#1611): point clouds don't sort against other primitives yet.
        // TODO(#1025, #4787): Better handling of 2D objects, use per-2D layer sorting instead of depth offsets.

        let lookup_bind_group_layout = collector
            .render_ctx()
            .renderer::<PointCloudRenderer>()
            .bind_group_layout_lookup;

        for (batch_index, batch) in self.batches.iter().enumerate() {
            // TODO(andreas, emilk): Sort points on the GPU immediately before drawing instead of creating
            // a CPU-sorted lookup texture during drawable collection.
            let lookup_bind_group = if let Some(sort) = &batch.sort {
                let render_ctx = collector.render_ctx();
                let Some(lookup_texture) =
                    build_back_to_front_lookup_texture(render_ctx, sort, view_info)
                else {
                    // TODO(andreas): propagate error.
                    continue;
                };

                Some(render_ctx.gpu_resources.bind_groups.alloc(
                    &render_ctx.device,
                    &render_ctx.gpu_resources,
                    &BindGroupDesc {
                        label: "PointCloudDrawData::lookup_bind_group".into(),
                        entries: smallvec![BindGroupEntry::DefaultTextureView(
                            lookup_texture.handle,
                        )],
                        layout: lookup_bind_group_layout,
                    },
                ))
            } else {
                None
            };

            let drawable_index = {
                let mut drawables = self.drawables.lock();

                // Discard outdated drawables first.
                let frame_index = collector.render_ctx().active_frame.frame_index;
                if drawables.frame_index != frame_index {
                    drawables.frame_index = frame_index;
                    drawables.entries.clear();
                }

                drawables.entries.push(PointCloudDrawable {
                    batch_index,
                    lookup_bind_group,
                });
                drawables.entries.len() - 1
            };

            collector.add_drawable(
                batch.active_phases,
                DrawDataDrawable::from_world_position(
                    view_info,
                    batch.center_world_position.into(),
                    drawable_index as _,
                ),
            );
        }
    }
}

/// Data that is valid for a batch of point cloud points.
pub struct PointCloudBatchInfo {
    pub label: Label,

    /// Transformation applies to point positions
    ///
    /// TODO(andreas): We don't apply scaling to the radius yet. Need to pass a scaling factor like this in
    /// `let scale = Mat3::from(world_from_obj).determinant().abs().cbrt()`
    pub world_from_obj: glam::Affine3A,

    /// Additional properties of this point cloud batch.
    pub flags: PointCloudBatchFlags,

    /// Number of points covered by this batch.
    ///
    /// The batch will start with the next point after the one the previous batch ended with.
    pub point_count: u32,

    /// Object-space bounds used to place the batch in the draw-phase distance ordering.
    pub object_space_bounding_box: macaw::BoundingBox,

    /// Optional outline mask setting for the entire batch.
    pub overall_outline_mask_ids: OutlineMaskPreference,

    /// Defines an outline mask for an individual vertex ranges.
    ///
    /// Vertex ranges are relative within the current batch.
    ///
    /// Having many of these individual outline masks can be slow as they require each their own uniform buffer & draw call.
    /// This feature is meant for a limited number of "extra selections"
    /// If an overall mask is defined as well, the per-point-range masks is overwriting the overall mask.
    pub additional_outline_mask_ids_vertex_ranges: Vec<(Range<u32>, OutlineMaskPreference)>,

    /// Picking object id that applies for the entire batch.
    pub picking_object_id: PickingLayerObjectId,

    /// Depth offset applied after projection.
    pub depth_offset: DepthOffset,

    /// Object-space positions of the points in this batch, used to sort transparent batches
    /// back-to-front on the CPU.
    ///
    /// Only populated for batches with [`PointCloudBatchFlags::FLAG_PREMULTIPLIED_ALPHA`];
    /// filled in automatically by [`crate::PointCloudBuilder`].
    pub sort_positions: Option<Vec<glam::Vec3>>,

    /// Caller-owned cache for the back-to-front ordering, persisted across frames to seed the next
    /// sort for each view.
    ///
    /// The caller owns the storage and its invalidation. `None` disables the optimization, sorting
    /// from scratch every frame. Only relevant for transparent batches.
    pub sort_order_cache: Option<PointCloudSortOrderCache>,
}

impl Default for PointCloudBatchInfo {
    #[inline]
    fn default() -> Self {
        Self {
            label: Label::default(),
            world_from_obj: glam::Affine3A::IDENTITY,
            flags: PointCloudBatchFlags::FLAG_ENABLE_SHADING,
            point_count: 0,
            object_space_bounding_box: macaw::BoundingBox::nothing(),
            overall_outline_mask_ids: OutlineMaskPreference::NONE,
            additional_outline_mask_ids_vertex_ranges: Vec::new(),
            picking_object_id: Default::default(),
            depth_offset: 0,
            sort_positions: None,
            sort_order_cache: None,
        }
    }
}

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum PointCloudDrawDataError {
    #[error("Failed to transfer data to the GPU: {0}")]
    FailedTransferringDataToGpu(#[from] crate::allocator::CpuWriteGpuReadError),
}

impl PointCloudDrawData {
    /// Transforms and uploads point cloud data to be consumed by gpu.
    ///
    /// Try to bundle all points into a single draw data instance whenever possible.
    /// Number of vertices and colors has to be equal.
    ///
    /// If no batches are passed, all points are assumed to be in a single batch with identity transform.
    pub fn new(builder: PointCloudBuilder<'_>) -> Result<Self, PointCloudDrawDataError> {
        re_tracing::profile_function!();

        let PointCloudBuilder {
            ctx,
            position_radius_buffer: vertices_buffer,
            color_buffer,
            picking_instance_ids_buffer,
            batches,
            radius_boost_in_ui_points_for_outlines,
        } = builder;

        let point_renderer = ctx.renderer::<PointCloudRenderer>();
        let batches = batches.as_slice();

        if vertices_buffer.is_empty() {
            return Ok(Self {
                bind_group_all_points: None,
                bind_group_all_points_outline_mask: None,
                batches: Vec::new(),
                drawables: Arc::new(Mutex::new(PointCloudDrawables::default())),
            });
        }

        let num_vertices = vertices_buffer.len();

        let fallback_batches = [PointCloudBatchInfo {
            label: "fallback_batches".into(),
            world_from_obj: glam::Affine3A::IDENTITY,
            flags: PointCloudBatchFlags::empty(),
            point_count: num_vertices as _,
            object_space_bounding_box: macaw::BoundingBox::nothing(),
            overall_outline_mask_ids: OutlineMaskPreference::NONE,
            additional_outline_mask_ids_vertex_ranges: Vec::new(),
            picking_object_id: Default::default(),
            depth_offset: 0,
            sort_positions: None,
            sort_order_cache: None,
        }];
        let batches = if batches.is_empty() {
            &fallback_batches
        } else {
            batches
        };

        let position_data_texture = vertices_buffer.finish(
            wgpu::TextureFormat::Rgba32Float,
            "PointCloudDrawData::position_data_texture",
        )?;
        let color_texture = color_buffer.finish(
            wgpu::TextureFormat::Rgba8UnormSrgb,
            "PointCloudDrawData::color_texture",
        )?;
        let picking_instance_id_texture = picking_instance_ids_buffer.finish(
            wgpu::TextureFormat::Rg32Uint,
            "PointCloudDrawData::picking_instance_id_texture",
        )?;

        let draw_data_uniform_buffer_bindings = create_and_fill_uniform_buffer_batch(
            ctx,
            "PointCloudDrawData::DrawDataUniformBuffer".into(),
            [
                gpu_data::DrawDataUniformBuffer {
                    radius_boost_in_ui_points: 0.0.into(),
                    end_padding: Default::default(),
                },
                gpu_data::DrawDataUniformBuffer {
                    radius_boost_in_ui_points: radius_boost_in_ui_points_for_outlines.into(),
                    end_padding: Default::default(),
                },
            ]
            .into_iter(),
        );
        let (draw_data_uniform_buffer_bindings_normal, draw_data_uniform_buffer_bindings_outline) =
            draw_data_uniform_buffer_bindings
                .into_iter()
                .collect_tuple()
                .unwrap();

        let mk_bind_group = |label, draw_data_uniform_buffer_binding| {
            ctx.gpu_resources.bind_groups.alloc(
                &ctx.device,
                &ctx.gpu_resources,
                &BindGroupDesc {
                    label,
                    entries: smallvec![
                        BindGroupEntry::DefaultTextureView(position_data_texture.handle),
                        BindGroupEntry::DefaultTextureView(color_texture.handle),
                        BindGroupEntry::DefaultTextureView(picking_instance_id_texture.handle),
                        draw_data_uniform_buffer_binding,
                    ],
                    layout: point_renderer.bind_group_layout_all_points,
                },
            )
        };

        let bind_group_all_points = mk_bind_group(
            "PointCloudDrawData::bind_group_all_points".into(),
            draw_data_uniform_buffer_bindings_normal,
        );
        let bind_group_all_points_outline_mask = mk_bind_group(
            "PointCloudDrawData::bind_group_all_points_outline_mask".into(),
            draw_data_uniform_buffer_bindings_outline,
        );

        // Process batches
        let mut batches_internal = Vec::with_capacity(batches.len());
        {
            let mut first_point_index = 0;
            let uniform_buffer_bindings = create_and_fill_uniform_buffer_batch(
                ctx,
                "point batch uniform buffers".into(),
                batches.iter().map(|batch_info| {
                    let current_first_point_index = first_point_index;
                    first_point_index += batch_info.point_count;

                    let enable_index_lookup = batch_info
                        .sort_positions
                        .as_ref()
                        .is_some_and(|positions| !positions.is_empty());
                    let flags = batch_info.flags.bits()
                        | if enable_index_lookup {
                            PointCloudBatchFlags::FLAG_ENABLE_INDEX_LOOKUP.bits()
                        } else {
                            0
                        };

                    gpu_data::BatchUniformBuffer {
                        world_from_obj: batch_info.world_from_obj.into(),
                        flags,
                        outline_mask_ids: batch_info
                            .overall_outline_mask_ids
                            .0
                            .unwrap_or_default()
                            .into(),
                        picking_object_id: batch_info.picking_object_id,
                        depth_offset: batch_info.depth_offset as f32,
                        first_point_index: current_first_point_index,
                        _row_padding: 0,
                        end_padding: Default::default(),
                    }
                }),
            );

            // Generate additional "micro batches" for each point range that has a unique outline setting.
            // This is fairly costly if there's many, but easy and low-overhead if there's only few, which is usually what we expect!
            let mut uniform_buffer_bindings_mask_only_batches =
                create_and_fill_uniform_buffer_batch(
                    ctx,
                    "lines batch uniform buffers - mask only".into(),
                    batches
                        .iter()
                        .scan(0, |first_point_index, batch_info| {
                            let current_first_point_index = *first_point_index;
                            *first_point_index += batch_info.point_count;

                            // Masks never use sorting, so we don't need index lookup here.
                            let flags = batch_info
                                .flags
                                .difference(PointCloudBatchFlags::FLAG_ENABLE_INDEX_LOOKUP)
                                .bits();

                            Some(
                                batch_info
                                    .additional_outline_mask_ids_vertex_ranges
                                    .iter()
                                    .map(move |(_, mask)| gpu_data::BatchUniformBuffer {
                                        world_from_obj: batch_info.world_from_obj.into(),
                                        flags,
                                        outline_mask_ids: mask.0.unwrap_or_default().into(),
                                        picking_object_id: batch_info.picking_object_id,
                                        depth_offset: batch_info.depth_offset as f32,
                                        first_point_index: current_first_point_index,
                                        _row_padding: 0,
                                        end_padding: Default::default(),
                                    }),
                            )
                        })
                        .flatten()
                        .collect::<Vec<_>>()
                        .into_iter(),
                )
                .into_iter();

            let mut start_point_for_next_batch = 0;
            for (batch_info, uniform_buffer_binding) in
                std::iter::zip(batches, uniform_buffer_bindings)
            {
                let point_vertex_range_end = start_point_for_next_batch + batch_info.point_count;
                // Transparent batches are alpha-blended in the transparent phase, everything else
                // is drawn opaque (relying on alpha-to-coverage for edge anti-aliasing).
                let color_phase = if batch_info
                    .flags
                    .contains(PointCloudBatchFlags::FLAG_PREMULTIPLIED_ALPHA)
                {
                    DrawPhase::Transparent
                } else {
                    DrawPhase::Opaque
                };
                let mut active_phases = color_phase | DrawPhase::PickingLayer;
                // Does the entire batch participate in the outline mask phase?
                if batch_info.overall_outline_mask_ids.is_some() {
                    active_phases.insert(DrawPhase::OutlineMask);
                }

                // Transparent batches keep their world-space positions so they can be sorted
                // back-to-front every frame in `collect_drawables`.
                let sort = batch_info
                    .sort_positions
                    .as_ref()
                    .filter(|positions| !positions.is_empty())
                    .map(|obj_positions| TransparentSort {
                        object_positions: Arc::new(obj_positions.clone()),
                        object_from_world: batch_info.world_from_obj.inverse(),
                        sort_order_cache: batch_info.sort_order_cache.clone(),
                    });

                let object_position = if batch_info.object_space_bounding_box.is_finite()
                    && !batch_info.object_space_bounding_box.is_nothing()
                {
                    batch_info.object_space_bounding_box.center()
                } else {
                    glam::Vec3::ZERO
                };
                let center_world_position =
                    batch_info.world_from_obj.transform_point3(object_position);

                batches_internal.push(point_renderer.create_point_cloud_batch(
                    ctx,
                    batch_info.label.clone(),
                    uniform_buffer_binding,
                    start_point_for_next_batch..point_vertex_range_end,
                    center_world_position,
                    active_phases,
                    sort,
                ));

                for (range, _) in &batch_info.additional_outline_mask_ids_vertex_ranges {
                    let range = (range.start + start_point_for_next_batch)
                        ..(range.end + start_point_for_next_batch);
                    batches_internal.push(point_renderer.create_point_cloud_batch(
                        ctx,
                        format!("{:?} strip-only {:?}", batch_info.label, range).into(),
                        uniform_buffer_bindings_mask_only_batches.next().unwrap(),
                        range.clone(),
                        center_world_position,
                        enum_set![DrawPhase::OutlineMask],
                        None,
                    ));
                }

                start_point_for_next_batch = point_vertex_range_end;

                // Should happen only if the number of vertices was clamped.
                if start_point_for_next_batch >= num_vertices as u32 {
                    break;
                }
            }
        }

        Ok(Self {
            bind_group_all_points: Some(bind_group_all_points),
            bind_group_all_points_outline_mask: Some(bind_group_all_points_outline_mask),
            batches: batches_internal,
            drawables: Arc::new(Mutex::new(PointCloudDrawables::default())),
        })
    }
}

pub struct PointCloudRenderer {
    render_pipeline_color: GpuRenderPipelineHandle,
    render_pipeline_color_alpha_blended: GpuRenderPipelineHandle,
    render_pipeline_picking_layer: GpuRenderPipelineHandle,
    render_pipeline_outline_mask: GpuRenderPipelineHandle,
    bind_group_layout_all_points: GpuBindGroupLayoutHandle,
    bind_group_layout_batch: GpuBindGroupLayoutHandle,
    bind_group_layout_lookup: GpuBindGroupLayoutHandle,

    /// Shared by every point-cloud batch that does not use point-index redirection.
    dummy_lookup_bind_group: GpuBindGroup,
}

impl PointCloudRenderer {
    fn create_point_cloud_batch(
        &self,
        ctx: &RenderContext,
        label: Label,
        uniform_buffer_binding: BindGroupEntry,
        vertex_range: Range<u32>,
        center_world_position: glam::Vec3,
        active_phases: EnumSet<DrawPhase>,
        sort: Option<TransparentSort>,
    ) -> PointCloudBatch {
        // TODO(andreas): There should be only a single bindgroup with dynamic indices for all batches.
        //                  (each batch would then know which dynamic indices to use in the bindgroup)
        let bind_group = ctx.gpu_resources.bind_groups.alloc(
            &ctx.device,
            &ctx.gpu_resources,
            &BindGroupDesc {
                label,
                entries: smallvec![uniform_buffer_binding],
                layout: self.bind_group_layout_batch,
            },
        );

        PointCloudBatch {
            bind_group,
            vertex_range: (vertex_range.start * 6)..(vertex_range.end * 6),
            center_world_position,
            active_phases,
            sort,
        }
    }
}

impl Renderer for PointCloudRenderer {
    type RendererDrawData = PointCloudDrawData;

    fn create_renderer(ctx: &RenderContext) -> Self {
        re_tracing::profile_function!();

        let render_pipelines = &ctx.gpu_resources.render_pipelines;

        let bind_group_layout_all_points = ctx.gpu_resources.bind_group_layouts.get_or_create(
            &ctx.device,
            &BindGroupLayoutDesc {
                label: "PointCloudRenderer::bind_group_layout_all_points".into(),
                entries: vec![
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Uint,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: NonZeroU64::new(std::mem::size_of::<
                                gpu_data::DrawDataUniformBuffer,
                            >() as _),
                        },
                        count: None,
                    },
                ],
            },
        );

        let bind_group_layout_batch = ctx.gpu_resources.bind_group_layouts.get_or_create(
            &ctx.device,
            &BindGroupLayoutDesc {
                label: "PointCloudRenderer::bind_group_layout_batch".into(),
                entries: vec![wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: NonZeroU64::new(std::mem::size_of::<
                            gpu_data::BatchUniformBuffer,
                        >() as _),
                    },
                    count: None,
                }],
            },
        );
        let bind_group_layout_lookup = ctx.gpu_resources.bind_group_layouts.get_or_create(
            &ctx.device,
            &BindGroupLayoutDesc {
                label: "PointCloudRenderer::bind_group_layout_lookup".into(),
                entries: vec![wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                }],
            },
        );

        let dummy_lookup_bind_group = ctx.gpu_resources.bind_groups.alloc(
            &ctx.device,
            &ctx.gpu_resources,
            &BindGroupDesc {
                label: "PointCloudRenderer::dummy_lookup_bind_group".into(),
                entries: smallvec![BindGroupEntry::DefaultTextureView(
                    ctx.texture_manager_2d.zeroed_texture_uint().handle,
                )],
                layout: bind_group_layout_lookup,
            },
        );

        let pipeline_layout = ctx.gpu_resources.pipeline_layouts.get_or_create(
            ctx,
            &PipelineLayoutDesc {
                label: "PointCloudRenderer::pipeline_layout".into(),
                entries: vec![
                    ctx.global_bindings.layout,
                    bind_group_layout_all_points,
                    bind_group_layout_batch,
                    bind_group_layout_lookup,
                ],
            },
        );

        let shader_module_desc = include_shader_module!("../../shader/point_cloud.wgsl");
        let shader_module = ctx
            .gpu_resources
            .shader_modules
            .get_or_create(ctx, &shader_module_desc);

        let render_pipeline_desc_color = RenderPipelineDesc {
            label: "PointCloudRenderer::render_pipeline_color".into(),
            pipeline_layout,
            vertex_entrypoint: "vs_main".into(),
            vertex_handle: shader_module,
            fragment_entrypoint: "fs_main".into(),
            fragment_handle: shader_module,
            vertex_buffers: smallvec![],
            render_targets: smallvec![Some(ViewBuilder::MAIN_TARGET_ALPHA_TO_COVERAGE_COLOR_STATE)],
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: Some(ViewBuilder::MAIN_TARGET_DEFAULT_DEPTH_STATE),
            // We discard pixels to do the round cutout, therefore we need to calculate our own sampling mask.
            multisample: ViewBuilder::main_target_default_msaa_state(ctx.render_config(), true),
        };
        let render_pipeline_color =
            render_pipelines.get_or_create(ctx, &render_pipeline_desc_color);

        // Alpha-blended variant: premultiplied alpha blending of the shader's color & coverage,
        // used for batches that contain transparent points. Does not write depth and does not
        // rely on alpha-to-coverage MSAA.
        let render_pipeline_color_alpha_blended = render_pipelines.get_or_create(
            ctx,
            &RenderPipelineDesc {
                label: "PointCloudRenderer::render_pipeline_color_alpha_blended".into(),
                render_targets: smallvec![Some(wgpu::ColorTargetState {
                    format: ViewBuilder::MAIN_TARGET_COLOR_FORMAT,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                depth_stencil: Some(ViewBuilder::MAIN_TARGET_DEFAULT_DEPTH_STATE_NO_WRITE),
                multisample: ViewBuilder::main_target_default_msaa_state(
                    ctx.render_config(),
                    false,
                ),
                ..render_pipeline_desc_color.clone()
            },
        );
        let render_pipeline_picking_layer = render_pipelines.get_or_create(
            ctx,
            &RenderPipelineDesc {
                label: "PointCloudRenderer::render_pipeline_picking_layer".into(),
                fragment_entrypoint: "fs_main_picking_layer".into(),
                render_targets: smallvec![Some(PickingLayerProcessor::PICKING_LAYER_FORMAT.into())],
                depth_stencil: PickingLayerProcessor::PICKING_LAYER_DEPTH_STATE,
                multisample: PickingLayerProcessor::PICKING_LAYER_MSAA_STATE,
                ..render_pipeline_desc_color.clone()
            },
        );
        let render_pipeline_outline_mask = render_pipelines.get_or_create(
            ctx,
            &RenderPipelineDesc {
                label: "PointCloudRenderer::render_pipeline_outline_mask".into(),
                fragment_entrypoint: "fs_main_outline_mask".into(),
                render_targets: smallvec![Some(OutlineMaskProcessor::MASK_FORMAT.into())],
                depth_stencil: OutlineMaskProcessor::MASK_DEPTH_STATE,
                // Alpha to coverage doesn't work with the mask integer target.
                multisample: OutlineMaskProcessor::mask_default_msaa_state(ctx.device_caps().tier),
                ..render_pipeline_desc_color
            },
        );

        Self {
            render_pipeline_color,
            render_pipeline_color_alpha_blended,
            render_pipeline_picking_layer,
            render_pipeline_outline_mask,
            bind_group_layout_all_points,
            bind_group_layout_batch,
            bind_group_layout_lookup,
            dummy_lookup_bind_group,
        }
    }

    fn draw(
        &self,
        render_pipelines: &GpuRenderPipelinePoolAccessor<'_>,
        phase: DrawPhase,
        pass: &mut wgpu::RenderPass<'_>,
        draw_instructions: &[DrawInstruction<'_, Self::RendererDrawData>],
    ) -> Result<(), DrawError> {
        let pipeline_handle = match phase {
            DrawPhase::OutlineMask => self.render_pipeline_outline_mask,
            DrawPhase::Opaque => self.render_pipeline_color,
            DrawPhase::Transparent => self.render_pipeline_color_alpha_blended,
            DrawPhase::PickingLayer => self.render_pipeline_picking_layer,
            _ => unreachable!("We were called on a phase we weren't subscribed to: {phase:?}"),
        };
        let pipeline = render_pipelines.get(pipeline_handle)?;

        pass.set_pipeline(pipeline);

        for DrawInstruction {
            draw_data,
            drawables,
        } in draw_instructions
        {
            let bind_group_all_points = match phase {
                DrawPhase::OutlineMask => &draw_data.bind_group_all_points_outline_mask,
                DrawPhase::Opaque | DrawPhase::Transparent | DrawPhase::PickingLayer => {
                    &draw_data.bind_group_all_points
                }
                _ => unreachable!("We were called on a phase we weren't subscribed to: {phase:?}"),
            };
            let Some(bind_group_all_points) = bind_group_all_points else {
                re_log::debug_panic!(
                    "Point data bind group for draw phase {phase:?} was not set despite being submitted for drawing."
                );
                continue;
            };
            pass.set_bind_group(1, bind_group_all_points, &[]);

            let point_cloud_drawables = draw_data.drawables.lock();
            for drawable in *drawables {
                let point_cloud_drawable =
                    &point_cloud_drawables.entries[drawable.draw_data_payload as usize];
                let batch = &draw_data.batches[point_cloud_drawable.batch_index];

                let lookup_bind_group = point_cloud_drawable
                    .lookup_bind_group
                    .as_ref()
                    .unwrap_or(&self.dummy_lookup_bind_group);

                pass.set_bind_group(2, &batch.bind_group, &[]);
                pass.set_bind_group(3, lookup_bind_group, &[]);

                pass.draw(batch.vertex_range.clone(), 0..1);
            }
        }

        Ok(())
    }
}
