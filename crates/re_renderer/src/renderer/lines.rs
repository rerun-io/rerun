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
//! [...](https://www.reddit.com/r/opengl/comments/q7yikr/how_to_draw_several_quads_through_instancing/).
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
//! Data in the "position data texture" is laid out a follows (see `gpu_data::PositionData`):
//! ```raw
//!                   ___________________________________________________________________
//! position data    | pos, strip_idx | pos, strip_idx | pos, strip_idx | pos, strip_idx | ...
//!                   ___________________________________________________________________
//! (vertex shader)  |             quad 0              |              quad 2             |
//!                                    ______________________________________________________________
//!                                   |               quad 1            |              quad 3        | ...
//! ```
//!
//! Why not a triangle *strip* instead if *list*?
//! -----------------------------------------------
//!
//! As long as we're not able to restart the strip (requires indices!), we can't discard a quad in a triangle strip setup.
//! However, this could be solved with an index buffer which has the ability to restart triangle strips (something we haven't tried yet).
//!
//! Another much more tricky issue is handling of line miters:
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
//! If we want to keep the line along its skeleton with constant radius, the top right corner
//! would move further and further outward as we decrease the angle of the joint. Eventually it reaches infinity!
//! (i.e. not great to fix it up with discard in the fragment shader either)
//!
//! To prevent this we need to generate this shape:
//! ```raw
//! a-------------------b
//!                       \
//! X=================X    \
//!                  //     \
//! c---------d     //      e
//!          /     //      /
//!         f      X      g
//! ```
//!
//! To achieve this we need to do one of:
//! 1) generating a new triangle at `[d,b,e]`
//!     * can't do that without significant preprocessing, makes the entire pipeline much more complicated
//! 2) twist one of the quads, making both quads overlap in the area of `[d,b,e]` (doesn't add any new vertices)
//!    * unless (!) we duplicate vertices at one of the quads, the twist would need to continue for the rest of the strip!
//! 3) make one quad stop before the joint by forming `[a,b,d,c]`, the other one taking over the joint by forming `[b,e,g,f]`
//!    * implies breaking up the quads (point d would be part of one quad but not the other)
//!
//! (2) and (3) can be implemented relatively easy if we're using a triangle strip!
//! (2) can be implemented in theory with a triangle list, but means that any joint has ripple effects on the rest of the list.
//!
//! TODO(andreas): Implement (3). Right now we don't implement line caps at all.
//!
//!
//! Line start/end caps (arrows/rounded/etc.)
//! -----------------------------------------------
//! Yet another place where our triangle *strip* comes in handy is that we can take triangles from superfluous quads to form pointy arrows.
//! Again, we keep all the geometry calculating logic in the vertex shader.
//!
//! For all batches, independent whether we use caps or not our topology is as follow:
//!            _________________________________________________________
//!            \  |                         |\  |                       |\
//!             \ |  ... n strip quads ...  | \ | ... m strip quads ... | \
//!              \|_________________________|__\|_______________________|__\
//! (start cap triangle only)         (start+end triangle)              (end triangle only)
//!
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
use bytemuck::Zeroable;
use enumset::{enum_set, EnumSet};
use smallvec::smallvec;

use crate::{
    allocator::create_and_fill_uniform_buffer_batch,
    draw_phases::{DrawPhase, OutlineMaskProcessor},
    include_shader_module,
    size::Size,
    view_builder::ViewBuilder,
    wgpu_resources::{
        BindGroupDesc, BindGroupEntry, BindGroupLayoutDesc, GpuBindGroup, GpuBindGroupLayoutHandle,
        GpuRenderPipelineHandle, PipelineLayoutDesc, PoolError, RenderPipelineDesc, TextureDesc,
    },
    Color32, DebugLabel, DepthOffset, LineStripSeriesBuilder, OutlineMaskPreference,
    PickingLayerObjectId, PickingLayerProcessor,
};

