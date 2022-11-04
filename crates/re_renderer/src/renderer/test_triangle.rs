use smallvec::smallvec;

use crate::{
    context::SharedRendererData,
    include_file,
    resource_pools::{
        pipeline_layout_pool::*, render_pipeline_pool::*, shader_module_pool::*, WgpuResourcePools,
    },
    view_builder::ViewBuilder,
};

use super::*;

pub struct TestTriangle {
    render_pipeline: GpuRenderPipelineHandle,
}

#[derive(Clone)]
pub struct TestTriangleDrawable;

impl Drawable for TestTriangleDrawable {
    type Renderer = TestTriangle;
}

impl TestTriangleDrawable {
    pub fn new(ctx: &mut RenderContext, device: &wgpu::Device) -> Self {
        ctx.renderers.get_or_create::<_, TestTriangle>(
            &ctx.shared_renderer_data,
            &mut ctx.resource_pools,
            device,
            &mut ctx.resolver,
        );

        TestTriangleDrawable {}
    }
}

impl Renderer for TestTriangle {
    type DrawData = TestTriangleDrawable;

    fn create_renderer<Fs: FileSystem>(
        shared_data: &SharedRendererData,
        pools: &mut WgpuResourcePools,
        device: &wgpu::Device,
        resolver: &mut FileResolver<Fs>,
    ) -> Self {
        let render_pipeline = pools.render_pipelines.get_or_create(
            device,
            &RenderPipelineDesc {
                label: "Test Triangle".into(),
                pipeline_layout: pools.pipeline_layouts.get_or_create(
                    device,
                    &PipelineLayoutDesc {
                        label: "global only".into(),
                        entries: vec![shared_data.global_bindings.layout],
                    },
                    &pools.bind_group_layouts,
                ),
                vertex_entrypoint: "vs_main".into(),
                vertex_handle: pools.shader_modules.get_or_create(
                    device,
                    resolver,
                    &ShaderModuleDesc {
                        label: "test_triangle (vertex)".into(),
                        source: include_file!("../../shader/test_triangle.wgsl"),
                    },
                ),
                fragment_entrypoint: "fs_main".into(),
                fragment_handle: pools.shader_modules.get_or_create(
                    device,
                    resolver,
                    &ShaderModuleDesc {
                        label: "test_triangle (fragment)".into(),
                        source: include_file!("../../shader/test_triangle.wgsl"),
                    },
                ),
                vertex_buffers: smallvec![],
                render_targets: smallvec![Some(ViewBuilder::FORMAT_HDR.into())],
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

        TestTriangle { render_pipeline }
    }

    fn draw<'a>(
        &self,
        pools: &'a WgpuResourcePools,
        pass: &mut wgpu::RenderPass<'a>,
        _draw_data: &TestTriangleDrawable,
    ) -> anyhow::Result<()> {
        let pipeline = pools.render_pipelines.get_resource(self.render_pipeline)?;
        pass.set_pipeline(&pipeline.pipeline);
        pass.draw(0..3, 0..1);
        Ok(())
    }
}
