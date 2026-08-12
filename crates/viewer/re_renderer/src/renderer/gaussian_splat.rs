//! Renderer for 3D gaussian splats.
//!
//! How it works:
//! =================
//! Like the point cloud renderer, gaussians are rendered as procedurally spanned quads
//! (no vertex buffer), with per-gaussian data uploaded as data textures for WebGL compatibility.
//!
//! Unlike points, the quads are not round billboards: the vertex shader projects each gaussian's
//! 3D covariance (from its per-axis scales and rotation) to a 2D screen-space covariance
//! (EWA splatting), spans a quad along its eigenvectors, and the fragment shader evaluates the
//! gaussian falloff to get per-pixel alpha.
//!
//! Gaussians are inherently transparent, so they always render in [`DrawPhase::Transparent`]
//! with premultiplied-alpha blending, sorted back-to-front on the CPU each frame
//! (same mechanism as transparent point clouds).

use std::num::NonZeroU64;
use std::ops::Range;
use std::sync::Arc;

use enumset::{EnumSet, enum_set};
use parking_lot::Mutex;
use smallvec::smallvec;

use super::{DrawData, DrawError, RenderContext, Renderer};
use crate::allocator::create_and_fill_uniform_buffer_batch;
use crate::draw_phases::{
    DrawPhase, OutlineMaskProcessor, PickingLayerObjectId, PickingLayerProcessor,
};
use crate::renderer::{DrawDataDrawable, DrawInstruction, DrawableCollectionViewInfo};
use crate::transparent_sort::{
    SortOrderCache, SortedDrawable, SortedDrawables, TransparentSort,
    build_back_to_front_lookup_texture,
};
use crate::view_builder::ViewBuilder;
use crate::wgpu_resources::{
    BindGroupDesc, BindGroupEntry, BindGroupLayoutDesc, GpuBindGroup, GpuBindGroupLayoutHandle,
    GpuRenderPipelineHandle, GpuRenderPipelinePoolAccessor, PipelineLayoutDesc, RenderPipelineDesc,
};
use crate::{
    DrawableCollector, GaussianSplatBuilder, Label, OutlineMaskPreference, include_shader_module,
};

/// Set on a batch's `flags` to redirect gaussian indices through its back-to-front lookup texture.
/// Must match `FLAG_ENABLE_INDEX_LOOKUP` in `gaussian_splat.wgsl`.
const FLAG_ENABLE_INDEX_LOOKUP: u32 = 1;

pub mod gpu_data {
    use crate::draw_phases::PickingLayerObjectId;
    use crate::wgpu_buffer_types;

    // Don't use `wgsl_buffer_types` for the texel structs since this data doesn't go into a
    // buffer, so alignment rules don't apply like on buffers.

    /// Center position and the x-axis scale, one `Rgba32Float` texel.
    #[repr(C, packed)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct GaussianPositionScaleX {
        pub pos: glam::Vec3,

