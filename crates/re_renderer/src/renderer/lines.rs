use crate::{
    include_file,
    resource_pools::{
        buffer_pool::{BufferDesc, BufferHandle},
        pipeline_layout_pool::PipelineLayoutDesc,
        render_pipeline_pool::*,
        shader_module_pool::ShaderModuleDesc,
    },
    view_builder::ViewBuilder,
};

use super::*;

pub struct LineDrawable {
    instance_buffer: BufferHandle,
    total_number_of_segments: u32,
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

    /// Value from 0 to 1. 0 makes a line invisible, 1 is filled out, 0.5 is half dashes.
    pub stippling: f32,
}

impl LineDrawable {
    pub fn new(
        ctx: &mut RenderContext,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        line_strips: &[LineStrip],
    ) -> anyhow::Result<Self> {
        ctx.renderers.get_or_create::<LineRenderer>(
            &ctx.shared_renderer_data,
            &mut ctx.resource_pools,
            device,
        );

        // Determine how many segments we need
        let total_number_of_segments = line_strips.iter().fold(0, |accum, strip| accum);

        // TODO(andreas): We want a "stack allocation" here that lives for one frame.]
        //                  Note also that this doesn't protect against sharing the same vertex buffer with several LineDrawable!
        let instance_buffer = ctx.resource_pools.buffers.request(
            device,
            &BufferDesc {
                label: "line instance buffer".into(),
                size: 42,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::VERTEX,
                content_id: 0,
            },
        );
        // TODO(andreas): We want a staging-belt(-like) mechanism to upload data instead of the queue.
        //                  This staging buffer would be provided by the belt.
        // let staging_buffer = Vec::with_capacity(total_number_of_segments);
        // staging_buffer.push(HalfInstancePart0 {
        //     pos: todo!(),
        //     color: todo!(),
        //     stipple: todo!(),
        // });

        Ok(LineDrawable {
            instance_buffer,
            total_number_of_segments: total_number_of_segments as u32,
        })
    }
}

pub struct LineRenderer {
    render_pipeline: RenderPipelineHandle,
}

#[repr(C, packed)]
struct HalfInstancePart0 {
    pos: glam::Vec3,
    color: [u8; 3],
    stipple: u8,
}

#[repr(C, packed)]
struct HalfInstancePart1 {
    pos: glam::Vec3,
    thickness: f32, // Could be a f16 if we want to pack even more attributes!
}

static_assertions::assert_eq_size!(HalfInstancePart0, HalfInstancePart1);

//                 ______________________________________________________________________________________________
// Instance Buffer | pos, thickness | pos, color+stipple | pos, thickness | pos, color+stipple | pos, thickness | ...
//                 ______________________________________________________________________________________________
// (vertex shader) |            instance 0               |            instance 2               | ...
//                                  _____________________________________________________________________________
// (vertex shader)                  |            instance 1               |              instance 3             | ...

impl Renderer for LineRenderer {
    type DrawData = LineDrawable;

    fn create_renderer(
        shared_data: &SharedRendererData,
        pools: &mut WgpuResourcePools,
        device: &wgpu::Device,
    ) -> Self {
        let pipeline_layout = pools.pipeline_layouts.request(
            device,
            &PipelineLayoutDesc {
                label: "global only".into(),
                entries: vec![shared_data.global_bindings.layout],
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
                vertex_buffers: vec![wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<HalfInstancePart0>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        // Start position
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 0,
                            shader_location: 0,
                        },
                        // Color + stipple for even instances, thickness for odd instances
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Uint32,
                            offset: 1,
                            shader_location: 1,
                        },
                        // End position
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 2,
                            shader_location: 2,
                        },
                        // Color + stipple for odd instances, thickness for even instances
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Uint32,
                            offset: 3,
                            shader_location: 3,
                        },
                    ],
                }],
                render_targets: vec![Some(ViewBuilder::FORMAT_HDR.into())],
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: ViewBuilder::FORMAT_DEPTH,
                    depth_compare: wgpu::CompareFunction::Always,
                    depth_write_enabled: true, // writes some depth for testing
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
            },
            &pools.pipeline_layouts,
            &pools.shader_modules,
        );

        LineRenderer { render_pipeline }
    }

    fn draw<'a>(
        &self,
        pools: &'a WgpuResourcePools,
        pass: &mut wgpu::RenderPass<'a>,
        draw_data: &Self::DrawData,
    ) -> anyhow::Result<()> {
        //pass.draw_indexed(indices, base_vertex, instances)
        Ok(())
    }
}
