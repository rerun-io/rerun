//! Line renderer for efficient rendering of many line(strips)
//!
//!
//! How it works:
//! =================
//!
//! Each drawn line strip consists of a series of quads and all quads are rendered in a single draw call.
//! The only data we upload are the user provided positions (the "skeleton" of the line so to speak) and line strip wide configurations.
//! The quads are oriented and spanned in a vertex shader.
//!
//! It is tempting to use instancing and store per-instance (==quad) data in a instance-stepped vertex buffer.
//! However, GPUs are notoriously bad at processing instances with a small batch size as
//! [various](https://gamedev.net/forums/topic/676540-fastest-way-to-draw-quads/5279146/)
//! [people](https://gamedev.net/forums/topic/702292-performance-fastest-quad-drawing/5406023/)
//! [point](https://www.reddit.com/r/vulkan/comments/le74sr/why_gpu_instancing_is_slow_for_small_meshes/)
//! [out](https://www.reddit.com/r/vulkan/comments/47kfve/instanced_rendering_performance/)
//! […](https://www.reddit.com/r/opengl/comments/q7yikr/how_to_draw_several_quads_through_instancing/).
//!
//! Instead, we use a single (un-instanced) triangle list draw call and use the vertex id to orient ourselves in the vertex shader
//! (e.g. the index of the current quad is `vertex_idx / 6` etc.).
//! Our triangle list topology pretends that there is only a single strip, but in reality we want to render several in one draw call.
//! So every time a new line strip starts (except on the first strip) we need to discard a quad by collapsing vertices into their predecessors.
//!
//! All data we fetch in the vertex shader is uploaded as textures in order to maintain WebGL compatibility.
//! (at the full webgpu feature level we could use raw buffers instead which are easier to handle and a better match for our access pattern)
//!
//! Data is provided in two separate textures, the "position data texture" and the "line strip texture".
//! The "line strip texture" contains packed information over properties that are global to a single strip (see `gpu_data::LineStripInfo`)
//! Data in the "position data texture" is laid out a follows (see `gpu_data::PositionRadius`):
//! ```raw
//!                   ___________________________________________________________________
//! position data    | pos, strip_idx | pos, strip_idx | pos, strip_idx | pos, strip_idx | …
//!                   ___________________________________________________________________
//! (vertex shader)  |             quad 0              |              quad 2             |
//!                                    ______________________________________________________________
//!                                   |               quad 1            |              quad 3        | …
//! ```
//!
//! Why not a triangle *strip* instead if *list*?
//! -----------------------------------------------
//!
//! As long as we're not able to restart the strip (requires indices!), we can't discard a quad in a triangle strip setup.
//! However, this could be solved with an index buffer which has the ability to restart triangle strips (something we haven't tried yet).
//!
//! Another much more tricky issue is handling of line joints:
//! Let's have a look at a corner between two line positions (line positions marked with `X`)
//! ```raw
//! o--------------------------o
//!                            /
//! X=================X       /
//!                  //      /
//! o---------o     //      /
//!          /     //      /
//!         o      X      o
//! ```
//! The problem is that the top right corner would move further and further outward as we decrease the angle of the joint.
//! Instead, we generate overlapping, detached quads and handle line joints as cut-outs in the fragment shader.
//!
//! Line start/end caps (arrows/etc.)
//! -----------------------------------------------
//! Yet another place where our triangle *strip* comes in handy is that we can take triangles from superfluous quads to form pointy arrows.
//! Again, we keep all the geometry calculating logic in the vertex shader.
//!
//! For all batches, independent whether we use caps or not our topology is as follow:
//!            _________________________________________________
//!            \  |                     |\  |                   |\
//!             \ |  … n strip quads …  | \ | … m strip quads … | \
//!              \|_____________________|__\|___________________|__\
//! (start cap triangle only)         (start+end triangle)              (end triangle only)
//!
//!
//! Things we might try in the future
//! ----------------------------------
//! * more line properties
//! * more per-position attributes
//! * experiment with indexed primitives to lower amount of vertices processed
//!    * note that this would let us remove the degenerated quads between lines, making the approach cleaner and removing the "restart bit"
//!

use std::{num::NonZeroU64, ops::Range};

use bitflags::bitflags;
use enumset::{enum_set, EnumSet};
use re_tracing::profile_function;
use smallvec::smallvec;

