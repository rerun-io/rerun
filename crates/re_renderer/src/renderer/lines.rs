//! Line renderer for efficient rendering of many line(strips)
//!
//!
//! How it works:
//! =================
//!
//! Each line strip consists of a series of quads.
//! All quads are rendered in a single draw call!
//! It is tempting to use instancing and store per-instance (==quad) data in a instance-stepped vertex buffer.
//! However, GPUs are notoriously bad at processing instances with a small batch size as
//! [various](https://gamedev.net/forums/topic/676540-fastest-way-to-draw-quads/5279146/)
//! [people](https://gamedev.net/forums/topic/702292-performance-fastest-quad-drawing/5406023/)
//! [point](https://www.reddit.com/r/vulkan/comments/le74sr/why_gpu_instancing_is_slow_for_small_meshes/)
//! [out](https://www.reddit.com/r/vulkan/comments/47kfve/instanced_rendering_performance/)
//! [...](https://www.reddit.com/r/opengl/comments/q7yikr/how_to_draw_several_quads_through_instancing/).
//! (important to note though that we didn't have the time yet to get performance numbers for our usecase here)
//!
//! Instead, we do a single triangle list draw call without any vertex buffer at all and fetch data
//! from textures instead (if it wasn't for WebGL support we'd read from a raw buffer).
//!
//! Our triangle list topology pretends that there is only a single single strip.
//! So every time a new line strip starts (except on the first strip) we need to discard a quad.
//!
//! Data in the segment texture is layed out a follows:
//! ```
//!                  ___________________________________________________________________
//! Segment Texture | pos, strip_idx | pos, strip_idx | pos, strip_idx | pos, strip_idx | ...
//!                  ___________________________________________________________________
//! (vertex shader) |             quad 0               |              quad 2            |
//!                                   ______________________________________________________________
//!                                  |               quad 1            |              quad 3        | ...
//! ```
//! This means we don't need to duplicate any position data at all!
//! Each strip index points to another smaller texture, describing properties that are global to an entire strip.
//!
//! Why not a triangle *strip* instead if list?:
//! As long as we're not able to restart the strip (requires indices!), we can't discard a quad!
//!
//! Things we might try in the future
//! ----------------------------------
//! * more line properties
//! * more per-position attributes (can pack further)
//! * use indexed primitives to lower amount of vertices processed
//!    * note that this would let us remove the degenerated quads between lines, making the approach cleaner and removing the "restart bit"
//!    * this also plays excellent with line strips if we're using [`wgpu::PrimitiveState::strip_index_format`] to restart the strips
//! * more out of curiosity, use instancing with instance vertex buffer after all and see how it performs
//!

use std::num::NonZeroU32;

use anyhow::Context;

use crate::{
    include_file,
    resource_pools::{
        bind_group_layout_pool::{BindGroupLayoutDesc, BindGroupLayoutHandle},
        bind_group_pool::{BindGroupDesc, BindGroupEntry, BindGroupHandle},
        pipeline_layout_pool::PipelineLayoutDesc,
        render_pipeline_pool::*,
        shader_module_pool::ShaderModuleDesc,
    },
    view_builder::ViewBuilder,
};

use super::*;

mod GpuData {
    #[repr(C, packed)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct HalfSegment {
        pub pos: glam::Vec3,
        // If we limit ourselves to 65536 line strip (we do as of writing!),
        // we get 16bit extra storage here. What do do with it?
        pub strip_index: u32,
    }
    static_assertions::assert_eq_size!(HalfSegment, glam::Vec4);

    #[repr(C, packed)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct LineStripInfo {
        pub color: [u8; 4], // alpha unused right now
        pub stippling: u8,
        pub unused: u8,
        pub thickness: half::f16,
    }
    static_assertions::assert_eq_size!(LineStripInfo, [u32; 2]);
}

#[derive(Clone)]
pub struct LineDrawable {
    bind_group: BindGroupHandle,
    num_quads: u32,
}

impl Drawable for LineDrawable {
    type Renderer = LineRenderer;
}

/// A series of connected lines that share a radius and a color.
pub struct LineStrip {
    /// Connected points. Must be at least 2.
    pub points: Vec<glam::Vec3>,