use super::{
    DrawData, FileResolver, FileSystem, LineVertex, RenderContext, Renderer, SharedRendererData,
    WgpuResourcePools,
};

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

    #[repr(C, packed)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct LineStripInfo {
        pub color: Color32, // alpha unused right now
        pub stippling: u8,
        pub flags: LineStripFlags,
        pub radius: SizeHalf,
    }
    static_assertions::assert_eq_size!(LineStripInfo, [u32; 2]);

    /// Uniform buffer that changes once per draw data rendering.
    #[repr(C, align(256))]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct DrawDataUniformBuffer {
        pub radius_boost_in_ui_points: wgpu_buffer_types::F32RowPadded,
        pub end_padding: [wgpu_buffer_types::PaddingRow; 16 - 1],
    }

    /// Uniform buffer that changes for every batch of line strips.
    #[repr(C, align(256))]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct BatchUniformBuffer {
        pub world_from_obj: wgpu_buffer_types::Mat4,
        pub outline_mask_ids: wgpu_buffer_types::UVec2,
        pub picking_object_id: PickingLayerObjectId,

        pub depth_offset: wgpu_buffer_types::F32RowPadded,

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
/// Expected to be recrated every frame.
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
    #[derive(Default, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct LineStripFlags : u8 {
        /// Puts a equilateral triangle at the end of the line strip (excludes other end caps).
        const FLAG_CAP_END_TRIANGLE = 0b0000_0001;

        /// Adds a round cap at the end of a line strip (excludes other end caps).
        const FLAG_CAP_END_ROUND = 0b0000_0010;

        /// By default, line caps end at the last/first position of the the line strip.
        /// This flag makes end caps extend outwards.
        const FLAG_CAP_END_EXTEND_OUTWARDS = 0b0000_0100;

        /// Puts a equilateral triangle at the start of the line strip (excludes other start caps).
        const FLAG_CAP_START_TRIANGLE = 0b0000_1000;

        /// Adds a round cap at the start of a line strip (excludes other start caps).
        const FLAG_CAP_START_ROUND = 0b0001_0000;

        /// By default, line caps end at the last/first position of the the line strip.
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
}

/// Style information for a line strip.
#[derive(Clone)]
pub struct LineStripInfo {
    /// Radius of the line strip in world space
    pub radius: Size,

    /// srgb color. Alpha unused right now
    pub color: Color32,

    /// Additional properties for the linestrip.
    pub flags: LineStripFlags,
    // Value from 0 to 1. 0 makes a line invisible, 1 is filled out, 0.5 is half dashes.
    // TODO(andreas): unsupported right now.
    //pub stippling: f32,
}