        /// Standard deviation along the (unrotated) x axis, in object units.
        pub scale_x: f32,
    }
    static_assertions::assert_eq_size!(GaussianPositionScaleX, glam::Vec4);

    /// Rotation as an `xyzw` quaternion, one `Rgba32Float` texel.
    // A plain array, not `glam::Vec4`: on SIMD targets `Vec4` is `#[repr(align(16))]`, which a
    // `#[repr(C, packed)]` struct is not allowed to contain.
    #[repr(C, packed)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct GaussianRotation {
        pub quat_xyzw: [f32; 4],
    }
    static_assertions::assert_eq_size!(GaussianRotation, glam::Vec4);

    /// The y & z axis scales, one `Rg32Float` texel.
    #[repr(C, packed)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct GaussianScaleYZ {
        pub scale_y: f32,
        pub scale_z: f32,
    }
    static_assertions::assert_eq_size!(GaussianScaleYZ, glam::Vec2);

    /// A single spherical harmonics coefficient, one `Rgba16Float` texel.
    ///
    /// Each gaussian occupies [`super::SH_TEXELS_PER_GAUSSIAN`] consecutive texels, one per
    /// coefficient (degrees 1 through 3, coefficient-major). The alpha channel is unused: it
    /// wastes an eighth of the texture, but keeps both the upload and the shader trivial.
    #[repr(C, packed)]
    #[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct GaussianShCoefficient {
        pub rgb_unused: [half::f16; 4],
    }
    static_assertions::assert_eq_size!(GaussianShCoefficient, u64);

    impl re_byte_size::SizeBytes for GaussianShCoefficient {
        // Plain-old-data, so nothing lives on the heap.
        const IS_POD: bool = true;

        #[inline]
        fn heap_size_bytes(&self) -> u64 {
            0
        }
    }

    impl GaussianShCoefficient {
        /// Widens an RGB coefficient, zeroing the unused fourth channel.
        #[inline]
        pub fn from_rgb([r, g, b]: [half::f16; 3]) -> Self {
            Self {
                rgb_unused: [r, g, b, half::f16::ZERO],
            }
        }
    }

    /// Uniform buffer that changes for every batch of gaussians.
    #[repr(C)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct BatchUniformBuffer {
        pub world_from_obj: wgpu_buffer_types::Mat4,

        /// Inverse of `world_from_obj`, used to evaluate the spherical harmonics in object space.
        pub obj_from_world: wgpu_buffer_types::Mat4,

        // Keep this field order in sync with the WGSL `BatchUniformBuffer`.
        pub flags: u32, // See the `FLAG_*` constants above.
        pub first_gaussian_index: u32,

        /// How many coefficients this batch stores (and evaluates) per gaussian.
        pub sh_num_coefficients: u32,

        /// Texel offset of this batch's region in the shared SH texture.
        pub sh_first_texel: u32,

        pub outline_mask_ids: wgpu_buffer_types::UVec2,
        pub picking_object_id: PickingLayerObjectId,

        pub end_padding: [wgpu_buffer_types::PaddingRow; 16 - 10],
    }
}

bitflags::bitflags! {
    /// Caller-settable property flags for a gaussian batch.
    ///
    /// Needs to be kept in sync with `gaussian_splat.wgsl`.
    #[repr(C)]
    #[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct GaussianSplatBatchFlags : u32 {
        /// If set, the batch has spherical harmonics coefficients for view-dependent color.
        ///
        /// `0b0001` is reserved for [`FLAG_ENABLE_INDEX_LOOKUP`], which is set internally.
        const FLAG_HAS_SH_COEFFICIENTS = 0b0010;
    }
}

/// Number of `Rgba16Float` texels a gaussian occupies in the SH data texture at the highest
/// spherical harmonics degree: one per coefficient of degrees 1 through 3.
///
/// A batch at a lower degree stores (and reads) correspondingly fewer per gaussian, so this is
/// an upper bound rather than a fixed stride -- see [`GaussianSplatBatchInfo::sh_num_coefficients`].
pub const SH_TEXELS_PER_GAUSSIAN: usize = 15;

/// Number of quad vertices emitted per gaussian (two triangles).
const VERTICES_PER_GAUSSIAN: u32 = 6;

/// Internal, ready to draw representation of [`GaussianSplatBatchInfo`]
#[derive(Clone)]
struct GaussianSplatBatch {
    bind_group: GpuBindGroup,
    vertex_range: Range<u32>,
    active_phases: EnumSet<DrawPhase>,

    /// World-space center of the batch, used as its inter-primitive draw-order sort key.
    center_world_position: glam::Vec3,

    /// Set for batches that should be painted back-to-front (all except outline-only micro batches).
    sort: Option<TransparentSort>,
}

/// A gaussian splat drawing operation.
/// Expected to be recreated every frame.
#[derive(Clone)]
pub struct GaussianSplatDrawData {
    bind_group_all_gaussians: Option<GpuBindGroup>,
    batches: Vec<GaussianSplatBatch>,

    /// Appended during drawable collection so each view retains its own sorted lookup texture.
    /// Entries from the previous frame are discarded when this draw data is reused.
    drawables: Arc<Mutex<SortedDrawables>>,
}

impl DrawData for GaussianSplatDrawData {
    type Renderer = GaussianSplatRenderer;

