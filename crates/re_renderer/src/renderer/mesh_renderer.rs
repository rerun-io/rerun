//! Basic mesh renderer

use crate::{
    include_file,
    mesh_manager::MeshHandle,
    resource_pools::{
        bind_group_layout_pool::{BindGroupLayoutDesc, BindGroupLayoutHandle},
        pipeline_layout_pool::PipelineLayoutDesc,
        render_pipeline_pool::{RenderPipelineDesc, RenderPipelineHandle},
        shader_module_pool::ShaderModuleDesc,
    },
    view_builder::ViewBuilder,
};

use super::*;

#[derive(Clone)]
pub struct MeshDrawable {}

impl Drawable for MeshDrawable {
    type Renderer = MeshRenderer;
}

pub struct MeshInstance {
    pub mesh: MeshHandle,
    pub transformation: macaw::Conformal3,
}

impl MeshDrawable {
    pub fn new(
        ctx: &mut RenderContext,
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        _instances: &[MeshInstance],
    ) -> anyhow::Result<Self> {
        let _mesh_renderer = ctx.renderers.get_or_create::<_, MeshRenderer>(
            &ctx.shared_renderer_data,
            &mut ctx.resource_pools,
            device,
            &mut ctx.resolver,
        );

        Ok(MeshDrawable {})
    }
}

pub struct MeshRenderer {
    render_pipeline: RenderPipelineHandle,
    bind_group_layout: BindGroupLayoutHandle,
}

impl Renderer for MeshRenderer {
    type DrawData = MeshDrawable;

    fn create_renderer<Fs: FileSystem>(
        shared_data: &SharedRendererData,
        pools: &mut WgpuResourcePools,
        device: &wgpu::Device,
        resolver: &mut FileResolver<Fs>,
    ) -> Self {
        let bind_group_layout = pools.bind_group_layouts.get_or_create(
            device,
            &BindGroupLayoutDesc {
                label: "mesh renderer".into(),
                entries: vec![], // TODO: No data at all??
            },
        );

        let pipeline_layout = pools.pipeline_layouts.get_or_create(
            device,
            &PipelineLayoutDesc {
                label: "mesh renderer".into(),
                entries: vec![shared_data.global_bindings.layout, bind_group_layout],
            },
            &pools.bind_group_layouts,
        );

        let shader_module = pools.shader_modules.get_or_create(
            device,
            resolver,
            &ShaderModuleDesc {
                label: "mesh renderer".into(),
                source: include_file!("../../shader/instanced_mesh.wgsl"),
            },
        );

        let render_pipeline = pools.render_pipelines.get_or_create(
            device,
            &RenderPipelineDesc {
                label: "mesh renderer".into(),
                pipeline_layout,
                vertex_entrypoint: "vs_main".into(),
                vertex_handle: shader_module,
                fragment_entrypoint: "fs_main".into(),
                fragment_handle: shader_module,

                // Instance buffer with pairwise overlapping instances!
                vertex_buffers: vec![],
                render_targets: vec![Some(ViewBuilder::FORMAT_HDR.into())],
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: ViewBuilder::FORMAT_DEPTH,
                    depth_compare: wgpu::CompareFunction::Greater,
                    depth_write_enabled: true,
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
            },
            &pools.pipeline_layouts,
            &pools.shader_modules,
        );

        MeshRenderer {
            render_pipeline,
            bind_group_layout,
        }
    }

    fn draw<'a>(
        &self,
        pools: &'a WgpuResourcePools,
        pass: &mut wgpu::RenderPass<'a>,
        draw_data: &Self::DrawData,
    ) -> anyhow::Result<()> {
        let pipeline = pools.render_pipelines.get_resource(self.render_pipeline)?;

        pass.set_pipeline(&pipeline.pipeline);

        // for instanced_mesh in instanced_meshes {
        //     pass.set_vertex_buffer(0, buffer_slice);
        //     pass.set_vertex_buffer(1, buffer_slice);
        //     pass.set_index_buffer(buffer_slice, index_format);

        //     for material in instanced_mesh.materials {
        //         pass.set_bind_group(1, &bind_group.bind_group, &[]);
        //         pass.draw_indexed(indices, base_vertex, instances);
        //     }
        // }

        Ok(())
    }
}
