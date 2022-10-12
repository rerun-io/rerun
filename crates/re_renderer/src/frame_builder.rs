use anyhow::Context;
use parking_lot::RwLock;
use std::sync::Arc;

use crate::{context::*, render_pipeline_pool::*, texture_pool::TextureHandle};

/// Mirrors the GPU contents of a frame-global uniform buffer.
/// Contains information that is constant for a single frame like camera.
/// (does not contain information that is special to a particular renderer or global to the Context)
//struct FrameUniformBuffer {
// TODO(andreas): camera matrix and the like.
//}

/// The highest level rendering block in `re_renderer`.
///
/// They are used to build up/collect various resources and then send them off for rendering.
/// Collecting objects in this fashion allows for re-use of common resources (e.g. camera)
#[derive(Default)]
pub struct FrameBuilder {
    test_render_pipeline: RenderPipelineHandle,

    hdr_render_target: TextureHandle,
    depth_buffer: TextureHandle,
}

pub type SharedFrameBuilder = Arc<RwLock<FrameBuilder>>;

impl FrameBuilder {
    const FORMAT_HDR: wgpu::TextureFormat = wgpu::TextureFormat::R16Float;
    const FORMAT_DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;

    pub fn new() -> Self {
        FrameBuilder {
            test_render_pipeline: RenderPipelineHandle::default(),
            hdr_render_target: TextureHandle::default(),
            depth_buffer: TextureHandle::default(),
        }
    }

    pub fn new_shared() -> SharedFrameBuilder {
        Arc::new(RwLock::new(FrameBuilder::new()))
    }

    pub fn setup_target(
        &mut self,
        ctx: &mut RenderContext,
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> &mut Self {
        // TODO(andreas): Should tonemapping preferences go here as well? Likely!
        // TODO(andreas): How should we treat multisampling. Once we start it we also need to deal with MSAA resolves
        self.hdr_render_target =
            ctx.texture_pool
                .request_2d_render_target(device, Self::FORMAT_HDR, width, height, 1);
        self.depth_buffer =
            ctx.texture_pool
                .request_2d_render_target(device, Self::FORMAT_DEPTH, width, height, 1);

        self
    }

    pub fn test_triangle(&mut self, ctx: &mut RenderContext, device: &wgpu::Device) -> &mut Self {
        self.test_render_pipeline = ctx.renderpipeline_pool.request_render_pipeline(
            device,
            &RenderPipelineDesc {
                label: "Test Triangle".into(),
                pipeline_layout: Vec::new(),
                vertex_shader: ShaderDesc {
                    shader_code: include_str!("../shader/test_triangle.wgsl").into(),
                    entry_point: "vs_main",
                },
                fragment_shader: ShaderDesc {
                    shader_code: include_str!("../shader/test_triangle.wgsl").into(),
                    entry_point: "fs_main",
                },
                vertex_buffers: vec![],
                render_targets: vec![Some(Self::FORMAT_HDR.into())],
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: Self::FORMAT_DEPTH,
                    depth_compare: wgpu::CompareFunction::Always,
                    depth_write_enabled: false,
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
            },
        );
        self
    }

    /// Draws the frame as instructed to a temporary HDR target.
    pub fn draw(
        &self,
        ctx: &mut RenderContext,
        encoder: &mut wgpu::CommandEncoder,
    ) -> anyhow::Result<()> {
        let color = ctx
            .texture_pool
            .texture(self.hdr_render_target)
            .with_context(|| format!("hdr render target"))?;
        let depth = ctx
            .texture_pool
            .texture(self.depth_buffer)
            .with_context(|| format!("depth buffer"))?;

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("frame builder hdr pass"), // TODO(andreas): It would be nice to specify this from the outside so we know which view we're rendering
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &color.default_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: true,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth.default_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: false, // discards the depth buffer after use, can be faster
                }),
                stencil_ops: None,
            }),
        });

        if let Ok(render_pipeline) = ctx
            .renderpipeline_pool
            .render_pipeline(self.test_render_pipeline)
        {
            pass.set_pipeline(&render_pipeline.pipeline);
            pass.draw(0..3, 0..1);
        }

        Ok(())
    }

    /// Applies tonemapping and draws the final result of a `FrameBuilder` to a given output `RenderPass`.
    ///
    /// The bound surface(s) on the `RenderPass` are expected to be the same format as specified on `Context` creation.
    pub fn finish<'a>(&self, ctx: &'a RenderContext, pass: &mut wgpu::RenderPass<'a>) {
        // TODO: tonemapping

        // if let Some(handle) = self.test_render_pipeline {
        //     let render_pipeline = ctx.render_pipeline(handle);

        //     if let Some(render_pipeline) = render_pipeline {
        //         pass.set_pipeline(render_pipeline);
        //         pass.draw(0..3, 0..1);
        //     }
        // }
    }
}
