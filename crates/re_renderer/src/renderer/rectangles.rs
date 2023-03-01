//! Renderer that makes it easy to draw textured 2d rectangles with transparency
//!
//! Transparency: (TODO(andreas):)
//! We're not performing any sorting on transparency yet, so the transparent rectangles pretty much
//! only work correctly when they are directly layered in front of another opaque rectangle.
//! We do *not* disable depth write.
//!
//! Implementation details:
//! We assume the standard usecase are individual textured rectangles.
//! Since we're not allowed to bind many textures at once (no widespread bindless support!),
//! we are forced to have individual bind groups per rectangle and thus a draw call per rectangle.

use smallvec::smallvec;

use crate::{
    allocator::create_and_fill_uniform_buffer_batch,
    depth_offset::DepthOffset,
    include_file,
    resource_managers::{GpuTexture2DHandle, ResourceManagerError},
    view_builder::ViewBuilder,
    wgpu_resources::{
        BindGroupDesc, BindGroupEntry, BindGroupLayoutDesc, GpuBindGroup, GpuBindGroupLayoutHandle,
        GpuRenderPipelineHandle, PipelineLayoutDesc, RenderPipelineDesc, SamplerDesc,
        ShaderModuleDesc,
    },
    Rgba,
};

use super::{
    DrawData, DrawPhase, FileResolver, FileSystem, RenderContext, Renderer, SharedRendererData,
    WgpuResourcePools,
};

mod gpu_data {
    use crate::wgpu_buffer_types;

    // Keep in sync with mirror in rectangle.wgsl
    #[repr(C, align(256))]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct UniformBuffer {
        pub top_left_corner_position: wgpu_buffer_types::Vec3RowPadded,
        pub extent_u: wgpu_buffer_types::Vec3RowPadded,
        pub extent_v: wgpu_buffer_types::Vec3Unpadded,
        pub depth_offset: f32,
        pub multiplicative_tint: crate::Rgba,

        pub end_padding: [wgpu_buffer_types::PaddingRow; 16 - 4],
    }
}

/// Texture filter setting for magnification (a texel covers several pixels).
#[derive(Debug)]
pub enum TextureFilterMag {
    Linear,
    Nearest,
    // TODO(andreas): Offer advanced (shader implemented) filters like cubic?
}

/// Texture filter setting for minification (several texels fall to one pixel).
#[derive(Debug)]
pub enum TextureFilterMin {
    Linear,
    Nearest,
    // TODO(andreas): Offer mipmapping here?
}

pub struct TexturedRect {
    /// Top left corner position in world space.
    pub top_left_corner_position: glam::Vec3,

    /// Vector that spans up the rectangle from its top left corner along the u axis of the texture.
    pub extent_u: glam::Vec3,

    /// Vector that spans up the rectangle from its top left corner along the v axis of the texture.
    pub extent_v: glam::Vec3,

    /// Texture that fills the rectangle
    pub texture: GpuTexture2DHandle,

    pub texture_filter_magnification: TextureFilterMag,
    pub texture_filter_minification: TextureFilterMin,

    /// Tint that is multiplied to the rect, supports pre-multiplied alpha.
    pub multiplicative_tint: Rgba,

    pub depth_offset: DepthOffset,
}

impl Default for TexturedRect {
    fn default() -> Self {
        Self {
            top_left_corner_position: glam::Vec3::ZERO,
            extent_u: glam::Vec3::ZERO,
            extent_v: glam::Vec3::ZERO,
            texture: GpuTexture2DHandle::invalid(),
            texture_filter_magnification: TextureFilterMag::Nearest,
            texture_filter_minification: TextureFilterMin::Linear,
            multiplicative_tint: Rgba::WHITE,
            depth_offset: 0,
        }
    }
}

#[derive(Clone)]
pub struct RectangleDrawData {
    bind_groups: Vec<GpuBindGroup>,
}

impl DrawData for RectangleDrawData {
    type Renderer = RectangleRenderer;
}