    /// Radius of the line strip in world space
    /// TODO(andreas) Should be able to specify if this is in pixels, or has a minimum width in pixels.
    pub radius: f32,

    /// srgb color. Alpha unused right now
    pub color: [u8; 4],

    /// Value from 0 to 1. 0 makes a line invisible, 1 is filled out, 0.5 is half dashes.
    /// TODO(andreas): unsupported right now.
    pub stippling: f32,
}

impl LineDrawable {
    pub fn new(
        ctx: &mut RenderContext,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        line_strips: &[LineStrip],
    ) -> anyhow::Result<Self> {
        let line_renderer = ctx.renderers.get_or_create::<LineRenderer>(
            &ctx.shared_renderer_data,
            &mut ctx.resource_pools,
            device,
        );

        // Texture are 2D since 1D textures are very limited in size (8k typically).
        // Need to keep these values in sync with lines.wgsl!
        const SEGMENT_TEXTURE_SIZE: u32 = 512; // 512 x 512 x vec4<f32> == 4mb, 262144 segments
        const LINE_STRIP_TEXTURE_SIZE: u32 = 256; // 128 x 128 x vec2<u32> == 0.5mb, 65536 line strips

        // Make sure rows the texture can be copied easily.
        static_assertions::const_assert_eq!(
            SEGMENT_TEXTURE_SIZE * std::mem::size_of::<GpuData::HalfSegment>() as u32
                % wgpu::COPY_BYTES_PER_ROW_ALIGNMENT,
            0
        );
        static_assertions::const_assert_eq!(
            LINE_STRIP_TEXTURE_SIZE * std::mem::size_of::<GpuData::LineStripInfo>() as u32
                % wgpu::COPY_BYTES_PER_ROW_ALIGNMENT,
            0
        );

        let num_line_strips = line_strips.len() as u32;
        if num_line_strips > LINE_STRIP_TEXTURE_SIZE * LINE_STRIP_TEXTURE_SIZE {
            // TODO(andreas) just create more draw work items each with its own texture to become "unlimited"
            anyhow::bail!(
                "Too many line strips! The maximum is {} but passed were {num_line_strips}",
                LINE_STRIP_TEXTURE_SIZE * LINE_STRIP_TEXTURE_SIZE
            );
        }

        let num_half_segments = (line_strips
            .iter()
            .fold(0, |c, strip| strip.points.len() + c)) as u32;
        if num_half_segments > SEGMENT_TEXTURE_SIZE * SEGMENT_TEXTURE_SIZE {
            // TODO(andreas) just create more draw work items each with its own texture to become "unlimited"
            anyhow::bail!(
                "Too many line segments! The maximum is {} but specified were {num_half_segments}",
                SEGMENT_TEXTURE_SIZE * SEGMENT_TEXTURE_SIZE
            );
        }

        // No index buffer, so after each strip we have a quad that is discarded.
        // This means from a geometry perspective there is only ONE strip, i.e. 2 less quads than there are half-segments!
        let num_quads = num_half_segments - 1;

        // TODO(andreas): We want a "stack allocation" here that lives for one frame.
        //                  Note also that this doesn't protect against sharing the same texture with several LineDrawable!
        let segment_texture = ctx.resource_pools.textures.request(
            device,
            &wgpu::TextureDescriptor {
                label: Some("line segments"),
                size: wgpu::Extent3d {
                    width: SEGMENT_TEXTURE_SIZE,
                    height: SEGMENT_TEXTURE_SIZE,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba32Float,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            },
        );
        let line_strip_texture = ctx.resource_pools.textures.request(
            device,
            &wgpu::TextureDescriptor {
                label: Some("line strips"),
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

        let bind_group = ctx.resource_pools.bind_groups.request(
            device,
            &BindGroupDesc {
                label: "line drawable".into(),
                entries: vec![
                    BindGroupEntry::TextureView(segment_texture),
                    BindGroupEntry::TextureView(line_strip_texture),
                ],
                layout: line_renderer.bind_group_layout,
            },
            &ctx.resource_pools.bind_group_layouts,
            &ctx.resource_pools.textures,
            &ctx.resource_pools.buffers,
            &ctx.resource_pools.samplers,
        );

        // TODO(andreas): We want a staging-belt(-like) mechanism to upload data instead of the queue.
        //                  These staging buffers would be provided by the belt.
        let mut segment_staging = Vec::with_capacity(num_half_segments as usize);
        let mut line_strip_info_staging = Vec::with_capacity(num_line_strips as usize);

        for (strip_index, line_strip) in line_strips.iter().enumerate() {
            segment_staging.extend(line_strip.points.iter().map(|&pos| GpuData::HalfSegment {
                pos,
                strip_index: strip_index as _,
            }));
            line_strip_info_staging.push(GpuData::LineStripInfo {
                color: line_strip.color,
                thickness: half::f16::from_f32(line_strip.radius),
                stippling: (line_strip.stippling.clamp(0.0, 1.0) * 255.0) as u8,
                unused: 0,
            })
        }

        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &ctx.resource_pools.textures.get(segment_texture)?.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(&segment_staging),
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: NonZeroU32::new(
                    SEGMENT_TEXTURE_SIZE * std::mem::size_of::<GpuData::HalfSegment>() as u32,
                ),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: num_half_segments % SEGMENT_TEXTURE_SIZE,
                height: (num_half_segments + SEGMENT_TEXTURE_SIZE - 1) / SEGMENT_TEXTURE_SIZE,
                depth_or_array_layers: 1,
            },
        );
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &ctx.resource_pools.textures.get(line_strip_texture)?.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(&line_strip_info_staging),
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: NonZeroU32::new(
                    LINE_STRIP_TEXTURE_SIZE * std::mem::size_of::<GpuData::LineStripInfo>() as u32,
                ),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: num_line_strips % LINE_STRIP_TEXTURE_SIZE,
                height: (num_line_strips + LINE_STRIP_TEXTURE_SIZE - 1) / LINE_STRIP_TEXTURE_SIZE,
                depth_or_array_layers: 1,
            },
        );

