use crate::{
    allocator::create_and_fill_uniform_buffer,
    draw_phases::DrawPhase,
    include_shader_module,
    wgpu_resources::{
        BindGroupDesc, BindGroupEntry, BindGroupLayoutDesc, GpuBindGroup, GpuBindGroupLayoutHandle,
        GpuRenderPipelineHandle, GpuRenderPipelinePoolAccessor, GpuTexture, PipelineLayoutDesc,
        RenderPipelineDesc,
    },
    RectInt,
};

use super::{DrawData, DrawError, RenderContext, Renderer};

use smallvec::smallvec;

mod gpu_data {
    use crate::wgpu_buffer_types;

    /// Modes configuring what the debug overlay shader shows.
    #[derive(Copy, Clone)]
    pub enum DebugOverlayMode {
        /// Show the texture on the f32 texture binding slot.
        ShowFloatTexture = 0,

        /// Show the texture on the uint texture binding slot.
        ShowUintTexture = 1,
    }

    /// Keep in sync with `debug_overlay.wgsl`
    #[repr(C)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct DebugOverlayUniformBuffer {
        pub screen_resolution: wgpu_buffer_types::Vec2,
        pub position_in_pixel: wgpu_buffer_types::Vec2,
        pub extent_in_pixel: wgpu_buffer_types::Vec2,

        /// A value of `DebugOverlayMode`
        pub mode: u32,

        pub _padding: u32,
        pub end_padding: [wgpu_buffer_types::PaddingRow; 16 - 2],
    }
}

pub struct DebugOverlayRenderer {
    render_pipeline: GpuRenderPipelineHandle,
    bind_group_layout: GpuBindGroupLayoutHandle,
}

#[derive(thiserror::Error, Debug)]
pub enum DebugOverlayError {
    #[error("Can't display texture with format: {0:?}")]
    UnsupportedTextureFormat(wgpu::TextureFormat),
}

/// Debug overlay for quick & dirty display of texture contents.
///
/// Executed as part of the composition draw phase in order to allow "direct" output to the screen.
///
/// Do *not* use this in production!
/// See also `debug_overlay.wgsl` - you are encouraged to edit this shader for your concrete debugging needs!
#[derive(Clone)]
pub struct DebugOverlayDrawData {
    bind_group: GpuBindGroup,
}

impl DrawData for DebugOverlayDrawData {
    type Renderer = DebugOverlayRenderer;
}

impl DebugOverlayDrawData {
    pub fn new(
        ctx: &RenderContext,
        debug_texture: &GpuTexture,
        screen_resolution: glam::UVec2,
        overlay_rect: RectInt,
    ) -> Result<Self, DebugOverlayError> {
        let debug_overlay = ctx.renderer::<DebugOverlayRenderer>();

        let mode = match debug_texture
            .texture
            .format()
            .sample_type(Some(wgpu::TextureAspect::All), None)
        {
            Some(wgpu::TextureSampleType::Depth | wgpu::TextureSampleType::Float { .. }) => {
                gpu_data::DebugOverlayMode::ShowFloatTexture
            }
            Some(wgpu::TextureSampleType::Sint | wgpu::TextureSampleType::Uint) => {
                gpu_data::DebugOverlayMode::ShowUintTexture
            }
            None => {
                return Err(DebugOverlayError::UnsupportedTextureFormat(
                    debug_texture.texture.format(),
                ))
            }
        };

        let uniform_buffer_binding = create_and_fill_uniform_buffer(
            ctx,
            "DebugOverlayDrawData".into(),
            gpu_data::DebugOverlayUniformBuffer {
                screen_resolution: screen_resolution.as_vec2().into(),
                position_in_pixel: overlay_rect.min.as_vec2().into(),
                extent_in_pixel: overlay_rect.extent.as_vec2().into(),
                mode: mode as u32,
                _padding: 0,
                end_padding: Default::default(),
            },
        );

        let (texture_float, texture_uint) = match mode {
            gpu_data::DebugOverlayMode::ShowFloatTexture => (
                debug_texture.handle,
                ctx.texture_manager_2d.zeroed_texture_uint().handle,
            ),
            gpu_data::DebugOverlayMode::ShowUintTexture => (
                ctx.texture_manager_2d.white_texture_unorm().handle,
                debug_texture.handle,
            ),
        };

        Ok(Self {
            bind_group: ctx.gpu_resources.bind_groups.alloc(
                &ctx.device,
                &ctx.gpu_resources,
                &BindGroupDesc {
                    label: "DebugOverlay".into(),
                    entries: smallvec![
                        uniform_buffer_binding,
                        BindGroupEntry::DefaultTextureView(texture_float),
                        BindGroupEntry::DefaultTextureView(texture_uint),
                    ],
                    layout: debug_overlay.bind_group_layout,
                },
            ),
        })
    }
}

impl Renderer for DebugOverlayRenderer {
    type RendererDrawData = DebugOverlayDrawData;

    fn create_renderer(ctx: &RenderContext) -> Self {
        re_tracing::profile_function!();

        let bind_group_layout = ctx.gpu_resources.bind_group_layouts.get_or_create(
            &ctx.device,
            &BindGroupLayoutDesc {
                label: "DebugOverlay::bind_group_layout".into(),
                entries: vec![
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: std::num::NonZeroU64::new(std::mem::size_of::<
                                gpu_data::DebugOverlayUniformBuffer,
                            >(
                            )
                                as _),
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Uint,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            },
        );

        let shader_module = ctx.gpu_resources.shader_modules.get_or_create(
            ctx,
            &include_shader_module!("../../shader/debug_overlay.wgsl"),
        );
        let render_pipeline = ctx.gpu_resources.render_pipelines.get_or_create(
            ctx,
            &RenderPipelineDesc {
                label: "DebugOverlayDrawData::render_pipeline_regular".into(),
                pipeline_layout: ctx.gpu_resources.pipeline_layouts.get_or_create(
                    ctx,
                    &PipelineLayoutDesc {
                        label: "DebugOverlay".into(),
                        entries: vec![ctx.global_bindings.layout, bind_group_layout],
                    },
                ),
                vertex_entrypoint: "main_vs".into(),
                vertex_handle: shader_module,
                fragment_entrypoint: "main_fs".into(),
                fragment_handle: shader_module,
                vertex_buffers: smallvec![],
                render_targets: smallvec![Some(ctx.output_format_color().into())],
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleStrip,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
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
        draw_data: &DebugOverlayDrawData,
    ) -> Result<(), DrawError> {
        let pipeline = render_pipelines.get(self.render_pipeline)?;

        pass.set_pipeline(pipeline);
        pass.set_bind_group(1, &draw_data.bind_group, &[]);
        pass.draw(0..4, 0..1);

        Ok(())
    }

    fn participated_phases() -> &'static [DrawPhase] {
        &[DrawPhase::Compositing]
    }
}