use crate::{
    allocator::create_and_fill_uniform_buffer_batch,
    draw_phases::{DrawPhase, OutlineMaskProcessor},
    include_shader_module,
    view_builder::ViewBuilder,
    wgpu_resources::{
        BindGroupDesc, BindGroupEntry, BindGroupLayoutDesc, GpuBindGroup, GpuBindGroupLayoutHandle,
        GpuRenderPipelineHandle, GpuRenderPipelinePoolAccessor, PipelineLayoutDesc, PoolError,
        RenderPipelineDesc,
    },
    DebugLabel, DepthOffset, LineDrawableBuilder, OutlineMaskPreference, PickingLayerObjectId,
    PickingLayerProcessor,
};

use super::{DrawData, DrawError, RenderContext, Renderer};

pub mod gpu_data {
    // Don't use `wgsl_buffer_types` since none of this data goes into a buffer, so its alignment rules don't apply.

    use crate::{size::SizeHalf, wgpu_buffer_types, Color32, PickingLayerObjectId};

    use super::LineStripFlags;

    #[repr(C, packed)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct LineVertex {
        pub position: glam::Vec3,
        // TODO(andreas): If we limit ourselves to 65536 line strip (we do as of writing!), we get 16bit extra storage here.
        // We probably want to store accumulated line length in there so that we can do stippling in the fragment shader
        pub strip_index: u32,
    }
    // (unlike the fields in a uniform buffer)
    static_assertions::assert_eq_size!(LineVertex, glam::Vec4);

    impl LineVertex {
        /// Sentinel vertex used at the start and the end of the line vertex data texture to facilitate caps.
        pub const SENTINEL: Self = Self {
            position: glam::vec3(f32::MAX, f32::MAX, f32::MAX),
            strip_index: u32::MAX,
        };

        /// Number of sentinel vertices, one at the start and one at the end.
        pub const NUM_SENTINEL_VERTICES: usize = 2;
    }

    #[repr(C, packed)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct LineStripInfo {
        pub color: Color32, // alpha unused right now
        pub stippling: u8,
        pub flags: LineStripFlags,
        pub radius: SizeHalf,
    }
    static_assertions::assert_eq_size!(LineStripInfo, [u32; 2]);

    impl Default for LineStripInfo {
        fn default() -> Self {
            Self {
                radius: crate::Size::new_ui_points(1.5).into(),
                color: Color32::WHITE,
                stippling: 0,
                flags: LineStripFlags::empty(),
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

    /// Uniform buffer that changes for every batch of line strips.
    #[repr(C)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct BatchUniformBuffer {
        pub world_from_obj: wgpu_buffer_types::Mat4,
        pub outline_mask_ids: wgpu_buffer_types::UVec2,
        pub picking_object_id: PickingLayerObjectId,

        pub depth_offset: f32,
        pub triangle_cap_length_factor: f32,
        pub triangle_cap_width_factor: f32,
        pub _padding: f32,

        pub end_padding: [wgpu_buffer_types::PaddingRow; 16 - 6],
    }
}

/// Internal, ready to draw representation of [`LineBatchInfo`]
#[derive(Clone)]
struct LineStripBatch {
    bind_group: GpuBindGroup,
    vertex_range: Range<u32>,
    active_phases: EnumSet<DrawPhase>,
}

/// A line drawing operation. Encompasses several lines, each consisting of a list of positions.
/// Expected to be recreated every frame.
#[derive(Clone)]
pub struct LineDrawData {
    bind_group_all_lines: Option<GpuBindGroup>,
    bind_group_all_lines_outline_mask: Option<GpuBindGroup>,
    batches: Vec<LineStripBatch>,
}

impl DrawData for LineDrawData {
    type Renderer = LineRenderer;
}

bitflags! {
    /// Property flags for a line strip
    ///
    /// Needs to be kept in sync with `lines.wgsl`
    #[repr(C)]
    #[derive(Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct LineStripFlags : u8 {
        /// Puts a equilateral triangle at the end of the line strip (excludes other end caps).
        const FLAG_CAP_END_TRIANGLE = 0b0000_0001;

        /// Adds a round cap at the end of a line strip (excludes other end caps).
        const FLAG_CAP_END_ROUND = 0b0000_0010;

        /// By default, line caps end at the last/first position of the line strip.
        /// This flag makes end caps extend outwards.
        const FLAG_CAP_END_EXTEND_OUTWARDS = 0b0000_0100;

        /// Puts a equilateral triangle at the start of the line strip (excludes other start caps).
        const FLAG_CAP_START_TRIANGLE = 0b0000_1000;

        /// Adds a round cap at the start of a line strip (excludes other start caps).
        const FLAG_CAP_START_ROUND = 0b0001_0000;

        /// By default, line caps end at the last/first position of the line strip.
        /// This flag makes end caps extend outwards.
        const FLAG_CAP_START_EXTEND_OUTWARDS = 0b0010_0000;

        /// Enable color gradient across the line.
        ///
        /// TODO(andreas): Could be moved to per batch flags.
        const FLAG_COLOR_GRADIENT = 0b0100_0000;

        /// Forces spanning the line's quads as-if the camera was orthographic.
        ///
        /// This is useful for lines that are on a plane that is parallel to the camera:
        /// Without this flag, the lines will poke through the camera plane as they orient themselves towards the camera.
        /// Note that since distances to the camera are computed differently in orthographic mode, this changes how screen space sizes are computed.
        ///
        /// TODO(andreas): Could be moved to per batch flags.
        const FLAG_FORCE_ORTHO_SPANNING = 0b1000_0000;

        /// Combination of flags to extend lines outwards with round caps.
        const FLAGS_OUTWARD_EXTENDING_ROUND_CAPS =
            LineStripFlags::FLAG_CAP_START_ROUND.bits() |
            LineStripFlags::FLAG_CAP_END_ROUND.bits() |
            LineStripFlags::FLAG_CAP_START_EXTEND_OUTWARDS.bits() |
            LineStripFlags::FLAG_CAP_END_EXTEND_OUTWARDS.bits();
    }
}

