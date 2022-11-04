use smallvec::smallvec;

use crate::{
    context::SharedRendererData,
    include_file,
    resource_pools::{
        pipeline_layout_pool::*, render_pipeline_pool::*, shader_module_pool::ShaderModuleDesc,
        WgpuResourcePools,
    },
    view_builder::ViewBuilder,
};

use super::*;

/// Renders a generated skybox from a color gradient
///
/// Is not actually a skybox, but a fullscreen effect.
/// Should be rendered *last* to reduce amount of overdraw!
pub struct GenericSkybox {
    render_pipeline: GpuRenderPipelineHandle,
}

#[derive(Clone)]
pub struct GenericSkyboxDrawable {}

impl Drawable for GenericSkyboxDrawable {
    type Renderer = GenericSkybox;
}

impl GenericSkyboxDrawable {
    pub fn new(ctx: &mut RenderContext, device: &wgpu::Device) -> Self {
        ctx.renderers.get_or_create::<_, GenericSkybox>(
            &ctx.shared_renderer_data,
            &mut ctx.resource_pools,
            device,
            &mut ctx.resolver,
        );

        GenericSkyboxDrawable {}
    }
}

impl Renderer for GenericSkybox {
    type DrawData = GenericSkyboxDrawable;

    fn create_renderer<Fs: FileSystem>(
        shared_data: &SharedRendererData,
        pools: &mut WgpuResourcePools,
        device: &wgpu::Device,
        resolver: &mut FileResolver<Fs>,
    ) -> Self {
        let render_pipeline = pools.render_pipelines.get_or_create(
            device,
            &RenderPipelineDesc {
                label: "generic_skybox".into(),
                pipeline_layout: pools.pipeline_layouts.get_or_create(
                    device,
                    &PipelineLayoutDesc {
                        label: "global only".into(),
                        entries: vec![shared_data.global_bindings.layout],
                    },
                    &pools.bind_group_layouts,
                ),

                vertex_entrypoint: "main".into(),
                vertex_handle: pools.shader_modules.get_or_create(
                    device,
                    resolver,
                    &ShaderModuleDesc {
                        label: "screen_triangle (vertex)".into(),
                        source: include_file!("../../shader/screen_triangle.wgsl"),
                    },
                ),
                fragment_entrypoint: "main".into(),
                fragment_handle: pools.shader_modules.get_or_create(
                    device,
                    resolver,
                    &ShaderModuleDesc {
                        label: "generic_skybox (fragment)".into(),
                        source: include_file!("../../shader/generic_skybox.wgsl"),
                    },
                ),
                vertex_buffers: smallvec![],
                render_targets: smallvec![Some(ViewBuilder::FORMAT_HDR.into())],
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
            &pools.shader_modules,
        );
        GenericSkybox { render_pipeline }
    }

    fn draw<'a>(
        &self,
        pools: &'a WgpuResourcePools,
        pass: &mut wgpu::RenderPass<'a>,
        _draw_data: &GenericSkyboxDrawable,
    ) -> anyhow::Result<()> {
        let pipeline = pools.render_pipelines.get_resource(self.render_pipeline)?;

        pass.set_pipeline(&pipeline.pipeline);
        pass.draw(0..3, 0..1);

        Ok(())
    }

    fn draw_order() -> u32 {
        DrawOrder::Background as u32
    }
}