impl Default for LineStripInfo {
    fn default() -> Self {
        Self {
            radius: Size::AUTO,
            color: Color32::WHITE,
            flags: LineStripFlags::empty(),
        }
    }
}

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum LineDrawDataError {
    #[error("Line vertex refers to unknown line strip.")]
    InvalidStripIndex,

    #[error("A resource failed to resolve.")]
    PoolError(#[from] PoolError),
}

// Textures are 2D since 1D textures are very limited in size (8k typically).
// Need to keep these values in sync with lines.wgsl!
const POSITION_TEXTURE_SIZE: u32 = 512; // 512 x 512 x vec4<f32> == 4MiB, 262144 PositionData
const LINE_STRIP_TEXTURE_SIZE: u32 = 256; // 256 x 256 x vec2<u32> == 0.5MiB, 65536 line strips

impl LineDrawData {
    /// Total maximum number of line vertices per [`LineDrawData`].
    ///
    /// TODO(#957): Get rid of this limit!.
    pub const MAX_NUM_VERTICES: usize =
        (POSITION_TEXTURE_SIZE * POSITION_TEXTURE_SIZE - 2) as usize; // Subtract sentinels

    /// Total maximum number of line strips per [`LineDrawData`].
    ///
    /// TODO(#957): Get rid of this limit!.
    pub const MAX_NUM_STRIPS: usize = (LINE_STRIP_TEXTURE_SIZE * LINE_STRIP_TEXTURE_SIZE) as usize;

    /// Transforms and uploads line strip data to be consumed by gpu.
    ///
    /// Try to bundle all line strips into a single draw data instance whenever possible.
    /// If you pass zero lines instances, subsequent drawing will do nothing.
    ///
    /// If no batches are passed, all lines are assumed to be in a single batch with identity transform.
    pub fn new(
        ctx: &mut RenderContext,
        line_builder: LineStripSeriesBuilder,
    ) -> Result<Self, LineDrawDataError> {
        let mut renderers = ctx.renderers.write();
        let line_renderer = renderers.get_or_create::<_, LineRenderer>(
            &ctx.shared_renderer_data,
            &mut ctx.gpu_resources,
            &ctx.device,
            &mut ctx.resolver,
        );

        if line_builder.strips.is_empty() {
            return Ok(LineDrawData {
                bind_group_all_lines: None,
                bind_group_all_lines_outline_mask: None,
                batches: Vec::new(),
            });
        }

        let LineStripSeriesBuilder {
            vertices,
            batches,
            strips,
            mut picking_instance_ids_buffer,
            radius_boost_in_ui_points_for_outlines,
        } = line_builder;

        let batches = if batches.is_empty() {
            vec![LineBatchInfo {
                world_from_obj: glam::Affine3A::IDENTITY,
                label: "LineDrawData::fallback_batch".into(),
                line_vertex_count: vertices.len() as _,
                overall_outline_mask_ids: OutlineMaskPreference::NONE,
                picking_object_id: PickingLayerObjectId::default(),
                additional_outline_mask_ids_vertex_ranges: Vec::new(),
                depth_offset: 0,
            }]
        } else {
            batches
        };

        // Make sure the size of a row is a multiple of the row byte alignment to make buffer copies easier.
        static_assertions::const_assert_eq!(
            POSITION_TEXTURE_SIZE * std::mem::size_of::<gpu_data::LineVertex>() as u32
                % wgpu::COPY_BYTES_PER_ROW_ALIGNMENT,
            0
        );
        static_assertions::const_assert_eq!(
            LINE_STRIP_TEXTURE_SIZE * std::mem::size_of::<gpu_data::LineStripInfo>() as u32
                % wgpu::COPY_BYTES_PER_ROW_ALIGNMENT,
            0
        );

        let vertices = if vertices.len() >= Self::MAX_NUM_VERTICES {
            re_log::error_once!("Reached maximum number of supported line vertices. Clamping down to {}, passed were {}.
 See also https://github.com/rerun-io/rerun/issues/957", Self::MAX_NUM_VERTICES, vertices.len() );
            &vertices[..Self::MAX_NUM_VERTICES]
        } else {
            &vertices[..]
        };
        let strips = if strips.len() > Self::MAX_NUM_STRIPS {
            re_log::error_once!("Reached maximum number of supported line strips. Clamping down to {}, passed were {}. This may lead to rendering artifacts.
 See also https://github.com/rerun-io/rerun/issues/957", Self::MAX_NUM_STRIPS, strips.len());
            &strips[..Self::MAX_NUM_STRIPS]
        } else {
            // Can only check for strip index validity if we haven't clamped the strips!
            if vertices
                .iter()
                .any(|v| v.strip_index >= strips.len() as u32)
            {
                return Err(LineDrawDataError::InvalidStripIndex);
            }
            &strips[..]
        };

        let num_strips = strips.len() as u32;
        // Add a sentinel vertex both at the beginning and the end to make cap calculation easier.
        let num_segments = vertices.len() as u32 + 2;

        // TODO(andreas): We want a "stack allocation" here that lives for one frame.
        //                  Note also that this doesn't protect against sharing the same texture with several LineDrawData!
        let position_data_texture_desc = TextureDesc {
            label: "LineDrawData::position_data_texture".into(),
            size: wgpu::Extent3d {
                width: POSITION_TEXTURE_SIZE,
                height: POSITION_TEXTURE_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        };
        let position_data_texture = ctx
            .gpu_resources
            .textures
            .alloc(&ctx.device, &position_data_texture_desc);

        let line_strip_texture_desc = TextureDesc {
            label: "LineDrawData::line_strip_texture".into(),
            size: wgpu::Extent3d {
                width: LINE_STRIP_TEXTURE_SIZE,
                height: LINE_STRIP_TEXTURE_SIZE,
                depth_or_array_layers: 1,
            },
            format: wgpu::TextureFormat::Rg32Uint,
            ..position_data_texture_desc
        };
        let line_strip_texture = ctx
            .gpu_resources
            .textures
            .alloc(&ctx.device, &line_strip_texture_desc);
        let picking_instance_id_texture = ctx.gpu_resources.textures.alloc(
            &ctx.device,
            &TextureDesc {
                label: "LineDrawData::picking_instance_id_texture".into(),
                format: wgpu::TextureFormat::Rg32Uint,
                ..line_strip_texture_desc
            },
        );

        // Upload position data.
        {
            // To make the data upload simpler (and have it be done in one go), we always update full rows of each of our textures
            let mut position_data_staging = Vec::with_capacity(wgpu::util::align_to(
                num_segments,
                POSITION_TEXTURE_SIZE,
            ) as usize);
            // sentinel at the beginning to facilitate caps.
            position_data_staging.push(LineVertex {
                position: glam::vec3(f32::MAX, f32::MAX, f32::MAX),
                strip_index: u32::MAX,
            });
            position_data_staging.extend(vertices.iter());
            // placeholder at the end to facilitate caps.
            position_data_staging.push(LineVertex {
                position: glam::vec3(f32::MAX, f32::MAX, f32::MAX),
                strip_index: u32::MAX,
            });
            position_data_staging.extend(std::iter::repeat(gpu_data::LineVertex::zeroed()).take(
                (wgpu::util::align_to(num_segments, POSITION_TEXTURE_SIZE) - num_segments) as usize,
            ));

            // TODO(andreas): Use staging belt here.
            ctx.queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &position_data_texture.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                bytemuck::cast_slice(&position_data_staging),
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(
                        POSITION_TEXTURE_SIZE * std::mem::size_of::<gpu_data::LineVertex>() as u32,
                    ),
                    rows_per_image: None,
                },
                wgpu::Extent3d {
                    width: POSITION_TEXTURE_SIZE,
                    height: (num_segments + POSITION_TEXTURE_SIZE - 1) / POSITION_TEXTURE_SIZE,
                    depth_or_array_layers: 1,
                },
            );
        }

        // Upload strip data.
        {
            let mut line_strip_info_staging = Vec::with_capacity(wgpu::util::align_to(
                num_strips,
                LINE_STRIP_TEXTURE_SIZE,
            ) as usize);
            line_strip_info_staging.extend(strips.iter().map(|line_strip| {
                gpu_data::LineStripInfo {
                    color: line_strip.color,
                    radius: line_strip.radius.into(),
                    stippling: 0, //(line_strip.stippling.clamp(0.0, 1.0) * 255.0) as u8,
                    flags: line_strip.flags,
                }
            }));
            let num_strips_padding =
                (wgpu::util::align_to(num_strips, LINE_STRIP_TEXTURE_SIZE) - num_strips) as usize;
            line_strip_info_staging.extend(
                std::iter::repeat(gpu_data::LineStripInfo::zeroed()).take(num_strips_padding),
            );

            let strip_texture_extent = wgpu::Extent3d {
                width: LINE_STRIP_TEXTURE_SIZE,
                height: (num_strips + LINE_STRIP_TEXTURE_SIZE - 1) / LINE_STRIP_TEXTURE_SIZE,
                depth_or_array_layers: 1,
            };

            // TODO(andreas): Use staging belt here.
            ctx.queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &line_strip_texture.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                bytemuck::cast_slice(&line_strip_info_staging),
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(
                        LINE_STRIP_TEXTURE_SIZE
                            * std::mem::size_of::<gpu_data::LineStripInfo>() as u32,
                    ),
                    rows_per_image: None,
                },
                strip_texture_extent,
            );

            picking_instance_ids_buffer
                .extend(std::iter::repeat(Default::default()).take(num_strips_padding));
            picking_instance_ids_buffer.copy_to_texture2d(
                ctx.active_frame.before_view_builder_encoder.lock().get(),
                wgpu::ImageCopyTexture {
                    texture: &picking_instance_id_texture.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                glam::uvec2(strip_texture_extent.width, strip_texture_extent.height),
            );
        }

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
                    BindGroupEntry::DefaultTextureView(position_data_texture.handle),
                    BindGroupEntry::DefaultTextureView(line_strip_texture.handle),
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
                    BindGroupEntry::DefaultTextureView(position_data_texture.handle),
                    BindGroupEntry::DefaultTextureView(line_strip_texture.handle),
                    BindGroupEntry::DefaultTextureView(picking_instance_id_texture.handle),
                    draw_data_uniform_buffer_bindings[1].clone(),
                ],
                layout: line_renderer.bind_group_layout_all_lines,
            },
        );

        // Process batches
        let mut batches_internal = Vec::with_capacity(batches.len());
        {
            let uniform_buffer_bindings = create_and_fill_uniform_buffer_batch(
                ctx,
                "lines batch uniform buffers".into(),
                batches
                    .iter()
                    .map(|batch_info| gpu_data::BatchUniformBuffer {
                        world_from_obj: batch_info.world_from_obj.into(),
                        outline_mask_ids: batch_info
                            .overall_outline_mask_ids
                            .0
                            .unwrap_or_default()
                            .into(),
                        picking_object_id: batch_info.picking_object_id,
                        depth_offset: (batch_info.depth_offset as f32).into(),
                        end_padding: Default::default(),
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
                                .map(|(_, mask)| gpu_data::BatchUniformBuffer {
                                    world_from_obj: batch_info.world_from_obj.into(),
                                    outline_mask_ids: mask.0.unwrap_or_default().into(),
                                    picking_object_id: batch_info.picking_object_id,
                                    depth_offset: (batch_info.depth_offset as f32).into(),
                                    end_padding: Default::default(),
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
                    .min(Self::MAX_NUM_VERTICES as u32);
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

                // Should happen only if the number of vertices was clamped.
                if start_vertex_for_next_batch >= vertices.len() as u32 {
                    break;
                }
            }
        }

        Ok(LineDrawData {
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

    fn create_renderer<Fs: FileSystem>(
        shared_data: &SharedRendererData,
        pools: &mut WgpuResourcePools,
        device: &wgpu::Device,
        resolver: &mut FileResolver<Fs>,
    ) -> Self {
        let bind_group_layout_all_lines = pools.bind_group_layouts.get_or_create(
            device,
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

        let bind_group_layout_batch = pools.bind_group_layouts.get_or_create(
            device,
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

        let pipeline_layout = pools.pipeline_layouts.get_or_create(
            device,
            &PipelineLayoutDesc {
                label: "LineRenderer::pipeline_layout".into(),
                entries: vec![
                    shared_data.global_bindings.layout,
                    bind_group_layout_all_lines,
                    bind_group_layout_batch,
                ],
            },
            &pools.bind_group_layouts,
        );

        let shader_module = pools.shader_modules.get_or_create(
            device,
            resolver,
            &include_shader_module!("../../shader/lines.wgsl"),
        );

        let render_pipeline_desc_color = RenderPipelineDesc {
            label: "LineRenderer::render_pipeline_color".into(),
            pipeline_layout,
            vertex_entrypoint: "vs_main".into(),
            vertex_handle: shader_module,
            fragment_entrypoint: "fs_main".into(),
            fragment_handle: shader_module,
            vertex_buffers: smallvec![],
            render_targets: smallvec![Some(ViewBuilder::MAIN_TARGET_COLOR_FORMAT.into())],
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
        let render_pipeline_color = pools.render_pipelines.get_or_create(
            device,
            &render_pipeline_desc_color,
            &pools.pipeline_layouts,
            &pools.shader_modules,
        );
        let render_pipeline_picking_layer = pools.render_pipelines.get_or_create(
            device,
            &RenderPipelineDesc {
                label: "LineRenderer::render_pipeline_picking_layer".into(),
                fragment_entrypoint: "fs_main_picking_layer".into(),
                render_targets: smallvec![Some(PickingLayerProcessor::PICKING_LAYER_FORMAT.into())],
                depth_stencil: PickingLayerProcessor::PICKING_LAYER_DEPTH_STATE,
                multisample: PickingLayerProcessor::PICKING_LAYER_MSAA_STATE,
                ..render_pipeline_desc_color.clone()
            },
            &pools.pipeline_layouts,
            &pools.shader_modules,
        );
        let render_pipeline_outline_mask = pools.render_pipelines.get_or_create(
            device,
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
                multisample: OutlineMaskProcessor::mask_default_msaa_state(
                    shared_data.config.hardware_tier,
                ),
            },
            &pools.pipeline_layouts,
            &pools.shader_modules,
        );

        LineRenderer {
            render_pipeline_color,
            render_pipeline_picking_layer,
            render_pipeline_outline_mask,
            bind_group_layout_all_lines,
            bind_group_layout_batch,
        }
    }

    fn draw<'a>(
        &self,
        pools: &'a WgpuResourcePools,
        phase: DrawPhase,
        pass: &mut wgpu::RenderPass<'a>,
        draw_data: &'a Self::RendererDrawData,
    ) -> anyhow::Result<()> {
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

        let pipeline = pools.render_pipelines.get_resource(pipeline_handle)?;

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
