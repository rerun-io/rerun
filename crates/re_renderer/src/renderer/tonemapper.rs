use crate::{
    context::RenderContext,
    resource_pools::{
        bind_group_layout_pool::*, bind_group_pool::*, pipeline_layout_pool::*,
        render_pipeline_pool::*, sampler_pool::*, texture_pool::TextureHandle,
    },
};

use super::renderer::*;

pub struct Tonemapper {
    render_pipeline: RenderPipelineHandle,
    bind_group_layout: BindGroupLayoutHandle,
    sampler: SamplerHandle,
}

pub struct TonemapperDrawInput {
    hdr_target: TextureHandle,
}

#[derive(Default)]
pub struct TonemapperDrawData {
    hdr_target_bind_group: BindGroupHandle,
}

impl Renderer for Tonemapper {
    fn new(ctx: &mut RenderContext, device: &wgpu::Device) -> Self {
        // Sampler without any filtering.
        let sampler = ctx.samplers.request(
            device,
            &SamplerDesc {
                label: "nearest".into(),
                ..Default::default()
            },
        );

        let bind_group_layout = ctx.bind_group_layouts.request(
            device,
            &BindGroupLayoutDesc {
                label: "tonemapping".into(),
                entries: vec![
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::default(),
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // TODO(andreas): a bunch of basic sampler should go to future bind-group 0 which will always be bound
                    // (handle for that one should probably live on the context or some other object encapsulating knowledge about it)
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                        count: None,
                    },
                ],
            },
        );

        let render_pipeline = ctx.render_pipelines.request(
            device,
            &RenderPipelineDesc {
                label: "Tonemapping".into(),
                pipeline_layout: ctx.pipeline_layouts.request(
                    device,
                    &PipelineLayoutDesc {
                        label: "empty".into(),
                        entries: vec![bind_group_layout],
                    },
                    &ctx.bind_group_layouts,
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
                render_targets: vec![Some(ctx.output_format_color().into())],
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
            },
            &ctx.pipeline_layouts,
        );

        Tonemapper {
            render_pipeline,
            bind_group_layout,
            sampler,
        }
    }
}

impl RendererImpl<TonemapperDrawInput, TonemapperDrawData> for Tonemapper {
    fn build_draw_data(
        &self,
        ctx: &mut RenderContext,
        device: &wgpu::Device,
        input: &TonemapperDrawInput,
    ) -> TonemapperDrawData {
        TonemapperDrawData {
            hdr_target_bind_group: ctx.bind_groups.request(
                device,
                &BindGroupDesc {
                    label: "tonemapping".into(),
                    entries: vec![
                        BindGroupEntry::TextureView(input.hdr_target),
                        BindGroupEntry::Sampler(self.sampler),
                    ],
                    layout: self.bind_group_layout,
                },
                &ctx.bind_group_layouts,
                &ctx.textures,
                &ctx.samplers,
            ),
        }
    }

    fn draw<'a>(
        &self,
        ctx: &'a RenderContext,
        pass: &mut wgpu::RenderPass<'a>,
        draw_data: &TonemapperDrawData,
    ) -> anyhow::Result<()> {
        let pipeline = ctx.render_pipelines.get(self.render_pipeline)?;
        let bind_group = ctx.bind_groups.get(draw_data.hdr_target_bind_group)?;

        pass.set_pipeline(&pipeline.pipeline);
        pass.set_bind_group(0, &bind_group.bind_group, &[]);
        pass.draw(0..3, 0..1);

        Ok(())
    }
}
