use smallvec::smallvec;

use super::{DrawData, DrawError, RenderContext, Renderer};
use crate::allocator::create_and_fill_uniform_buffer;
use crate::draw_phases::DrawPhase;
use crate::renderer::{
    DrawDataDrawable, DrawInstruction, DrawableCollectionViewInfo, screen_triangle_vertex_shader,
};
use crate::view_builder::ViewBuilder;
use crate::wgpu_resources::{
    BindGroupDesc, BindGroupLayoutDesc, GpuBindGroup, GpuBindGroupLayoutHandle,
    GpuRenderPipelineHandle, GpuRenderPipelinePoolAccessor, PipelineLayoutDesc, RenderPipelineDesc,
};
use crate::{DrawableCollector, include_shader_module};

/// The type of generic skybox to render.
///
/// If you want a solid background color, don't add the skybox at all and instead set a clear color.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum GenericSkyboxType {
    #[default]
    GradientDark = 0,
    GradientBright = 1,
}

mod gpu_data {
    use crate::wgpu_buffer_types;

    #[repr(C)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct UniformBuffer {
        pub background_type: wgpu_buffer_types::U32RowPadded,
        pub _end_padding: [wgpu_buffer_types::PaddingRow; 16 - 1],
    }
}

/// Renders a generated skybox from a color gradient
///
/// Is not actually a skybox, but a fullscreen effect.
/// Should be rendered *last* to reduce amount of overdraw!
pub struct GenericSkybox {
    render_pipeline: GpuRenderPipelineHandle,
    bind_group_layout: GpuBindGroupLayoutHandle,
}

#[derive(Clone)]
pub struct GenericSkyboxDrawData {
    bind_group: GpuBindGroup,
}

impl DrawData for GenericSkyboxDrawData {
    type Renderer = GenericSkybox;

    fn collect_drawables(
        &self,
        _view_info: &DrawableCollectionViewInfo,
        collector: &mut DrawableCollector<'_>,
    ) {
        collector.add_drawable(
            DrawPhase::Background,
            DrawDataDrawable {
                distance_sort_key: 0.0,
                draw_data_payload: 0,
            },
        );
    }
}

impl GenericSkyboxDrawData {
    pub fn new(ctx: &RenderContext, typ: GenericSkyboxType) -> Self {
        let skybox_renderer = ctx.renderer::<GenericSkybox>();

        let uniform_buffer = gpu_data::UniformBuffer {
            background_type: (typ as u32).into(),
            _end_padding: Default::default(),
        };

        let uniform_buffer_binding =
            create_and_fill_uniform_buffer(ctx, "skybox uniform buffer".into(), uniform_buffer);

        let bind_group = ctx.gpu_resources.bind_groups.alloc(
            &ctx.device,
            &ctx.gpu_resources,
            &BindGroupDesc {
                label: "GenericSkyboxDrawData::bind_group".into(),
                entries: smallvec![uniform_buffer_binding,],
                layout: skybox_renderer.bind_group_layout,
            },
        );

        Self { bind_group }
    }
}

impl Renderer for GenericSkybox {
    type RendererDrawData = GenericSkyboxDrawData;

    fn create_renderer(ctx: &RenderContext) -> Self {
        re_tracing::profile_function!();

        let bind_group_layout = ctx.gpu_resources.bind_group_layouts.get_or_create(
            &ctx.device,
            &(BindGroupLayoutDesc {
                label: "GenericSkybox::bind_group_layout".into(),
                entries: vec![wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: (std::mem::size_of::<gpu_data::UniformBuffer>() as u64)
                            .try_into()
                            .ok(),
                    },
                    count: None,
                }],
            }),
        );

        let vertex_handle = screen_triangle_vertex_shader(ctx);
        let render_pipeline = ctx.gpu_resources.render_pipelines.get_or_create(
            ctx,
            &RenderPipelineDesc {
                label: "GenericSkybox::render_pipeline".into(),
                pipeline_layout: ctx.gpu_resources.pipeline_layouts.get_or_create(
                    ctx,
                    &PipelineLayoutDesc {
                        label: "GenericSkybox::render_pipeline".into(),
                        entries: vec![ctx.global_bindings.layout, bind_group_layout],
                    },
                ),

                vertex_entrypoint: "main".into(),
                vertex_handle,
                fragment_entrypoint: "main".into(),
                fragment_handle: ctx.gpu_resources.shader_modules.get_or_create(
                    ctx,
                    &include_shader_module!("../../shader/generic_skybox.wgsl"),
                ),
                vertex_buffers: smallvec![],
                render_targets: smallvec![Some(ViewBuilder::MAIN_TARGET_COLOR_FORMAT.into())],
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: ViewBuilder::MAIN_TARGET_DEPTH_FORMAT,
                    // Pass depth test only if the fragment hasn't been written to.
                    // This allows us to draw the skybox last which is much more efficient than using it as a clear pass!
                    depth_compare: wgpu::CompareFunction::Equal,
                    depth_write_enabled: false,
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: ViewBuilder::main_target_default_msaa_state(
                    ctx.render_config(),
                    false,
                ),
            },
        );
        Self {
            render_pipeline,
            bind_group_layout,
        }
    }

    fn draw(
        &self,
        render_pipelines: &GpuRenderPipelinePoolAccessor<'_>,
        _phase: DrawPhase,
        pass: &mut wgpu::RenderPass<'_>,
        draw_instructions: &[DrawInstruction<'_, Self::RendererDrawData>],
    ) -> Result<(), DrawError> {
        re_tracing::profile_function!();

        let pipeline = render_pipelines.get(self.render_pipeline)?;
        pass.set_pipeline(pipeline);

        for DrawInstruction { draw_data, .. } in draw_instructions {
            pass.set_bind_group(1, &draw_data.bind_group, &[]);
            pass.draw(0..3, 0..1);
        }

        Ok(())
    }
}