        Ok(LineDrawable {
            bind_group,
            num_quads,
        })
    }
}

pub struct LineRenderer {
    render_pipeline: RenderPipelineHandle,
    bind_group_layout: BindGroupLayoutHandle,
}

impl Renderer for LineRenderer {
    type DrawData = LineDrawable;

    fn create_renderer(
        shared_data: &SharedRendererData,
        pools: &mut WgpuResourcePools,
        device: &wgpu::Device,
    ) -> Self {
        let bind_group_layout = pools.bind_group_layouts.request(
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

        let pipeline_layout = pools.pipeline_layouts.request(
            device,
            &PipelineLayoutDesc {
                label: "line renderer".into(),
                entries: vec![shared_data.global_bindings.layout, bind_group_layout],
            },
            &pools.bind_group_layouts,
        );

        let shader_module = pools.shader_modules.request(
            device,
            &ShaderModuleDesc {
                label: "LineRenderer".into(),
                source: include_file!("../../shader/lines.wgsl"),
            },
        );

        let render_pipeline = pools.render_pipelines.request(
            device,
            &RenderPipelineDesc {
                label: "LineRenderer".into(),
                pipeline_layout,
                vertex_entrypoint: "vs_main".into(),
                vertex_handle: shader_module,
                fragment_entrypoint: "fs_main".into(),
                fragment_handle: shader_module,

                // Instance buffer with pairwise overlapping instances!
                vertex_buffers: vec![],
                render_targets: vec![Some(ViewBuilder::FORMAT_HDR.into())],
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: ViewBuilder::FORMAT_DEPTH,
                    depth_compare: wgpu::CompareFunction::Always, // TODO: put in correct depth test
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
        let pipeline = pools.render_pipelines.get(self.render_pipeline)?;
        let bind_group = pools.bind_groups.get(draw_data.bind_group)?;

        pass.set_pipeline(&pipeline.pipeline);
        pass.set_bind_group(1, &bind_group.bind_group, &[]);
        pass.draw(0..draw_data.num_quads * 6, 0..1);

        Ok(())
    }
}
