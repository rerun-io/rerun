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
//! Data in the "position data texture" is layed out a follows (see `gpu_data::PositionData`):
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
//! Another much more tricky issue is handling of line caps:
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
//! If we want to keep the line along its skeleton with constant thickness, the top right corner
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
//! Things we might try in the future
//! ----------------------------------
//! * more line properties
//! * more per-position attributes
//! * experiment with indexed primitives to lower amount of vertices processed
//!    * note that this would let us remove the degenerated quads between lines, making the approach cleaner and removing the "restart bit"
//!

use std::num::NonZeroU32;

use bytemuck::Zeroable;
use smallvec::smallvec;

use crate::{
    include_file,
    renderer::utils::next_multiple_of,
    resource_pools::{
        bind_group_layout_pool::{BindGroupLayoutDesc, GpuBindGroupLayoutHandle},
        bind_group_pool::{BindGroupDesc, BindGroupEntry, GpuBindGroupHandleStrong},
        pipeline_layout_pool::PipelineLayoutDesc,
        render_pipeline_pool::*,
        shader_module_pool::ShaderModuleDesc,
        texture_pool::TextureDesc,
    },
    view_builder::ViewBuilder,
};

use super::*;

mod gpu_data {
    // Don't use `wgsl_buffer_types` since none of this data goes into a buffer, so its alignment rules don't apply.

    #[repr(C, packed)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct PositionData {
        pub pos: glam::Vec3,
        // TODO(andreas): If we limit ourselves to 65536 line strip (we do as of writing!), we get 16bit extra storage here.
        // We probably want to store accumulated line length in there so that we can do stippling in the fragment shader
        pub strip_index: u32,
    }
    // (unlike the fields in a uniform buffer)
    static_assertions::assert_eq_size!(PositionData, glam::Vec4);

    #[repr(C, packed)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct LineStripInfo {
        pub srgb_color: [u8; 4], // alpha unused right now
        pub stippling: u8,
        pub unused: u8,
        pub thickness: half::f16,
    }
    static_assertions::assert_eq_size!(LineStripInfo, [u32; 2]);
}

/// A line drawing operation. Encompasses several lines, each consisting of a list of positions.
/// Expected to be recrated every frame.
#[derive(Clone)]
pub struct LineDrawable {
    bind_group: GpuBindGroupHandleStrong,
    num_quads: u32,
}

impl Drawable for LineDrawable {
    type Renderer = LineRenderer;
}

/// Description of series of connected line segments that share a radius and a color.
#[derive(Clone)]
pub struct LineStrip {
    /// Connected points. Must be at least 2.
    pub points: Vec<glam::Vec3>,

    /// Radius of the line strip in world space
    /// TODO(andreas) Should be able to specify if this is in pixels, or both by providing a minimum in pixels.
    pub radius: f32,

    /// srgb color. Alpha unused right now
    pub color: [u8; 4],
    // Value from 0 to 1. 0 makes a line invisible, 1 is filled out, 0.5 is half dashes.
    // TODO(andreas): unsupported right now.
    //pub stippling: f32,
}

