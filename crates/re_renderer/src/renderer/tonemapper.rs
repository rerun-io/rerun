use crate::{
    context::SharedRendererData,
    resource_pools::{
        bind_group_layout_pool::*, bind_group_pool::*, pipeline_layout_pool::*,
        render_pipeline_pool::*, texture_pool::TextureHandle, WgpuResourcePools,
    },
};

use super::*;

pub struct Tonemapper {
    render_pipeline: RenderPipelineHandle,
    bind_group_layout: BindGroupLayoutHandle,
}

#[derive(Default, Clone)]
pub struct TonemapperDrawable {
    /// [`BindGroup`] pointing at the current HDR source and
    /// a uniform buffer for describing a tonemapper configuration.
    hdr_target_bind_group: BindGroupHandle,
}

impl Drawable for TonemapperDrawable {
    type Renderer = Tonemapper;
}

impl TonemapperDrawable {
    pub fn new(ctx: &mut RenderContext, device: &wgpu::Device, hdr_target: TextureHandle) -> Self {
        let pools = &mut ctx.resource_pools;
        let tonemapper =
            ctx.renderers
                .get_or_create::<Tonemapper>(&ctx.shared_renderer_data, pools, device);
        TonemapperDrawable {
            hdr_target_bind_group: pools.bind_groups.request(
                device,
                &BindGroupDesc {
                    label: "tonemapping".into(),
                    entries: vec![BindGroupEntry::TextureView(hdr_target)],
                    layout: tonemapper.bind_group_layout,
                },
                &pools.bind_group_layouts,
                &pools.textures,
                &pools.buffers,
                &pools.samplers,
            ),
        }
    }
}

impl Renderer for Tonemapper {
    type DrawData = TonemapperDrawable;

    fn create_renderer(
        shared_data: &SharedRendererData,
        pools: &mut WgpuResourcePools,
        device: &wgpu::Device,
    ) -> Self {
        let bind_group_layout = pools.bind_group_layouts.request(
            device,
            &BindGroupLayoutDesc {
                label: "tonemapping".into(),
                entries: vec![wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::default(),
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                }],
            },
        );

        let render_pipeline = pools.render_pipelines.request(
            device,
            &RenderPipelineDesc {
                label: "tonemapping".into(),
                pipeline_layout: pools.pipeline_layouts.request(
                    device,
                    &PipelineLayoutDesc {
                        label: "tonemapping".into(),
                        entries: vec![shared_data.global_bindings.layout, bind_group_layout],
                    },
                    &pools.bind_group_layouts,
                ),
                vertex_shader: ShaderDesc {
                    shader_code: include_str!("../../shader/screen_triangle.wgsl").into(),
                    entry_point: "main",
                },
                fragment_shader: ShaderDesc {
                    shader_code: include_str!("../../shader/tonemap.wgsl").into(),
                    entry_point: "main",
                },
                vertex_buffers: vec![],
                render_targets: vec![Some(shared_data.config.output_format_color.into())],
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
            },
            &pools.pipeline_layouts,
        );

        Tonemapper {
            render_pipeline,
            bind_group_layout,
        }
    }

    fn draw<'a>(
        &self,
        pools: &'a WgpuResourcePools,
        pass: &mut wgpu::RenderPass<'a>,
        draw_data: &TonemapperDrawable,
    ) -> anyhow::Result<()> {
        let pipeline = pools.render_pipelines.get(self.render_pipeline)?;
        let bind_group = pools.bind_groups.get(draw_data.hdr_target_bind_group)?;

        pass.set_pipeline(&pipeline.pipeline);
        pass.set_bind_group(1, &bind_group.bind_group, &[]);
        pass.draw(0..3, 0..1);

        Ok(())
    }
}
