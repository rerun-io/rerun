use crate::{
    context::SharedRendererData,
    include_file,
    resource_pools::{
        bind_group_layout_pool::*, bind_group_pool::*, pipeline_layout_pool::*,
        render_pipeline_pool::*, shader_module_pool::*, texture_pool::TextureHandle,
        WgpuResourcePools,
    },
};

use super::Renderer;

pub(crate) struct Tonemapper {
    render_pipeline: RenderPipelineHandle,
    bind_group_layout: BindGroupLayoutHandle,
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
                vertex_entrypoint: "main".into(),
                vertex_handle: pools.shader_modules.request(
                    device,
                    &ShaderModuleDesc {
                        label: "screen_triangle (vertex)".into(),
                        source: include_file!("../../shader/screen_triangle.wgsl"),
                    },
                ),
                fragment_entrypoint: "main".into(),
                fragment_handle: pools.shader_modules.request(
                    device,
                    &ShaderModuleDesc {
                        label: "tonemap (fragment)".into(),
                        source: include_file!("../../shader/tonemap.wgsl"),
                    },
                ),
                vertex_buffers: vec![],
                render_targets: vec![Some(shared_data.config.output_format_color.into())],
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
            },
            &pools.pipeline_layouts,
            &pools.shader_modules,
        );

        Tonemapper {
            render_pipeline,
            bind_group_layout,
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
                    entries: vec![BindGroupEntry::TextureView(data.hdr_target)],
                    layout: self.bind_group_layout,
                },
                &pools.bind_group_layouts,
                &pools.textures,
                &pools.buffers,
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
        pass.set_bind_group(1, &bind_group.bind_group, &[]);
        pass.draw(0..3, 0..1);

        Ok(())
    }
}