    fn collect_drawables(
        &self,
        view_info: &DrawableCollectionViewInfo,
        collector: &mut DrawableCollector<'_>,
    ) {
        // TODO(#1611): gaussians don't sort against other primitives yet.

        let lookup_bind_group_layout = collector
            .render_ctx()
            .renderer::<GaussianSplatRenderer>()
            .bind_group_layout_lookup;

        for (batch_index, batch) in self.batches.iter().enumerate() {
            let lookup_bind_group = if let Some(sort) = &batch.sort {
                let render_ctx = collector.render_ctx();
                let Some(lookup_texture) =
                    build_back_to_front_lookup_texture(render_ctx, sort, view_info)
                else {
                    continue;
                };
                Some(render_ctx.gpu_resources.bind_groups.alloc(
                    &render_ctx.device,
                    &render_ctx.gpu_resources,
                    &BindGroupDesc {
                        label: "GaussianSplatDrawData::lookup_bind_group".into(),
                        entries: smallvec![BindGroupEntry::DefaultTextureView(
                            lookup_texture.handle
                        )],
                        layout: lookup_bind_group_layout,
                    },
                ))
            } else {
                None
            };

            let frame_index = collector.render_ctx().active_frame.frame_index;
            let drawable_index = self.drawables.lock().push_for_frame(
                frame_index,
                SortedDrawable {
                    batch_index,
                    lookup_bind_group,
                },
            );

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

/// Data that is valid for a batch of gaussians.
pub struct GaussianSplatBatchInfo {
    pub label: Label,

    /// Transformation applied to the gaussians (both centers and covariances).
    pub world_from_obj: glam::Affine3A,

    /// Additional properties of this batch.
    pub flags: GaussianSplatBatchFlags,

    /// How many coefficients this batch stores per gaussian: 0, 3, 8 or 15, for spherical
    /// harmonics degrees 0 through 3 respectively.
    ///
    /// Set by [`crate::GaussianSplatBuilder`] from the value passed to `add_gaussians`; lower
    /// values mean less to upload and less for the vertex shader to fetch and evaluate.
    pub sh_num_coefficients: u32,

    /// Number of gaussians covered by this batch.
    ///
    /// The batch will start with the next gaussian after the one the previous batch ended with.
    pub gaussian_count: u32,

    /// Object-space bounding box of the gaussian centers in this batch.
    ///
    /// [`macaw::BoundingBox::nothing`] means "unknown": the bounds are then computed from
    /// [`Self::sort_positions`] instead.
    pub object_space_bounding_box: macaw::BoundingBox,

    /// Optional outline mask setting for the entire batch.
    pub overall_outline_mask_ids: OutlineMaskPreference,

    /// Defines an outline mask for individual gaussian ranges (relative to this batch).
    ///
    /// Having many of these can be slow as they require their own uniform buffer & draw call each.
    /// This feature is meant for a limited number of "extra selections".
    /// If an overall mask is defined as well, the per-range masks overwrite the overall mask.
    pub additional_outline_mask_ids_vertex_ranges: Vec<(Range<u32>, OutlineMaskPreference)>,

    /// Picking object id that applies for the entire batch.
    pub picking_object_id: PickingLayerObjectId,

    /// Object-space centers of the gaussians in this batch, used to sort back-to-front
    /// on the CPU. Filled in automatically by [`crate::GaussianSplatBuilder`].
    pub sort_positions: Option<Vec<glam::Vec3>>,

    /// Caller-owned cache for the back-to-front ordering, persisted across frames to seed the next
    /// sort for each view (a big speed-up).
    ///
    /// The caller owns the storage (and thus its lifetime/invalidation). `None` disables the
    /// optimization, sorting from scratch every frame.
    pub sort_order_cache: Option<SortOrderCache>,
}

impl Default for GaussianSplatBatchInfo {
    #[inline]
    fn default() -> Self {
        Self {
            label: Label::default(),
            world_from_obj: glam::Affine3A::IDENTITY,
            flags: GaussianSplatBatchFlags::empty(),
            sh_num_coefficients: 0,
            gaussian_count: 0,
            object_space_bounding_box: macaw::BoundingBox::nothing(),
            overall_outline_mask_ids: OutlineMaskPreference::NONE,
            additional_outline_mask_ids_vertex_ranges: Vec::new(),
            picking_object_id: Default::default(),
            sort_positions: None,
            sort_order_cache: None,
        }
    }
}

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum GaussianSplatDrawDataError {
    #[error("Failed to transfer data to the GPU: {0}")]
    FailedTransferringDataToGpu(#[from] crate::allocator::CpuWriteGpuReadError),
}

impl GaussianSplatDrawData {
    /// Transforms and uploads gaussian data to be consumed by the GPU.
    ///
    /// Try to bundle all gaussians into a single draw data instance whenever possible.
    pub fn new(builder: GaussianSplatBuilder<'_>) -> Result<Self, GaussianSplatDrawDataError> {
        re_tracing::profile_function!();

        let GaussianSplatBuilder {
            ctx,
            position_scale_x_buffer,
            rotation_buffer,
            scale_yz_buffer,
            sh_buffer,
            color_buffer,
            picking_instance_ids_buffer,
            batches,
        } = builder;

        let renderer = ctx.renderer::<GaussianSplatRenderer>();
        let batches = batches.as_slice();

        if position_scale_x_buffer.is_empty() {
            return Ok(Self {
                bind_group_all_gaussians: None,
                batches: Vec::new(),
                drawables: Arc::new(Mutex::new(SortedDrawables::default())),
            });
        }

        let num_gaussians = position_scale_x_buffer.len();

        let fallback_batches = [GaussianSplatBatchInfo {
            label: "fallback_batches".into(),
            gaussian_count: num_gaussians as _,
            ..Default::default()
        }];
        let batches = if batches.is_empty() {
            &fallback_batches
        } else {
            batches
        };

        let position_scale_x_texture = position_scale_x_buffer.finish(
            wgpu::TextureFormat::Rgba32Float,
            "GaussianSplatDrawData::position_scale_x_texture",
        )?;
        let rotation_texture = rotation_buffer.finish(
            wgpu::TextureFormat::Rgba32Float,
            "GaussianSplatDrawData::rotation_texture",
        )?;
        let scale_yz_texture = scale_yz_buffer.finish(
            wgpu::TextureFormat::Rg32Float,
            "GaussianSplatDrawData::scale_yz_texture",
        )?;
        let sh_texture_handle = if sh_buffer.is_empty() {
            // No batch has spherical harmonics, so nothing ever samples this texture -- but it
            // still has to be a valid, bindable texture.
            ctx.texture_manager_2d.zeroed_texture_float().handle
        } else {
            sh_buffer
                .finish(
                    wgpu::TextureFormat::Rgba16Float,
                    "GaussianSplatDrawData::sh_texture",
                )?
                .handle
        };
        let color_texture = color_buffer.finish(
            wgpu::TextureFormat::Rgba8UnormSrgb,
            "GaussianSplatDrawData::color_texture",
        )?;
        let picking_instance_id_texture = picking_instance_ids_buffer.finish(
            wgpu::TextureFormat::Rg32Uint,
            "GaussianSplatDrawData::picking_instance_id_texture",
        )?;

        let bind_group_all_gaussians = ctx.gpu_resources.bind_groups.alloc(
            &ctx.device,
            &ctx.gpu_resources,
            &BindGroupDesc {
                label: "GaussianSplatDrawData::bind_group_all_gaussians".into(),
                entries: smallvec![
                    BindGroupEntry::DefaultTextureView(position_scale_x_texture.handle),
                    BindGroupEntry::DefaultTextureView(rotation_texture.handle),
                    BindGroupEntry::DefaultTextureView(scale_yz_texture.handle),
                    BindGroupEntry::DefaultTextureView(sh_texture_handle),
                    BindGroupEntry::DefaultTextureView(color_texture.handle),
                    BindGroupEntry::DefaultTextureView(picking_instance_id_texture.handle),
                ],
                layout: renderer.bind_group_layout_all_gaussians,
            },
        );

        // Process batches
        let mut batches_internal = Vec::with_capacity(batches.len());
        {
            let make_batch_uniform_buffer =
                |batch_info: &GaussianSplatBatchInfo,
                 outline_mask_ids: OutlineMaskPreference,
                 flags: u32,
                 first_gaussian_index: u32,
                 sh_first_texel: u32| {
                    gpu_data::BatchUniformBuffer {
                        world_from_obj: batch_info.world_from_obj.into(),
                        obj_from_world: batch_info.world_from_obj.inverse().into(),
                        flags: flags | batch_info.flags.bits(),
                        first_gaussian_index,
                        sh_num_coefficients: batch_info.sh_num_coefficients,
                        sh_first_texel,
                        outline_mask_ids: outline_mask_ids.0.unwrap_or_default().into(),
                        picking_object_id: batch_info.picking_object_id,

                        end_padding: Default::default(),
                    }
                };

            let batch_is_sorted = |batch_info: &GaussianSplatBatchInfo| {
                batch_info
                    .sort_positions
                    .as_ref()
                    .is_some_and(|p| !p.is_empty())
            };

            // The SH texture holds one region per batch that has coefficients, laid out
            // in batch order, so their offsets accumulate alongside the gaussian indices.
            let batch_offsets = batches
                .iter()
                .scan(
                    (0, 0),
                    |(first_gaussian_index, first_sh_texel), batch_info| {
                        let offsets = (*first_gaussian_index, *first_sh_texel);
                        *first_gaussian_index += batch_info.gaussian_count;
                        *first_sh_texel +=
                            batch_info.gaussian_count * batch_info.sh_num_coefficients;
                        Some(offsets)
                    },
                )
                .collect::<Vec<_>>();

            let uniform_buffer_bindings = create_and_fill_uniform_buffer_batch(
                ctx,
                "gaussian batch uniform buffers".into(),
                std::iter::zip(batches, &batch_offsets)
                    .map(|(batch_info, &(first_gaussian_index, sh_first_texel))| {
                        let flags = if batch_is_sorted(batch_info) {
                            FLAG_ENABLE_INDEX_LOOKUP
                        } else {
                            0
                        };
                        make_batch_uniform_buffer(
                            batch_info,
                            batch_info.overall_outline_mask_ids,
                            flags,
                            first_gaussian_index,
                            sh_first_texel,
                        )
                    })
                    .collect::<Vec<_>>()
                    .into_iter(),
            );

            // Generate additional "micro batches" for each gaussian range with a unique outline
            // setting (used e.g. for hover highlights of individual instances).
            let mut uniform_buffer_bindings_mask_only_batches =
                create_and_fill_uniform_buffer_batch(
                    ctx,
                    "gaussian batch uniform buffers - mask only".into(),
                    std::iter::zip(batches, &batch_offsets)
                        .flat_map(|(batch_info, &(first_gaussian_index, sh_first_texel))| {
                            batch_info
                                .additional_outline_mask_ids_vertex_ranges
                                .iter()
                                .map(move |(_, mask)| {
                                    // These micro batches index into the same gaussian & SH
                                    // regions as the batch they belong to, so they need its
                                    // offsets. They never use the sorted lookup though: their
                                    // ranges are in unsorted instance order.
                                    make_batch_uniform_buffer(
                                        batch_info,
                                        *mask,
                                        0,
                                        first_gaussian_index,
                                        sh_first_texel,
                                    )
                                })
                        })
                        .collect::<Vec<_>>()
                        .into_iter(),
                )
                .into_iter();

            let mut start_gaussian_for_next_batch = 0;
            for (batch_info, uniform_buffer_binding) in
                std::iter::zip(batches, uniform_buffer_bindings)
            {
                re_tracing::profile_scope!("batch");
                let gaussian_range_end = start_gaussian_for_next_batch + batch_info.gaussian_count;

                let mut active_phases = DrawPhase::Transparent | DrawPhase::PickingLayer;
                if batch_info.overall_outline_mask_ids.is_some() {
                    active_phases.insert(DrawPhase::OutlineMask);
                }

                // World-space center of the batch, used as its inter-primitive sort key.
                // Prefer the bounds supplied by the caller; otherwise derive them from the centers.
                let object_bbox = if batch_info.object_space_bounding_box.is_finite()
                    && batch_info.object_space_bounding_box.is_something()
                {
                    batch_info.object_space_bounding_box
                } else {
                    batch_info
                        .sort_positions
                        .as_ref()
                        .map(|p| crate::util::bounding_box_from_points(p.iter().copied()))
                        .unwrap_or_else(macaw::BoundingBox::nothing)
                };
                let object_center = if object_bbox.is_finite() {
                    object_bbox.center()
                } else {
                    glam::Vec3::ZERO
                };
                let center_world_position =
                    batch_info.world_from_obj.transform_point3(object_center);

                // Keep the object-space centers around so the batch can be sorted back-to-front
                // every frame in `collect_drawables`.
                let sort =
                    batch_info
                        .sort_positions
                        .as_ref()
                        .map(|obj_positions| TransparentSort {
                            object_positions: Arc::new(obj_positions.clone()),
                            object_from_world: batch_info.world_from_obj.inverse(),
                            sort_order_cache: batch_info.sort_order_cache.clone(),
                        });

                batches_internal.push(renderer.create_gaussian_splat_batch(
                    ctx,
                    batch_info.label.clone(),
                    uniform_buffer_binding,
                    start_gaussian_for_next_batch..gaussian_range_end,
                    active_phases,
                    center_world_position,
                    sort,
                ));

                for (range, _) in &batch_info.additional_outline_mask_ids_vertex_ranges {
                    let range = (range.start + start_gaussian_for_next_batch)
                        ..(range.end + start_gaussian_for_next_batch);
                    batches_internal.push(renderer.create_gaussian_splat_batch(
                        ctx,
                        format!("{:?} outline-only {:?}", batch_info.label, range).into(),
                        uniform_buffer_bindings_mask_only_batches.next().unwrap(),
                        range.clone(),
                        enum_set![DrawPhase::OutlineMask],
                        center_world_position,
                        None,
                    ));
                }

                start_gaussian_for_next_batch = gaussian_range_end;

                // Should happen only if the number of gaussians was clamped.
                if start_gaussian_for_next_batch >= num_gaussians as u32 {
                    break;
                }
            }
        }

        Ok(Self {
            bind_group_all_gaussians: Some(bind_group_all_gaussians),
            batches: batches_internal,
            drawables: Arc::new(Mutex::new(SortedDrawables::default())),
        })
    }
}

pub struct GaussianSplatRenderer {
    render_pipeline_color: GpuRenderPipelineHandle,
    render_pipeline_picking_layer: GpuRenderPipelineHandle,
    render_pipeline_outline_mask: GpuRenderPipelineHandle,
    bind_group_layout_all_gaussians: GpuBindGroupLayoutHandle,
    bind_group_layout_batch: GpuBindGroupLayoutHandle,
    bind_group_layout_lookup: GpuBindGroupLayoutHandle,

    /// Bound in place of the per-view back-to-front lookup texture for batches that aren't sorted.
    dummy_lookup_bind_group: GpuBindGroup,
}

impl GaussianSplatRenderer {
    fn create_gaussian_splat_batch(
        &self,
        ctx: &RenderContext,
        label: Label,
        uniform_buffer_binding: BindGroupEntry,
        gaussian_range: Range<u32>,
        active_phases: EnumSet<DrawPhase>,
        center_world_position: glam::Vec3,
        sort: Option<TransparentSort>,
    ) -> GaussianSplatBatch {
        let bind_group = ctx.gpu_resources.bind_groups.alloc(
            &ctx.device,
            &ctx.gpu_resources,
            &BindGroupDesc {
                label,
                entries: smallvec![uniform_buffer_binding],
                layout: self.bind_group_layout_batch,
            },
        );

        GaussianSplatBatch {
            bind_group,
            vertex_range: (gaussian_range.start * VERTICES_PER_GAUSSIAN)
                ..(gaussian_range.end * VERTICES_PER_GAUSSIAN),
            active_phases,
            center_world_position,
            sort,
        }
    }
}

impl Renderer for GaussianSplatRenderer {
    type RendererDrawData = GaussianSplatDrawData;

    fn create_renderer(ctx: &RenderContext) -> Self {
        re_tracing::profile_function!();

        let render_pipelines = &ctx.gpu_resources.render_pipelines;

        let float_texture_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };

        let bind_group_layout_all_gaussians = ctx.gpu_resources.bind_group_layouts.get_or_create(
            &ctx.device,
            &BindGroupLayoutDesc {
                label: "GaussianSplatRenderer::bind_group_layout_all_gaussians".into(),
                entries: vec![
                    float_texture_entry(0), // position + scale.x
                    float_texture_entry(1), // rotation
                    float_texture_entry(2), // scale.yz
                    float_texture_entry(3), // spherical harmonics coefficients
                    float_texture_entry(4), // color
                    wgpu::BindGroupLayoutEntry {
                        binding: 5,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Uint,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            },
        );

        let bind_group_layout_batch = ctx.gpu_resources.bind_group_layouts.get_or_create(
            &ctx.device,
            &BindGroupLayoutDesc {
                label: "GaussianSplatRenderer::bind_group_layout_batch".into(),
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
                label: "GaussianSplatRenderer::bind_group_layout_lookup".into(),
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
                label: "GaussianSplatRenderer::dummy_lookup_bind_group".into(),
                entries: smallvec![BindGroupEntry::DefaultTextureView(
                    ctx.texture_manager_2d.zeroed_texture_uint().handle,
                )],
                layout: bind_group_layout_lookup,
            },
        );

        let pipeline_layout = ctx.gpu_resources.pipeline_layouts.get_or_create(
            ctx,
            &PipelineLayoutDesc {
                label: "GaussianSplatRenderer::pipeline_layout".into(),
                entries: vec![
                    ctx.global_bindings.layout,
                    bind_group_layout_all_gaussians,
                    bind_group_layout_batch,
                    bind_group_layout_lookup,
                ],
            },
        );

        let shader_module_desc = include_shader_module!("../../shader/gaussian_splat.wgsl");
        let shader_module = ctx
            .gpu_resources
            .shader_modules
            .get_or_create(ctx, &shader_module_desc);

        // Gaussians are always transparent: premultiplied-alpha blending, no depth write
        // (but depth-tested against opaque geometry), no alpha-to-coverage.
        let render_pipeline_desc_color = RenderPipelineDesc {
            label: "GaussianSplatRenderer::render_pipeline_color".into(),
            pipeline_layout,
            vertex_entrypoint: "vs_main".into(),
            vertex_handle: shader_module,
            fragment_entrypoint: "fs_main".into(),
            fragment_handle: shader_module,
            vertex_buffers: smallvec![],
            render_targets: smallvec![Some(wgpu::ColorTargetState {
                format: ViewBuilder::MAIN_TARGET_COLOR_FORMAT,
                blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: Some(ViewBuilder::MAIN_TARGET_DEFAULT_DEPTH_STATE_NO_WRITE),
            multisample: ViewBuilder::main_target_default_msaa_state(ctx.render_config(), false),
        };
        let render_pipeline_color =
            render_pipelines.get_or_create(ctx, &render_pipeline_desc_color);

        let render_pipeline_picking_layer = render_pipelines.get_or_create(
            ctx,
            &RenderPipelineDesc {
                label: "GaussianSplatRenderer::render_pipeline_picking_layer".into(),
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
                label: "GaussianSplatRenderer::render_pipeline_outline_mask".into(),
                fragment_entrypoint: "fs_main_outline_mask".into(),
                render_targets: smallvec![Some(OutlineMaskProcessor::MASK_FORMAT.into())],
                depth_stencil: OutlineMaskProcessor::MASK_DEPTH_STATE,
                multisample: OutlineMaskProcessor::mask_default_msaa_state(ctx.device_caps().tier),
                ..render_pipeline_desc_color
            },
        );

        Self {
            render_pipeline_color,
            render_pipeline_picking_layer,
            render_pipeline_outline_mask,
            bind_group_layout_all_gaussians,
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
            DrawPhase::Transparent => self.render_pipeline_color,
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
            let Some(bind_group_all_gaussians) = &draw_data.bind_group_all_gaussians else {
                re_log::debug_panic!(
                    "Gaussian data bind group was not set despite being submitted for drawing."
                );
                continue;
            };
            pass.set_bind_group(1, bind_group_all_gaussians, &[]);

            let gaussian_drawables = draw_data.drawables.lock();
            for drawable in *drawables {
                let gaussian_drawable = gaussian_drawables.get(drawable.draw_data_payload as usize);
                let batch = &draw_data.batches[gaussian_drawable.batch_index];

                // The color pass is drawn back-to-front via this view's per-frame lookup texture
                // (built in `collect_drawables`); the shader redirects each gaussian through it.
                // Batches without a lookup texture draw in buffer order via the dummy binding.
                let lookup_bind_group = gaussian_drawable
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view_builder::{
        Projection, TargetConfiguration, ViewBuilder, ViewPickingConfiguration,
    };
    use crate::{
        PickingLayerInstanceId, PickingLayerProcessor, RectInt, Rgba, Rgba32Unmul, ViewBuilderId,
        draw_phases::PickingLayerObjectId,
    };

    /// A single opaque gaussian at the origin should write its object- and instance-id into the
    /// picking layer, and reading back the center pixel should return exactly those ids.
    ///
    /// In particular this guards the `BatchUniformBuffer` layout: the WGSL and Rust struct fields
    /// must line up, or the shader reads the picking id from padding (always zero).
    #[test]
    fn picking_layer_reports_object_and_instance_ids() {
        let mut ctx = RenderContext::new_test();

        let object_id = PickingLayerObjectId(0x1234_5678_9abc_def0);
        let instance_id = PickingLayerInstanceId(7);
        let resolution = [64_u32, 64];
        let readback_identifier = 42;

        ctx.begin_frame();

        let mut view_builder = ViewBuilder::new(
            &ctx,
            TargetConfiguration {
                name: "gaussian_picking".into(),
                resolution_in_pixel: resolution,
                view_from_world: macaw::IsoTransform::look_at_rh(
                    glam::Vec3::new(0.0, 0.0, 3.0),
                    glam::Vec3::ZERO,
                    glam::Vec3::Y,
                )
                .expect("valid camera"),
                projection_from_view: Projection::Perspective {
                    vertical_fov: 50.0_f32.to_radians(),
                    near_plane_distance: 0.01,
                    aspect_ratio: 1.0,
                },
                picking_config: Some(ViewPickingConfiguration {
                    picking_rect: RectInt {
                        min: glam::IVec2::ZERO,
                        extent: glam::UVec2::new(resolution[0], resolution[1]),
                    },
                    readback_identifier,
                    show_debug_view: false,
                }),
                ..Default::default()
            },
            ViewBuilderId::new(0),
        )
        .expect("failed to create view builder");

        let mut builder = GaussianSplatBuilder::new(&ctx);
        builder
            .batch("test")
            .picking_object_id(object_id)
            .add_gaussians(
                &[glam::Vec3::ZERO],
                &[glam::Vec3::splat(0.5)],
                &[glam::Quat::IDENTITY],
                &[Rgba32Unmul::WHITE],
                &[],
                0,
                &[instance_id],
            );
        view_builder.queue_draw(&ctx, builder.into_draw_data().expect("draw data"));

        let command_buffer = view_builder.draw(&ctx, Rgba::BLACK).expect("draw");
        ctx.before_submit();
        ctx.queue.submit([command_buffer]);
        ctx.device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(std::time::Duration::from_secs(10)),
            })
            .expect("gpu wait");

        // Pump frames until the picking readback arrives.
        let mut result = None;
        for _ in 0..10 {
            ctx.begin_frame();
            result = PickingLayerProcessor::readback_result(&ctx, readback_identifier);
            if result.is_some() {
                break;
            }
        }
        let result = result.expect("no picking readback received");

        // The gaussian sits on the view axis, so it covers the center pixel.
        let center = glam::UVec2::new(resolution[0] / 2, resolution[1] / 2);
        let picked = result.picked_id(center);
        assert_eq!(picked.object, object_id, "wrong picking object id");
        assert_eq!(picked.instance, instance_id, "wrong picking instance id");
    }
}
