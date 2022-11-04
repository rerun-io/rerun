use crate::{
    context::SharedRendererData,
    include_file,
    resource_pools::{
        bind_group_layout_pool::*, bind_group_pool::*, pipeline_layout_pool::*,
        render_pipeline_pool::*, shader_module_pool::*, texture_pool::GpuTextureHandleStrong,
        WgpuResourcePools,
    },
};

use super::*;

use smallvec::smallvec;

pub struct Tonemapper {
    render_pipeline: GpuRenderPipelineHandle,
    bind_group_layout: GpuBindGroupLayoutHandle,
}

#[derive(Clone)]
pub struct TonemapperDrawable {
    /// [`BindGroup`] pointing at the current HDR source and
    /// a uniform buffer for describing a tonemapper configuration.
    hdr_target_bind_group: GpuBindGroupHandleStrong,
}

impl Drawable for TonemapperDrawable {
    type Renderer = Tonemapper;
}

impl TonemapperDrawable {
    pub fn new(
        ctx: &mut RenderContext,
        device: &wgpu::Device,
        hdr_target: &GpuTextureHandleStrong,
    ) -> Self {
        let pools = &mut ctx.resource_pools;
        let tonemapper = ctx.renderers.get_or_create::<_, Tonemapper>(
            &ctx.shared_renderer_data,
            pools,
            device,
            &mut ctx.resolver,
        );
        TonemapperDrawable {
            hdr_target_bind_group: pools.bind_groups.alloc(
                device,
                &BindGroupDesc {
                    label: "tonemapping".into(),
                    entries: smallvec![BindGroupEntry::DefaultTextureView(**hdr_target)],
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

    fn create_renderer<Fs: FileSystem>(
        shared_data: &SharedRendererData,
        pools: &mut WgpuResourcePools,
        device: &wgpu::Device,
        resolver: &mut FileResolver<Fs>,
    ) -> Self {
        let bind_group_layout = pools.bind_group_layouts.get_or_create(
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

        let render_pipeline = pools.render_pipelines.get_or_create(
            device,
            &RenderPipelineDesc {
                label: "tonemapping".into(),
                pipeline_layout: pools.pipeline_layouts.get_or_create(
                    device,
                    &PipelineLayoutDesc {
                        label: "tonemapping".into(),
                        entries: vec![shared_data.global_bindings.layout, bind_group_layout],
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
                        label: "tonemap (fragment)".into(),
                        source: include_file!("../../shader/tonemap.wgsl"),
                    },
                ),
                vertex_buffers: smallvec![],
                render_targets: smallvec![Some(shared_data.config.output_format_color.into())],
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

    fn draw<'a>(
        &self,
        pools: &'a WgpuResourcePools,
        pass: &mut wgpu::RenderPass<'a>,
        draw_data: &TonemapperDrawable,
    ) -> anyhow::Result<()> {
        let pipeline = pools.render_pipelines.get_resource(self.render_pipeline)?;
        let bind_group = pools
            .bind_groups
            .get_resource(&draw_data.hdr_target_bind_group)?;

        pass.set_pipeline(&pipeline.pipeline);
        pass.set_bind_group(1, &bind_group.bind_group, &[]);
        pass.draw(0..3, 0..1);

        Ok(())
    }
}
