use crate::{
    context::SharedRendererData,
    resource_pools::{pipeline_layout_pool::*, render_pipeline_pool::*, WgpuResourcePools},
    view_builder::ViewBuilder,
};

use super::*;

pub struct TestTriangle {
    render_pipeline: RenderPipelineHandle,
}

#[derive(Clone)]
pub struct TestTriangleDrawData;

impl DrawData for TestTriangleDrawData {
    type Renderer = TestTriangle;
}

impl TestTriangleDrawData {
    pub fn new(ctx: &mut RenderContext, device: &wgpu::Device) -> Self {
        ctx.renderers.get_or_create::<TestTriangle>(
            &ctx.shared_renderer_data,
            &mut ctx.resource_pools,
            device,
        );

        TestTriangleDrawData {}
    }
}

impl Renderer for TestTriangle {
    type D = TestTriangleDrawData;

    fn create_renderer(
        shared_data: &SharedRendererData,
        pools: &mut WgpuResourcePools,
        device: &wgpu::Device,
    ) -> Self {
        let render_pipeline = pools.render_pipelines.request(
            device,
            &RenderPipelineDesc {
                label: "Test Triangle".into(),
                pipeline_layout: pools.pipeline_layouts.request(
                    device,
                    &PipelineLayoutDesc {
                        label: "global only".into(),
                        entries: vec![shared_data.global_bindings.layout],
                    },
                    &pools.bind_group_layouts,
                ),
                vertex_shader: ShaderDesc {
                    shader_code: include_str!("../../shader/test_triangle.wgsl").into(),
                    entry_point: "vs_main",
                },
                fragment_shader: ShaderDesc {
                    shader_code: include_str!("../../shader/test_triangle.wgsl").into(),
                    entry_point: "fs_main",
                },
                vertex_buffers: vec![],
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
        );

        TestTriangle { render_pipeline }
    }

    fn draw<'a>(
        &self,
        pools: &'a WgpuResourcePools,
        pass: &mut wgpu::RenderPass<'a>,
        _draw_data: &TestTriangleDrawData,
    ) -> anyhow::Result<()> {
        let pipeline = pools.render_pipelines.get(self.render_pipeline)?;
        pass.set_pipeline(&pipeline.pipeline);
        pass.draw(0..3, 0..1);
        Ok(())
    }
}
