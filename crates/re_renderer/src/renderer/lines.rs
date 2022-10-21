//! Line renderer for efficient rendering of many line(strips)
//!
//!
//! How it works:
//! =================
//!
//! Each line strip consists of a series of quads.
//! All quads are rendered in a single draw call.
//! It is tempting to use instancing and store per-instance (==quad) data in a instance-stepped vertex buffer.
//! However, GPUs are notoriously bad at processing instances with a small batch size as
//! [various](https://gamedev.net/forums/topic/676540-fastest-way-to-draw-quads/5279146/)
//! [people](https://gamedev.net/forums/topic/702292-performance-fastest-quad-drawing/5406023/)
//! [point](https://www.reddit.com/r/vulkan/comments/le74sr/why_gpu_instancing_is_slow_for_small_meshes/)
//! [out](https://www.reddit.com/r/vulkan/comments/47kfve/instanced_rendering_performance/)
//! [...](https://www.reddit.com/r/opengl/comments/q7yikr/how_to_draw_several_quads_through_instancing/).
//! (important to note though that we didn't have the time to explore this for our usecase here)
//!
//! Instead, we do a single triangle list draw call without any vertex buffer at all and fetch data
//! from a texture instead (if it wasn't for WebGL support we'd read from a raw buffer!)
//!
//! To save memory for connected line strips, we interleave the data for quads as follows:
//! ```
//!                  ____________________________________________________________________________________________
//! Segment Texture | pos, thickness | pos, color+stipple | pos, thickness | pos, color+stipple | pos, thickness | ...
//!                  ____________________________________________________________________________________________
//! (vertex shader) |            quad 0                   |                quad 2               | ...
//!                                   ___________________________________________________________________________
//!                                  |               quad 1                |                  quad 3             | ...
//! ```
//! The drawback of this is that if we want to start a new line strip, we need to add a sentinel half-instance and discard a quad.
//!
//!
//! Things we might try in the future
//! ----------------------------------
//! * use instance vertex buffer after all and see how it performs
//!     * note that this would let us remove the degenerated quads between lines, making the approach cleaner
//! * use indexed primitives to lower amount of vertices processed (requires large, predictable index buffer
//! * use line strips with index primitives (using [`wgpu::PrimitiveState::strip_index_format`])
//!

use std::num::NonZeroU32;

use anyhow::Context;

use crate::{
    include_file,
    resource_pools::{
        bind_group_layout_pool::{BindGroupLayout, BindGroupLayoutDesc, BindGroupLayoutHandle},
        bind_group_pool::{BindGroupDesc, BindGroupEntry, BindGroupHandle},
        buffer_pool::{BufferDesc, BufferHandle},
        pipeline_layout_pool::PipelineLayoutDesc,
        render_pipeline_pool::*,
        shader_module_pool::ShaderModuleDesc,
        texture_pool::TextureHandle,
    },
    view_builder::ViewBuilder,
};

use super::*;

