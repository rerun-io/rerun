use crate::{
    context::RenderContextConfig,
    resource_pools::{
        bind_group_layout_pool::*, bind_group_pool::*, pipeline_layout_pool::*,
        render_pipeline_pool::*, sampler_pool::*, texture_pool::TextureHandle, WgpuResourcePools,
    },
};

use super::Renderer;

pub(crate) struct Tonemapper {
    render_pipeline: RenderPipelineHandle,
    bind_group_layout: BindGroupLayoutHandle,
    sampler: SamplerHandle,
}

pub(crate) struct TonemapperPrepareData {
    pub hdr_target: TextureHandle,
    // TODO(andreas): Tonemapper
}

#[derive(Default)]
pub(crate) struct TonemapperDrawData {
    /// [`BindGroup`] pointing at the current HDR source and
    /// a uniform buffer for describing a tonemapper configuration.
    hdr_target_bind_group: BindGroupHandle,
}

impl Renderer for Tonemapper {
    type PrepareData = TonemapperPrepareData;
    type DrawData = TonemapperDrawData;

    fn new(
        ctx_config: &RenderContextConfig,
        pools: &mut WgpuResourcePools,
        device: &wgpu::Device,
    ) -> Self {
        // Sampler without any filtering.
        let sampler = pools.samplers.request(
            device,
            &SamplerDesc {
                label: "nearest".into(),
                ..Default::default()
            },
        );

        let bind_group_layout = pools.bind_group_layouts.request(
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

        let render_pipeline = pools.render_pipelines.request(
            device,
            &RenderPipelineDesc {
                label: "Tonemapping".into(),
                pipeline_layout: pools.pipeline_layouts.request(
                    device,
                    &PipelineLayoutDesc {
                        label: "empty".into(),
                        entries: vec![bind_group_layout],
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
                render_targets: vec![Some(ctx_config.output_format_color.into())],
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
            },
            &pools.pipeline_layouts,
        );

        Tonemapper {
            render_pipeline,
            bind_group_layout,
            sampler,
        }
    }

    fn prepare(
        &self,
        pools: &mut WgpuResourcePools,
        device: &wgpu::Device,
        data: &Self::PrepareData,
    ) -> Self::DrawData {
        TonemapperDrawData {
            hdr_target_bind_group: pools.bind_groups.request(
                device,
                &BindGroupDesc {
                    label: "tonemapping".into(),
                    entries: vec![
                        BindGroupEntry::TextureView(data.hdr_target),
                        BindGroupEntry::Sampler(self.sampler),
                    ],
                    layout: self.bind_group_layout,
                },
                &pools.bind_group_layouts,
                &pools.textures,
                &pools.samplers,
            ),
        }
    }

    fn draw<'a>(
        &self,
        pools: &'a WgpuResourcePools,
        pass: &mut wgpu::RenderPass<'a>,
        draw_data: &Self::DrawData,
    ) -> anyhow::Result<()> {
        let pipeline = pools.render_pipelines.get(self.render_pipeline)?;
        let bind_group = pools.bind_groups.get(draw_data.hdr_target_bind_group)?;

        pass.set_pipeline(&pipeline.pipeline);
        pass.set_bind_group(0, &bind_group.bind_group, &[]);
        pass.draw(0..3, 0..1);

        Ok(())
    }
}