impl LineDrawable {
    pub fn new(
        ctx: &mut RenderContext,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        line_strips: &[LineStrip],
    ) -> anyhow::Result<Self> {
        let line_renderer = ctx.renderers.get_or_create::<_, LineRenderer>(
            &ctx.shared_renderer_data,
            &mut ctx.resource_pools,
            device,
            &mut ctx.resolver,
        );

        // Textures are 2D since 1D textures are very limited in size (8k typically).
        // Need to keep these values in sync with lines.wgsl!
        const POSITION_TEXTURE_SIZE: u32 = 512; // 512 x 512 x vec4<f32> == 4mb, 262144 PositionDatas
        const LINE_STRIP_TEXTURE_SIZE: u32 = 256; // 256 x 256 x vec2<u32> == 0.5mb, 65536 line strips

        // Make sure the size of a row is a multiple of the row byte alignment to make buffer copies easier.
        static_assertions::const_assert_eq!(
            POSITION_TEXTURE_SIZE * std::mem::size_of::<gpu_data::PositionData>() as u32
                % wgpu::COPY_BYTES_PER_ROW_ALIGNMENT,
            0
        );
        static_assertions::const_assert_eq!(
            LINE_STRIP_TEXTURE_SIZE * std::mem::size_of::<gpu_data::LineStripInfo>() as u32
                % wgpu::COPY_BYTES_PER_ROW_ALIGNMENT,
            0
        );

        let num_line_strips = line_strips.len() as u32;
        let num_positions = line_strips
            .iter()
            .map(|strip| strip.points.len())
            .sum::<usize>() as u32;

        // TODO(andreas): just create more draw work items each with its own texture to become "unlimited"
        anyhow::ensure!(
            num_line_strips <= LINE_STRIP_TEXTURE_SIZE * LINE_STRIP_TEXTURE_SIZE,
            "Too many line strips! The maximum is {} but passed were {num_line_strips}",
            LINE_STRIP_TEXTURE_SIZE * LINE_STRIP_TEXTURE_SIZE
        );
        anyhow::ensure!(
            line_strips.iter().any(|strip| strip.points.len() >= 2),
            "Line strips need to have at least two points each."
        );
        // TODO(andreas): just create more draw work items each with its own texture to become "unlimited".
        //              (note that this one is a bit trickier to fix than extra line-strips, as we need to split a strip!)
        anyhow::ensure!(num_positions <= POSITION_TEXTURE_SIZE * POSITION_TEXTURE_SIZE,
                "Too many line segments! The maximum number of positions is {} but specified were {num_positions}",
                POSITION_TEXTURE_SIZE * POSITION_TEXTURE_SIZE
            );

        // No index buffer, so after each strip we have a quad that is discarded.
        // This means from a geometry perspective there is only ONE strip, i.e. 2 less quads than there are half-PositionDatas!
        let num_quads = num_positions - 1;

        // TODO(andreas): We want a "stack allocation" here that lives for one frame.
        //                  Note also that this doesn't protect against sharing the same texture with several LineDrawable!
        let position_data_texture = ctx.resource_pools.textures.alloc(
            device,
            &TextureDesc {
                label: "line position data".into(),
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
            },
        );
        let line_strip_texture = ctx.resource_pools.textures.alloc(
            device,
            &TextureDesc {
                label: "line strips".into(),
                size: wgpu::Extent3d {
                    width: LINE_STRIP_TEXTURE_SIZE,
                    height: LINE_STRIP_TEXTURE_SIZE,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rg32Uint,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            },
        );

        // TODO(andreas): We want a staging-belt(-like) mechanism to upload data instead of the queue.
        //                  These staging buffers would be provided by the belt.
        // To make the data upload simpler (and have it be done in one go), we always update full rows of each of our textures
        let mut position_data_staging =
            Vec::with_capacity(next_multiple_of(num_positions, POSITION_TEXTURE_SIZE) as usize);
        for (strip_index, line_strip) in line_strips.iter().enumerate() {
            position_data_staging.extend(line_strip.points.iter().map(|&pos| {
                gpu_data::PositionData {
                    pos,
                    strip_index: strip_index as _,
                }
            }));
        }
        position_data_staging.extend(std::iter::repeat(gpu_data::PositionData::zeroed()).take(
            (next_multiple_of(num_positions, POSITION_TEXTURE_SIZE) - num_positions) as usize,
        ));

        let mut line_strip_info_staging =
            Vec::with_capacity(next_multiple_of(num_line_strips, LINE_STRIP_TEXTURE_SIZE) as usize);
        line_strip_info_staging.extend(line_strips.iter().map(|line_strip| {
            gpu_data::LineStripInfo {
                srgb_color: line_strip.color,
                thickness: half::f16::from_f32(line_strip.radius),
                stippling: 0, //(line_strip.stippling.clamp(0.0, 1.0) * 255.0) as u8,
                unused: 0,
            }
        }));
        line_strip_info_staging.extend(std::iter::repeat(gpu_data::LineStripInfo::zeroed()).take(
            (next_multiple_of(num_line_strips, LINE_STRIP_TEXTURE_SIZE) - num_line_strips) as usize,
        ));

        // Upload data from staging buffers to gpu.
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &ctx
                    .resource_pools
                    .textures
                    .get_resource(&position_data_texture)?
                    .texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(&position_data_staging),
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: NonZeroU32::new(
                    POSITION_TEXTURE_SIZE * std::mem::size_of::<gpu_data::PositionData>() as u32,
                ),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: POSITION_TEXTURE_SIZE,
                height: (num_positions + POSITION_TEXTURE_SIZE - 1) / POSITION_TEXTURE_SIZE,
                depth_or_array_layers: 1,
            },
        );
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &ctx
                    .resource_pools
                    .textures
                    .get_resource(&line_strip_texture)?
                    .texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(&line_strip_info_staging),
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: NonZeroU32::new(
                    LINE_STRIP_TEXTURE_SIZE * std::mem::size_of::<gpu_data::LineStripInfo>() as u32,
                ),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: LINE_STRIP_TEXTURE_SIZE,
                height: (num_line_strips + LINE_STRIP_TEXTURE_SIZE - 1) / LINE_STRIP_TEXTURE_SIZE,
                depth_or_array_layers: 1,
            },
        );

        Ok(LineDrawable {
            bind_group: ctx.resource_pools.bind_groups.alloc(
                device,
                &BindGroupDesc {
                    label: "line drawable".into(),
                    entries: smallvec![
                        BindGroupEntry::DefaultTextureView(*position_data_texture),
                        BindGroupEntry::DefaultTextureView(*line_strip_texture),
                    ],
                    layout: line_renderer.bind_group_layout,
                },
                &ctx.resource_pools.bind_group_layouts,
                &ctx.resource_pools.textures,
                &ctx.resource_pools.buffers,
                &ctx.resource_pools.samplers,
            ),
            num_quads,
        })
    }
}

