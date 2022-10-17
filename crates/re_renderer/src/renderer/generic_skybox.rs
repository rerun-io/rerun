use crate::{
    context::RenderContextConfig,
    resource_pools::{pipeline_layout_pool::*, render_pipeline_pool::*, WgpuResourcePools},
};

use super::Renderer;

/// Renders a generated skybox from a color gradient
///
/// Is not actually a skybox, but a fullscreen effect.
/// Should be rendered *last* to reduce amount of overdraw!
pub(crate) struct GenericSkybox {
    render_pipeline: RenderPipelineHandle,
}

pub(crate) struct GenericSkyboxPrepareData {}

#[derive(Default)]
pub(crate) struct GenericSkyboxDrawData {}

impl Renderer for GenericSkybox {
    type PrepareData = GenericSkyboxPrepareData;
    type DrawData = GenericSkyboxDrawData;

    fn create_renderer(
        ctx_config: &RenderContextConfig,
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
                        label: "empty".into(),
                        entries: Vec::new(),
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
                render_targets: vec![Some(ctx_config.output_format_color.into())],
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
            },
            &pools.pipeline_layouts,
        );

        GenericSkybox { render_pipeline }
    }

    fn prepare(
        &self,
        _pools: &mut WgpuResourcePools,
        _device: &wgpu::Device,
        _data: &Self::PrepareData,
    ) -> Self::DrawData {
        GenericSkyboxDrawData {}
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
