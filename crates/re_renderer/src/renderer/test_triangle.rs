use crate::{
    context::RenderContextConfig,
    frame_builder::FrameBuilder,
    resource_pools::{pipeline_layout_pool::*, render_pipeline_pool::*, WgpuResourcePools},
};

use super::Renderer;

pub(crate) struct TestTriangle {
    render_pipeline: RenderPipelineHandle,
}

pub(crate) struct TestTrianglePrepareData;

#[derive(Default)]
pub(crate) struct TestTriangleDrawData;

impl Renderer for TestTriangle {
    type PrepareData = TestTrianglePrepareData;
    type DrawData = TestTriangleDrawData;

    fn create_renderer(
        _ctx_config: &RenderContextConfig,
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
                        label: "empty".into(),
                        entries: Vec::new(),
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
                render_targets: vec![Some(FrameBuilder::FORMAT_HDR.into())],
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: FrameBuilder::FORMAT_DEPTH,
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

    fn prepare(
        &self,
        _pools: &mut WgpuResourcePools,
        _device: &wgpu::Device,
        _data: &Self::PrepareData,
    ) -> Self::DrawData {
        TestTriangleDrawData {}
    }

    fn draw<'a>(
        &self,
        pools: &'a WgpuResourcePools,
        pass: &mut wgpu::RenderPass<'a>,
        _draw_data: &Self::DrawData,
    ) -> anyhow::Result<()> {
        let pipeline = pools.render_pipelines.get(self.render_pipeline)?;
        pass.set_pipeline(&pipeline.pipeline);
        pass.draw(0..3, 0..1);
        Ok(())
    }
}