pub struct LineRenderer {
    render_pipeline: GpuRenderPipelineHandle,
    bind_group_layout: GpuBindGroupLayoutHandle,
}

impl Renderer for LineRenderer {
    type DrawData = LineDrawable;

    fn create_renderer<Fs: FileSystem>(
        shared_data: &SharedRendererData,
        pools: &mut WgpuResourcePools,
        device: &wgpu::Device,
        resolver: &mut FileResolver<Fs>,
    ) -> Self {
        let bind_group_layout = pools.bind_group_layouts.get_or_create(
            device,
            &BindGroupLayoutDesc {
                label: "line renderer".into(),
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
                ],
            },
        );

        let pipeline_layout = pools.pipeline_layouts.get_or_create(
            device,
            &PipelineLayoutDesc {
                label: "line renderer".into(),
                entries: vec![shared_data.global_bindings.layout, bind_group_layout],
            },
            &pools.bind_group_layouts,
        );

        let shader_module = pools.shader_modules.get_or_create(
            device,
            resolver,
            &ShaderModuleDesc {
                label: "LineRenderer".into(),
                source: include_file!("../../shader/lines.wgsl"),
            },
        );

        let render_pipeline = pools.render_pipelines.get_or_create(
            device,
            &RenderPipelineDesc {
                label: "LineRenderer".into(),
                pipeline_layout,
                vertex_entrypoint: "vs_main".into(),
                vertex_handle: shader_module,
                fragment_entrypoint: "fs_main".into(),
                fragment_handle: shader_module,
                vertex_buffers: smallvec![],
                render_targets: smallvec![Some(ViewBuilder::FORMAT_HDR.into())],
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: ViewBuilder::FORMAT_DEPTH,
                    depth_compare: wgpu::CompareFunction::Greater,
                    depth_write_enabled: true,
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
            },
            &pools.pipeline_layouts,
            &pools.shader_modules,
        );

        LineRenderer {
            render_pipeline,
            bind_group_layout,
        }
    }

    fn draw<'a>(
        &self,
        pools: &'a WgpuResourcePools,
        pass: &mut wgpu::RenderPass<'a>,
        draw_data: &Self::DrawData,
    ) -> anyhow::Result<()> {
        let pipeline = pools.render_pipelines.get_resource(self.render_pipeline)?;
        let bind_group = pools.bind_groups.get_resource(&draw_data.bind_group)?;

        pass.set_pipeline(&pipeline.pipeline);
        pass.set_bind_group(1, &bind_group.bind_group, &[]);
        pass.draw(0..draw_data.num_quads * 6, 0..1);

        Ok(())
    }
}