mod GpuLineSegment {
    #[repr(C, packed)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct DataEven {
        pub color: [u8; 3],
        pub unused: u8,
    }

    #[repr(C, packed)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct DataOdd {
        pub thickness: f32, // Could be a f16 if we want to pack even more attributes!
    }

    static_assertions::assert_eq_size!(DataEven, DataOdd);

    #[repr(C)]
    #[derive(Clone, Copy, bytemuck::Zeroable)]
    pub union Data {
        pub even: DataEven,
        pub odd: DataOdd,
    }

    #[allow(unsafe_code)]
    unsafe impl bytemuck::Pod for Data {}

    #[repr(C, packed)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct HalfSegment {
        pub pos: glam::Vec3,
        pub data: Data,
    }

    static_assertions::assert_eq_size!(HalfSegment, glam::Vec4);
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

    /// srgb color
    pub color: [u8; 3],
    // TODO(andreas):
    // Value from 0 to 1. 0 makes a line invisible, 1 is filled out, 0.5 is half dashes.
    //pub stippling: f32,
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

        // Instance texture is 2D since 1D textures are very limited in size.
        const SEGMENT_TEXTURE_RESOLUTION: u32 = 512; // 512 x 512 x vec4 == 4mb, 262144 segments

        // Make sure rows this texture can be copied easily.
        static_assertions::const_assert_eq!(
            SEGMENT_TEXTURE_RESOLUTION * std::mem::size_of::<glam::Vec4>() as u32
                % wgpu::COPY_BYTES_PER_ROW_ALIGNMENT,
            0
        );

        // Determine how many half-segments we need
        let num_half_segments = line_strips.iter().fold(0, |accum, strip| {
            // Add a sentinel at the end of each strip
            strip.points.len() + 1 + accum
        }) as u32
            - 1; // Last one doesn't need a sentinel
        if num_half_segments > SEGMENT_TEXTURE_RESOLUTION * SEGMENT_TEXTURE_RESOLUTION {
            // TODO(andreas) just create more draw work items each with its own texture to become "unlimited"
            anyhow::bail!("Too many line segments! Need {num_half_segments} data entries but supported are at max {}", SEGMENT_TEXTURE_RESOLUTION * SEGMENT_TEXTURE_RESOLUTION);
        }

        // No index buffer, so after each strip we have a quad that is discarded (by means of having 0 thickness)
        let num_quads = num_half_segments - 1;

        // TODO(andreas): We want a "stack allocation" here that lives for one frame.
        //                  Note also that this doesn't protect against sharing the same texture with several LineDrawable!
        let segment_texture = ctx.resource_pools.textures.request(
            device,
            &wgpu::TextureDescriptor {
                label: Some("line instance texture"),
                size: wgpu::Extent3d {
                    width: SEGMENT_TEXTURE_RESOLUTION,
                    height: SEGMENT_TEXTURE_RESOLUTION,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba32Float,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            },
        );
        let bind_group = ctx.resource_pools.bind_groups.request(
            device,
            &BindGroupDesc {
                label: "line drawable".into(),
                entries: vec![BindGroupEntry::TextureView(segment_texture)],
                layout: line_renderer.bind_group_layout,
            },
            &ctx.resource_pools.bind_group_layouts,
            &ctx.resource_pools.textures,
            &ctx.resource_pools.buffers,
            &ctx.resource_pools.samplers,
        );

        // TODO(andreas): We want a staging-belt(-like) mechanism to upload data instead of the queue.
        //                  This staging buffer would be provided by the belt.
        let mut half_segment_staging = Vec::with_capacity(num_half_segments as usize);
        for line_strip in line_strips {
            for &pos in &line_strip.points {
                let data = if half_segment_staging.len() % 2 == 0 {
                    GpuLineSegment::Data {
                        even: GpuLineSegment::DataEven {
                            color: line_strip.color,
                            unused: 0,
                        },
                    }
                } else {
                    GpuLineSegment::Data {
                        odd: GpuLineSegment::DataOdd {
                            thickness: line_strip.radius,
                        },
                    }
                };

                half_segment_staging.push(GpuLineSegment::HalfSegment { pos, data });
            }

            // Unless this was the last strip, add a sentinel by duplicating the last element
            if half_segment_staging.len() > 0
                && half_segment_staging.len() < num_half_segments as usize
            {
                half_segment_staging.push(*half_segment_staging.last().unwrap());
            }
        }
        debug_assert_eq!(half_segment_staging.len(), num_half_segments as usize);

        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &ctx.resource_pools.textures.get(segment_texture)?.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(&half_segment_staging),
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: NonZeroU32::new(
                    SEGMENT_TEXTURE_RESOLUTION * std::mem::size_of::<glam::Vec4>() as u32,
                ),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: num_half_segments % SEGMENT_TEXTURE_RESOLUTION as u32,
                height: (num_half_segments + SEGMENT_TEXTURE_RESOLUTION - 1)
                    / SEGMENT_TEXTURE_RESOLUTION,
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
                entries: vec![wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                }],
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
