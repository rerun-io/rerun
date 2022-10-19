use crate::{
    context::SharedRendererData,
    resource_pools::{pipeline_layout_pool::*, render_pipeline_pool::*, WgpuResourcePools},
    view_builder::ViewBuilder,
};

use super::*;

/// Renders a generated skybox from a color gradient
///
/// Is not actually a skybox, but a fullscreen effect.
/// Should be rendered *last* to reduce amount of overdraw!
pub struct GenericSkybox {
    render_pipeline: RenderPipelineHandle,
}

#[derive(Clone)]
pub struct GenericSkyboxDrawData {}

impl DrawData for GenericSkyboxDrawData {
    type Renderer = GenericSkybox;
}

impl GenericSkyboxDrawData {
    pub fn new(ctx: &mut RenderContext, device: &wgpu::Device) -> Self {
        ctx.renderers.get_or_create::<GenericSkybox>(
            &ctx.shared_renderer_data,
            &mut ctx.resource_pools,
            device,
        );

        GenericSkyboxDrawData {}
    }
}

impl Renderer for GenericSkybox {
    type D = GenericSkyboxDrawData;

    fn create_renderer(
        shared_data: &SharedRendererData,
        pools: &mut WgpuResourcePools,
        device: &wgpu::Device,
    ) -> Self {
        let render_pipeline = pools.render_pipelines.request(
            device,
            &RenderPipelineDesc {
                label: "generic_skybox".into(),
                pipeline_layout: pools.pipeline_layouts.request(
                    device,
                    &PipelineLayoutDesc {
                        label: "global only".into(),
                        entries: vec![shared_data.global_bindings.layout],
                    },
                    &pools.bind_group_layouts,
                ),
                vertex_shader: ShaderDesc {
                    shader_code: include_str!("../../shader/screen_triangle.wgsl").into(),
                    entry_point: "main",
                },
                fragment_shader: ShaderDesc {
                    shader_code: include_str!("../../shader/generic_skybox.wgsl").into(),
                    entry_point: "main",
                },
                vertex_buffers: vec![],
                render_targets: vec![Some(ViewBuilder::FORMAT_HDR.into())],
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: ViewBuilder::FORMAT_DEPTH,
                    // Pass depth test only if the fragment hasn't been written to.
                    // This allows us to draw the skybox last which is much more efficient than using it as a clear pass!
                    depth_compare: wgpu::CompareFunction::Equal,
                    depth_write_enabled: false,
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
            },
            &pools.pipeline_layouts,
        );
        GenericSkybox { render_pipeline }
    }

    fn draw<'a>(
        &self,
        pools: &'a WgpuResourcePools,
        pass: &mut wgpu::RenderPass<'a>,
        _draw_data: &GenericSkyboxDrawData,
    ) -> anyhow::Result<()> {
        let pipeline = pools.render_pipelines.get(self.render_pipeline)?;

        pass.set_pipeline(&pipeline.pipeline);
        pass.draw(0..3, 0..1);

        Ok(())
    }

    fn draw_order() -> u32 {
        DrawOrder::Background as u32
    }
}