/// Data that is valid for a batch of line strips.
pub struct LineBatchInfo {
    pub label: DebugLabel,

    /// Transformation applies to line positions
    ///
    /// TODO(andreas): We don't apply scaling to the radius yet. Need to pass a scaling factor like this in
    /// `let scale = Mat3::from(world_from_obj).determinant().abs().cbrt()`
    pub world_from_obj: glam::Affine3A,

    /// Number of vertices covered by this batch.
    ///
    /// The batch will start with the next vertex after the one the previous batch ended with.
    /// It is expected that this vertex is the first vertex of a new batch.
    pub line_vertex_count: u32,

    /// Optional outline mask setting for the entire batch.
    pub overall_outline_mask_ids: OutlineMaskPreference,

    /// Defines an outline mask for an individual vertex ranges (can span several line strips!)
    ///
    /// Vertex ranges are *not* relative within the current batch, but relates to the draw data vertex buffer.
    ///
    /// Having many of these individual outline masks can be slow as they require each their own uniform buffer & draw call.
    /// This feature is meant for a limited number of "extra selections"
    /// If an overall mask is defined as well, the per-vertex-range masks is overwriting the overall mask.
    pub additional_outline_mask_ids_vertex_ranges: Vec<(Range<u32>, OutlineMaskPreference)>,

    /// Picking object id that applies for the entire batch.
    pub picking_object_id: PickingLayerObjectId,

    /// Depth offset applied after projection.
    pub depth_offset: DepthOffset,

    /// Length factor as multiple of a line's radius applied to all triangle caps in this batch.
    ///
    /// This controls how far the "pointy end" of the triangle/arrow-head extends.
    /// (defaults to 4.0)
    pub triangle_cap_length_factor: f32,

    /// Width factor as multiple of a line's radius applied to all triangle caps in this batch.
    ///
    /// This controls how wide the triangle/arrow-head is orthogonal to the line's direction.
    /// (defaults to 2.0)
    pub triangle_cap_width_factor: f32,
}