impl RectangleDrawData {
    pub fn new(
        ctx: &mut RenderContext,
        rectangles: &[TexturedRect],
    ) -> Result<Self, ResourceManagerError> {
        crate::profile_function!();

        let mut renderers = ctx.renderers.write();
        let rectangle_renderer = renderers.get_or_create::<_, RectangleRenderer>(
            &ctx.shared_renderer_data,
            &mut ctx.gpu_resources,
            &ctx.device,
            &mut ctx.resolver,
        );

        if rectangles.is_empty() {
            return Ok(RectangleDrawData {
                bind_groups: Vec::new(),
            });
        }

        let uniform_buffer_bindings = create_and_fill_uniform_buffer_batch(
            ctx,
            "rectangle uniform buffers".into(),
            rectangles.iter().map(|rectangle| gpu_data::UniformBuffer {
                top_left_corner_position: rectangle.top_left_corner_position.into(),
                extent_u: rectangle.extent_u.into(),
                extent_v: rectangle.extent_v.into(),
                depth_offset: rectangle.depth_offset as f32,
                multiplicative_tint: rectangle.multiplicative_tint,
                end_padding: Default::default(),
            }),
        );

        let mut bind_groups = Vec::with_capacity(rectangles.len());
        for (rectangle, uniform_buffer) in
            rectangles.iter().zip(uniform_buffer_bindings.into_iter())
        {
            let texture = ctx.texture_manager_2d.get(&rectangle.texture)?;

            let sampler = ctx.gpu_resources.samplers.get_or_create(
                &ctx.device,
                &SamplerDesc {
                    label: format!(
                        "rectangle sampler mag {:?} min {:?}",
                        rectangle.texture_filter_magnification,
                        rectangle.texture_filter_minification
                    )
                    .into(),
                    mag_filter: match rectangle.texture_filter_magnification {
                        TextureFilterMag::Linear => wgpu::FilterMode::Linear,
                        TextureFilterMag::Nearest => wgpu::FilterMode::Nearest,
                    },
                    min_filter: match rectangle.texture_filter_minification {
                        TextureFilterMin::Linear => wgpu::FilterMode::Linear,
                        TextureFilterMin::Nearest => wgpu::FilterMode::Nearest,
                    },
                    mipmap_filter: wgpu::FilterMode::Nearest,
                    ..Default::default()
                },
            );

            bind_groups.push(ctx.gpu_resources.bind_groups.alloc(
                &ctx.device,
                &ctx.gpu_resources,
                &BindGroupDesc {
                    label: "rectangle".into(),
                    entries: smallvec![
                        uniform_buffer,
                        BindGroupEntry::DefaultTextureView(texture.handle),
                        BindGroupEntry::Sampler(sampler)
                    ],
                    layout: rectangle_renderer.bind_group_layout,
                },
            ));
        }

        Ok(RectangleDrawData { bind_groups })
    }
}

pub struct RectangleRenderer {
    render_pipeline: GpuRenderPipelineHandle,
    bind_group_layout: GpuBindGroupLayoutHandle,
}

impl Renderer for RectangleRenderer {
    type RendererDrawData = RectangleDrawData;

    fn create_renderer<Fs: FileSystem>(
        shared_data: &SharedRendererData,
        pools: &mut WgpuResourcePools,
        device: &wgpu::Device,
        resolver: &mut FileResolver<Fs>,
    ) -> Self {
        crate::profile_function!();

        let bind_group_layout = pools.bind_group_layouts.get_or_create(
            device,
            &BindGroupLayoutDesc {
                label: "rectangles".into(),
                entries: vec![
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            // We could use dynamic offset here into a single large buffer.
                            // But we have to set a new texture anyways and its doubtful that splitting the bind group is of any use.
                            has_dynamic_offset: false,
                            min_binding_size: (std::mem::size_of::<gpu_data::UniformBuffer>()
                                as u64)
                                .try_into()
                                .ok(),
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            },
        );

        let pipeline_layout = pools.pipeline_layouts.get_or_create(
            device,
            &PipelineLayoutDesc {
                label: "rectangle".into(),
                entries: vec![shared_data.global_bindings.layout, bind_group_layout],
            },
            &pools.bind_group_layouts,
        );

        let shader_module = pools.shader_modules.get_or_create(
            device,
            resolver,
            &ShaderModuleDesc {
                label: "rectangle".into(),
                source: include_file!("../../shader/rectangle.wgsl"),
            },
        );

        let render_pipeline = pools.render_pipelines.get_or_create(
            device,
            &RenderPipelineDesc {
                label: "rectangle".into(),
                pipeline_layout,
                vertex_entrypoint: "vs_main".into(),
                vertex_handle: shader_module,
                fragment_entrypoint: "fs_main".into(),
                fragment_handle: shader_module,
                vertex_buffers: smallvec![],
                render_targets: smallvec![Some(wgpu::ColorTargetState {
                    format: ViewBuilder::MAIN_TARGET_COLOR_FORMAT,
                    // TODO(andreas): have two render pipelines, an opaque one and a transparent one. Transparent shouldn't write depth!
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleStrip,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: ViewBuilder::MAIN_TARGET_DEFAULT_DEPTH_STATE,
                multisample: ViewBuilder::MAIN_TARGET_DEFAULT_MSAA_STATE,
            },
            &pools.pipeline_layouts,
            &pools.shader_modules,
        );

        RectangleRenderer {
            render_pipeline,
            bind_group_layout,
        }
    }

    fn draw<'a>(
        &self,
        pools: &'a WgpuResourcePools,
        _phase: DrawPhase,
        pass: &mut wgpu::RenderPass<'a>,
        draw_data: &'a Self::RendererDrawData,
    ) -> anyhow::Result<()> {
        crate::profile_function!();
        if draw_data.bind_groups.is_empty() {
            return Ok(());
        }

        let pipeline = pools.render_pipelines.get_resource(self.render_pipeline)?;
        pass.set_pipeline(pipeline);

        for bind_group in &draw_data.bind_groups {
            pass.set_bind_group(1, bind_group, &[]);
            pass.draw(0..4, 0..1);
        }

        Ok(())
    }

    fn participated_phases() -> &'static [DrawPhase] {
        // TODO(andreas): This a hack. We have both opaque and transparent.
        &[DrawPhase::Opaque]
    }
}