impl Default for LineBatchInfo {
    fn default() -> Self {
        Self {
            label: "unknown_line_batch".into(),
            world_from_obj: glam::Affine3A::IDENTITY,
            line_vertex_count: 0,
            overall_outline_mask_ids: OutlineMaskPreference::NONE,
            additional_outline_mask_ids_vertex_ranges: Vec::new(),
            picking_object_id: PickingLayerObjectId::default(),
            depth_offset: 0,
            triangle_cap_length_factor: 4.0,
            triangle_cap_width_factor: 2.0,
        }
    }
}

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum LineDrawDataError {
    #[error("Line vertex refers to unknown line strip.")]
    InvalidStripIndex,

    #[error(transparent)]
    PoolError(#[from] PoolError),

    #[error(transparent)]
    FailedTransferringDataToGpu(#[from] crate::allocator::CpuWriteGpuReadError),

    #[error(transparent)]
    DataTextureSourceWriteError(#[from] crate::allocator::DataTextureSourceWriteError),
}

impl LineDrawData {
    /// Transforms and uploads line strip data to be consumed by gpu.
    ///
    /// Try to bundle all line strips into a single draw data instance whenever possible.
    /// If you pass zero lines instances, subsequent drawing will do nothing.
    ///
    /// If no batches are passed, all lines are assumed to be in a single batch with identity transform.
    pub fn new(line_builder: LineDrawableBuilder<'_>) -> Result<Self, LineDrawDataError> {
        let LineDrawableBuilder {
            ctx,
            vertices_buffer,
            batches,
            strips_buffer,
            picking_instance_ids_buffer,
            radius_boost_in_ui_points_for_outlines,
        } = line_builder;

        let line_renderer = ctx.renderer::<LineRenderer>();

        if strips_buffer.is_empty() {
            return Ok(Self {
                bind_group_all_lines: None,
                bind_group_all_lines_outline_mask: None,
                batches: Vec::new(),
            });
        }

        let batches = if batches.is_empty() {
            vec![LineBatchInfo {
                label: "LineDrawData::fallback_batch".into(),
                line_vertex_count: vertices_buffer.len() as _,
                ..Default::default()
            }]
        } else {
            batches
        };

        const NUM_SENTINEL_VERTICES: usize = 2;

        let max_texture_dimension_2d = ctx.device.limits().max_texture_dimension_2d;
        let max_num_texels = max_texture_dimension_2d as usize * max_texture_dimension_2d as usize;
        let max_num_vertices = max_num_texels - NUM_SENTINEL_VERTICES;

        let position_texture = vertices_buffer.finish(
            wgpu::TextureFormat::Rgba32Float,
            "LineDrawData::position_texture",
        )?;
        let strip_data_texture = strips_buffer.finish(
            wgpu::TextureFormat::Rg32Uint,
            "LineDrawData::strip_data_texture",
        )?;
        let picking_instance_id_texture = picking_instance_ids_buffer.finish(
            wgpu::TextureFormat::Rg32Uint,
            "LineDrawData::picking_instance_id_texture",
        )?;

        let draw_data_uniform_buffer_bindings = create_and_fill_uniform_buffer_batch(
            ctx,
            "LineDrawData::DrawDataUniformBuffer".into(),
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
        let bind_group_all_lines = ctx.gpu_resources.bind_groups.alloc(
            &ctx.device,
            &ctx.gpu_resources,
            &BindGroupDesc {
                label: "LineDrawData::bind_group_all_lines".into(),
                entries: smallvec![
                    BindGroupEntry::DefaultTextureView(position_texture.handle),
                    BindGroupEntry::DefaultTextureView(strip_data_texture.handle),
                    BindGroupEntry::DefaultTextureView(picking_instance_id_texture.handle),
                    draw_data_uniform_buffer_bindings[0].clone(),
                ],
                layout: line_renderer.bind_group_layout_all_lines,
            },
        );
        let bind_group_all_lines_outline_mask = ctx.gpu_resources.bind_groups.alloc(
            &ctx.device,
            &ctx.gpu_resources,
            &BindGroupDesc {
                label: "LineDrawData::bind_group_all_lines_outline_mask".into(),
                entries: smallvec![
                    BindGroupEntry::DefaultTextureView(position_texture.handle),
                    BindGroupEntry::DefaultTextureView(strip_data_texture.handle),
                    BindGroupEntry::DefaultTextureView(picking_instance_id_texture.handle),
                    draw_data_uniform_buffer_bindings[1].clone(),
                ],
                layout: line_renderer.bind_group_layout_all_lines,
            },
        );

        // Process batches
        let mut batches_internal = Vec::with_capacity(batches.len());
        {
            fn uniforms_from_batch_info(
                batch_info: &LineBatchInfo,
                outline_mask_ids: [u8; 2],
            ) -> gpu_data::BatchUniformBuffer {
                gpu_data::BatchUniformBuffer {
                    world_from_obj: batch_info.world_from_obj.into(),
                    outline_mask_ids: outline_mask_ids.into(),
                    picking_object_id: batch_info.picking_object_id,
                    depth_offset: batch_info.depth_offset as f32,
                    triangle_cap_length_factor: batch_info.triangle_cap_length_factor,
                    triangle_cap_width_factor: batch_info.triangle_cap_width_factor,
                    _padding: 0.0,
                    end_padding: Default::default(),
                }
            }

            let uniform_buffer_bindings = create_and_fill_uniform_buffer_batch(
                ctx,
                "lines batch uniform buffers".into(),
                batches.iter().map(|batch_info| {
                    uniforms_from_batch_info(
                        batch_info,
                        batch_info.overall_outline_mask_ids.0.unwrap_or_default(),
                    )
                }),
            );

            // Generate additional "micro batches" for each line vertex range that has a unique outline setting.
            // This is fairly costly if there's many, but easy and low-overhead if there's only few, which is usually what we expect!
            let mut uniform_buffer_bindings_mask_only_batches =
                create_and_fill_uniform_buffer_batch(
                    ctx,
                    "lines batch uniform buffers - mask only".into(),
                    batches
                        .iter()
                        .flat_map(|batch_info| {
                            batch_info
                                .additional_outline_mask_ids_vertex_ranges
                                .iter()
                                .map(|(_, mask)| {
                                    uniforms_from_batch_info(batch_info, mask.0.unwrap_or_default())
                                })
                        })
                        .collect::<Vec<_>>()
                        .into_iter(),
                )
                .into_iter();

            let mut start_vertex_for_next_batch = 0;
            for (batch_info, uniform_buffer_binding) in
                batches.iter().zip(uniform_buffer_bindings.into_iter())
            {
                let line_vertex_range_end = (start_vertex_for_next_batch
                    + batch_info.line_vertex_count)
                    .min(max_num_vertices as u32);
                let mut active_phases = enum_set![DrawPhase::Opaque | DrawPhase::PickingLayer];
                // Does the entire batch participate in the outline mask phase?
                if batch_info.overall_outline_mask_ids.is_some() {
                    active_phases.insert(DrawPhase::OutlineMask);
                }

                batches_internal.push(line_renderer.create_linestrip_batch(
                    ctx,
                    batch_info.label.clone(),
                    uniform_buffer_binding,
                    start_vertex_for_next_batch..line_vertex_range_end,
                    active_phases,
                ));

                for (range, _) in &batch_info.additional_outline_mask_ids_vertex_ranges {
                    batches_internal.push(line_renderer.create_linestrip_batch(
                        ctx,
                        format!("{} strip-only {range:?}", batch_info.label).into(),
                        uniform_buffer_bindings_mask_only_batches.next().unwrap(),
                        range.clone(),
                        enum_set![DrawPhase::OutlineMask],
                    ));
                }

                start_vertex_for_next_batch = line_vertex_range_end;
            }
        }

        Ok(Self {
            bind_group_all_lines: Some(bind_group_all_lines),
            bind_group_all_lines_outline_mask: Some(bind_group_all_lines_outline_mask),
            batches: batches_internal,
        })
    }
}

pub struct LineRenderer {
    render_pipeline_color: GpuRenderPipelineHandle,
    render_pipeline_picking_layer: GpuRenderPipelineHandle,
    render_pipeline_outline_mask: GpuRenderPipelineHandle,
    bind_group_layout_all_lines: GpuBindGroupLayoutHandle,
    bind_group_layout_batch: GpuBindGroupLayoutHandle,
}

impl LineRenderer {
    fn create_linestrip_batch(
        &self,
        ctx: &RenderContext,
        label: DebugLabel,
        uniform_buffer_binding: BindGroupEntry,
        line_vertex_range: Range<u32>,
        active_phases: EnumSet<DrawPhase>,
    ) -> LineStripBatch {
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

        LineStripBatch {
            bind_group,
            // We spawn a quad for every line skeleton vertex. Naturally, this yields one extra quad in total.
            // Which is rather convenient because we need to ensure there are start and end triangles,
            // so just from a number-of=vertices perspective this is correct already and the shader can take care of offsets.
            vertex_range: (line_vertex_range.start * 6)..(line_vertex_range.end * 6),
            active_phases,
        }
    }
}

impl Renderer for LineRenderer {
    type RendererDrawData = LineDrawData;

    fn participated_phases() -> &'static [DrawPhase] {
        &[
            DrawPhase::Opaque,
            DrawPhase::OutlineMask,
            DrawPhase::PickingLayer,
        ]
    }

    fn create_renderer(ctx: &RenderContext) -> Self {
        profile_function!();

        let render_pipelines = &ctx.gpu_resources.render_pipelines;

        let bind_group_layout_all_lines = ctx.gpu_resources.bind_group_layouts.get_or_create(
            &ctx.device,
            &BindGroupLayoutDesc {
                label: "LineRenderer::bind_group_layout_all_lines".into(),
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
                            sample_type: wgpu::TextureSampleType::Uint,
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
                label: "LineRenderer::bind_group_layout_batch".into(),
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

        let pipeline_layout = ctx.gpu_resources.pipeline_layouts.get_or_create(
            ctx,
            &PipelineLayoutDesc {
                label: "LineRenderer::pipeline_layout".into(),
                entries: vec![
                    ctx.global_bindings.layout,
                    bind_group_layout_all_lines,
                    bind_group_layout_batch,
                ],
            },
        );

        let shader_module = ctx
            .gpu_resources
            .shader_modules
            .get_or_create(ctx, &include_shader_module!("../../shader/lines.wgsl"));

        let render_pipeline_desc_color = RenderPipelineDesc {
            label: "LineRenderer::render_pipeline_color".into(),
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
            depth_stencil: ViewBuilder::MAIN_TARGET_DEFAULT_DEPTH_STATE,
            multisample: wgpu::MultisampleState {
                // We discard pixels to do the round cutout, therefore we need to calculate our own sampling mask.
                alpha_to_coverage_enabled: true,
                ..ViewBuilder::MAIN_TARGET_DEFAULT_MSAA_STATE
            },
        };
        let render_pipeline_color =
            render_pipelines.get_or_create(ctx, &render_pipeline_desc_color);
        let render_pipeline_picking_layer = render_pipelines.get_or_create(
            ctx,
            &RenderPipelineDesc {
                label: "LineRenderer::render_pipeline_picking_layer".into(),
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
                label: "LineRenderer::render_pipeline_outline_mask".into(),
                pipeline_layout,
                vertex_entrypoint: "vs_main".into(),
                vertex_handle: shader_module,
                fragment_entrypoint: "fs_main_outline_mask".into(),
                fragment_handle: shader_module,
                vertex_buffers: smallvec![],
                render_targets: smallvec![Some(OutlineMaskProcessor::MASK_FORMAT.into())],
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    ..Default::default()
                },
                depth_stencil: OutlineMaskProcessor::MASK_DEPTH_STATE,
                // Alpha to coverage doesn't work with the mask integer target.
                multisample: OutlineMaskProcessor::mask_default_msaa_state(ctx.device_caps().tier),
            },
        );

        Self {
            render_pipeline_color,
            render_pipeline_picking_layer,
            render_pipeline_outline_mask,
            bind_group_layout_all_lines,
            bind_group_layout_batch,
        }
    }

    fn draw(
        &self,
        render_pipelines: &GpuRenderPipelinePoolAccessor<'_>,
        phase: DrawPhase,
        pass: &mut wgpu::RenderPass<'_>,
        draw_data: &Self::RendererDrawData,
    ) -> Result<(), DrawError> {
        let (pipeline_handle, bind_group_all_lines) = match phase {
            DrawPhase::OutlineMask => (
                self.render_pipeline_outline_mask,
                &draw_data.bind_group_all_lines_outline_mask,
            ),
            DrawPhase::Opaque => (self.render_pipeline_color, &draw_data.bind_group_all_lines),
            DrawPhase::PickingLayer => (
                self.render_pipeline_picking_layer,
                &draw_data.bind_group_all_lines,
            ),
            _ => unreachable!("We were called on a phase we weren't subscribed to: {phase:?}"),
        };
        let Some(bind_group_all_lines) = bind_group_all_lines else {
            return Ok(()); // No lines submitted.
        };

        let pipeline = render_pipelines.get(pipeline_handle)?;

        pass.set_pipeline(pipeline);
        pass.set_bind_group(1, bind_group_all_lines, &[]);

        for batch in &draw_data.batches {
            if batch.active_phases.contains(phase) {
                pass.set_bind_group(2, &batch.bind_group, &[]);
                pass.draw(batch.vertex_range.clone(), 0..1);
            }
        }

        Ok(())
    }
}
