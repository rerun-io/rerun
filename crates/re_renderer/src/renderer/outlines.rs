//! Outlines
//!
//! TODO: How do they work, how are they configured. What's going on!
//! * mask layer
//! * MSAA handling

use crate::{
    context::SharedRendererData,
    include_file,
    view_builder::ViewBuilder,
    wgpu_resources::{
        BindGroupDesc, BindGroupEntry, BindGroupLayoutDesc, GpuBindGroup, GpuBindGroupLayoutHandle,
        GpuRenderPipelineHandle, GpuTexture, PipelineLayoutDesc, PoolError, RenderPipelineDesc,
        ShaderModuleDesc, WgpuResourcePools,
    },
    DebugLabel, FileResolver, FileSystem, RenderContext,
};

use super::{screen_triangle_vertex_shader, DrawData, DrawPhase, Renderer};

use smallvec::smallvec;

#[derive(Clone, Debug)]
pub struct OutlineConfig {
    /// Outline thickness for both layers in pixels. Fractional pixels are valid.
    ///
    /// Could do different thicknesses for both layers if the need arises, but for now this simplifies things.
    pub outline_thickness_pixel: f32,
    pub color_layer_0: crate::Rgba,
    pub color_layer_1: crate::Rgba,
}

// TODO(andreas): Is this a sort of DrawPhase implementor? Need a system for this.
pub struct OutlineMaskProcessor {
    label: DebugLabel,

    mask_texture: GpuTexture,
    mask_depth: GpuTexture,
    distance_textures: [GpuTexture; 2],

    bindgroup_jumpflooding_init: GpuBindGroup,
    bindgroup_read_distance: [GpuBindGroup; 2],

    render_pipeline_jumpflooding_init: GpuRenderPipelineHandle,
    render_pipeline_jumpflooding_step: GpuRenderPipelineHandle,
}

impl OutlineMaskProcessor {
    /// Format of the outline mask target.
    ///
    /// Two channels with each 256 object ids.
    pub const MASK_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg8Uint;

    pub const MASK_MSAA_STATE: wgpu::MultisampleState = wgpu::MultisampleState {
        count: ViewBuilder::MAIN_TARGET_SAMPLE_COUNT,
        mask: !0,
        alpha_to_coverage_enabled: false,
    };
    pub const MASK_DEPTH_FORMAT: wgpu::TextureFormat = ViewBuilder::MAIN_TARGET_DEPTH_FORMAT;
    pub const MASK_DEPTH_STATE: Option<wgpu::DepthStencilState> =
        ViewBuilder::MAIN_TARGET_DEFAULT_DEPTH_STATE;

    /// Holds two pairs of texture coordinates (one for each layer).
    ///
    /// Since we know the range is [0;1] [`wgpu::TextureFormat::Rgba16Snorm`] would be preferred,
    /// but this requires a non-standard feature.
    const DISTANCE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

    pub fn new(
        ctx: &mut RenderContext,
        config: OutlineConfig,
        view_name: &DebugLabel,
        resolution_in_pixel: [u32; 2],
    ) -> Self {
        let mut renderers = ctx.renderers.write();
        let compositor_renderer = renderers.get_or_create::<_, OutlineCompositor>(
            &ctx.shared_renderer_data,
            &mut ctx.gpu_resources,
            &ctx.device,
            &mut ctx.resolver,
        );

        let instance_label = view_name.clone().push_str(" - OutlineMaskProcessor");
        let screen_texture_size = wgpu::Extent3d {
            width: resolution_in_pixel[0],
            height: resolution_in_pixel[1],
            depth_or_array_layers: 1,
        };

        let mask_texture = ctx.gpu_resources.textures.alloc(
            &ctx.device,
            &crate::wgpu_resources::TextureDesc {
                label: instance_label.clone().push_str("::mask_texture"),
                size: screen_texture_size,
                mip_level_count: 1,
                sample_count: ViewBuilder::MAIN_TARGET_SAMPLE_COUNT,
                dimension: wgpu::TextureDimension::D2,
                format: Self::MASK_FORMAT,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::RENDER_ATTACHMENT,
            },
        );

        // We have a fresh depth buffer here that we need because:
        // * We want outlines visible even if there's an object in front, so don't re-use previous
        // * Overdraw IDs correctly
        // * TODO(andreas): Make overdrawn outlines more transparent by comparing depth
        let mask_depth = ctx.gpu_resources.textures.alloc(
            &ctx.device,
            &crate::wgpu_resources::TextureDesc {
                label: instance_label.clone().push_str("::mask_depth"),
                size: screen_texture_size,
                mip_level_count: 1,
                sample_count: ViewBuilder::MAIN_TARGET_SAMPLE_COUNT,
                dimension: wgpu::TextureDimension::D2,
                format: Self::MASK_DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            },
        );

        let distance_texture_desc = crate::wgpu_resources::TextureDesc {
            label: instance_label.clone().push_str("::distance_texture"),
            size: screen_texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DISTANCE_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
        };
        let distance_textures = [
            ctx.gpu_resources.textures.alloc(
                &ctx.device,
                &distance_texture_desc
                    .with_label(distance_texture_desc.label.clone().push_str("0")),
            ),
            ctx.gpu_resources.textures.alloc(
                &ctx.device,
                &distance_texture_desc
                    .with_label(distance_texture_desc.label.clone().push_str("1")),
            ),
        ];

        let bind_group_layout_read_mask = ctx.gpu_resources.bind_group_layouts.get_or_create(
            &ctx.device,
            &BindGroupLayoutDesc {
                label: "OutlineMaskProcessor::bind_group_layout_read_mask".into(),
                entries: vec![wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: true,
                    },
                    count: None,
                }],
            },
        );
        let bindgroup_jumpflooding_init = ctx.gpu_resources.bind_groups.alloc(
            &ctx.device,
            &BindGroupDesc {
                label: instance_label.clone().push_str("::jumpflooding_init"),
                entries: smallvec![BindGroupEntry::DefaultTextureView(mask_texture.handle)],
                layout: bind_group_layout_read_mask,
            },
            &ctx.gpu_resources.bind_group_layouts,
            &ctx.gpu_resources.textures,
            &ctx.gpu_resources.buffers,
            &ctx.gpu_resources.samplers,
        );
        let bindgroup_read_distance = [
            ctx.gpu_resources.bind_groups.alloc(
                &ctx.device,
                &BindGroupDesc {
                    label: instance_label.clone().push_str("::read_distance0"),
                    entries: smallvec![BindGroupEntry::DefaultTextureView(
                        distance_textures[0].handle
                    )],
                    layout: compositor_renderer.bind_group_layout_read_distance,
                },
                &ctx.gpu_resources.bind_group_layouts,
                &ctx.gpu_resources.textures,
                &ctx.gpu_resources.buffers,
                &ctx.gpu_resources.samplers,
            ),
            ctx.gpu_resources.bind_groups.alloc(
                &ctx.device,
                &BindGroupDesc {
                    label: instance_label.clone().push_str("::read_distance1"),
                    entries: smallvec![BindGroupEntry::DefaultTextureView(
                        distance_textures[1].handle
                    )],
                    layout: compositor_renderer.bind_group_layout_read_distance,
                },
                &ctx.gpu_resources.bind_group_layouts,
                &ctx.gpu_resources.textures,
                &ctx.gpu_resources.buffers,
                &ctx.gpu_resources.samplers,
            ),
        ];

        let screen_triangle_vertex_shader =
            screen_triangle_vertex_shader(&mut ctx.gpu_resources, &ctx.device, &mut ctx.resolver);
        let render_pipeline_jumpflooding_init = ctx.gpu_resources.render_pipelines.get_or_create(
            &ctx.device,
            &RenderPipelineDesc {
                label: "OutlineMaskProcessor::jumpflooding_init".into(),
                pipeline_layout: ctx.gpu_resources.pipeline_layouts.get_or_create(
                    &ctx.device,
                    &PipelineLayoutDesc {
                        label: "OutlineMaskProcessor::jumpflooding_init".into(),
                        entries: vec![bind_group_layout_read_mask],
                    },
                    &ctx.gpu_resources.bind_group_layouts,
                ),
                vertex_entrypoint: "main".into(),
                vertex_handle: screen_triangle_vertex_shader,
                fragment_entrypoint: "main".into(),
                fragment_handle: ctx.gpu_resources.shader_modules.get_or_create(
                    &ctx.device,
                    &mut ctx.resolver,
                    &ShaderModuleDesc {
                        label: "jumpflooding_init".into(),
                        source: include_file!("../../shader/outlines/jumpflooding_init.wgsl"),
                    },
                ),
                vertex_buffers: smallvec![],
                render_targets: smallvec![Some(Self::DISTANCE_FORMAT.into())],
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
            },
            &ctx.gpu_resources.pipeline_layouts,
            &ctx.gpu_resources.shader_modules,
        );
        let render_pipeline_jumpflooding_step = ctx.gpu_resources.render_pipelines.get_or_create(
            &ctx.device,
            &RenderPipelineDesc {
                label: "OutlineMaskProcessor::jumpflooding_step".into(),
                pipeline_layout: ctx.gpu_resources.pipeline_layouts.get_or_create(
                    &ctx.device,
                    &PipelineLayoutDesc {
                        label: "OutlineMaskProcessor::jumpflooding_step".into(),
                        entries: vec![compositor_renderer.bind_group_layout_read_distance],
                    },
                    &ctx.gpu_resources.bind_group_layouts,
                ),
                vertex_entrypoint: "main".into(),
                vertex_handle: screen_triangle_vertex_shader,
                fragment_entrypoint: "main".into(),
                fragment_handle: ctx.gpu_resources.shader_modules.get_or_create(
                    &ctx.device,
                    &mut ctx.resolver,
                    &ShaderModuleDesc {
                        label: "jumpflooding_step".into(),
                        source: include_file!("../../shader/outlines/jumpflooding_step.wgsl"),
                    },
                ),
                vertex_buffers: smallvec![],
                render_targets: smallvec![Some(Self::DISTANCE_FORMAT.into())],
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
            },
            &ctx.gpu_resources.pipeline_layouts,
            &ctx.gpu_resources.shader_modules,
        );

        Self {
            label: instance_label,
            mask_texture,
            mask_depth,
            distance_textures,
            bindgroup_jumpflooding_init,
            bindgroup_read_distance,
            render_pipeline_jumpflooding_init,
            render_pipeline_jumpflooding_step,
        }
    }

    pub fn start_mask_render_pass<'a>(
        &'a self,
        encoder: &'a mut wgpu::CommandEncoder,
    ) -> wgpu::RenderPass<'a> {
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: self.label.clone().push_str(" - mask pass").get(),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.mask_texture.default_view,
                resolve_target: None, // We're going to do a manual resolve.
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: true,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.mask_depth.default_view,
                depth_ops: Some(wgpu::Operations {
                    load: ViewBuilder::DEFAULT_DEPTH_CLEAR,
                    store: false,
                }),
                stencil_ops: None,
            }),
        })
    }

    pub fn compute_outlines(
        self,
        pools: &WgpuResourcePools,
        encoder: &mut wgpu::CommandEncoder,
    ) -> Result<OutlineCompositingDrawData, PoolError> {
        let pipelines = &pools.render_pipelines;

        // Initialize the jump flooding into distance texture 0 by looking at the mask texture.
        {
            let mut jumpflooding_init = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: self.label.clone().push_str(" - jumpflooding_init").get(),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.distance_textures[0].default_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

            let render_pipeline = pipelines.get_resource(self.render_pipeline_jumpflooding_init)?;
            jumpflooding_init.set_bind_group(0, &self.bindgroup_jumpflooding_init, &[]);
            jumpflooding_init.set_pipeline(render_pipeline);
            jumpflooding_init.draw(0..3, 0..1);
        }

        Ok(OutlineCompositingDrawData {
            bind_group: self.bindgroup_read_distance[0].clone(),
        })
    }
}

pub struct OutlineCompositor {
    render_pipeline: GpuRenderPipelineHandle,
    bind_group_layout_read_distance: GpuBindGroupLayoutHandle,
}

#[derive(Clone)]
pub struct OutlineCompositingDrawData {
    bind_group: GpuBindGroup,
}

impl DrawData for OutlineCompositingDrawData {
    type Renderer = OutlineCompositor;
}

impl Renderer for OutlineCompositor {
    type RendererDrawData = OutlineCompositingDrawData;

    fn participated_phases() -> &'static [DrawPhase] {
        &[DrawPhase::Compositing]
    }

    fn create_renderer<Fs: FileSystem>(
        shared_data: &SharedRendererData,
        pools: &mut WgpuResourcePools,
        device: &wgpu::Device,
        resolver: &mut FileResolver<Fs>,
    ) -> Self {
        let bind_group_layout_read_distance = pools.bind_group_layouts.get_or_create(
            device,
            &BindGroupLayoutDesc {
                label: "OutlineCompositor::bind_group_layout_read_distance".into(),
                entries: vec![wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                }],
            },
        );
        let vertex_handle = screen_triangle_vertex_shader(pools, device, resolver);
        let render_pipeline = pools.render_pipelines.get_or_create(
            device,
            &RenderPipelineDesc {
                label: "OutlineCompositor".into(),
                pipeline_layout: pools.pipeline_layouts.get_or_create(
                    device,
                    &PipelineLayoutDesc {
                        label: "OutlineCompositor".into(),
                        entries: vec![
                            shared_data.global_bindings.layout,
                            bind_group_layout_read_distance,
                        ],
                    },
                    &pools.bind_group_layouts,
                ),
                vertex_entrypoint: "main".into(),
                vertex_handle,
                fragment_entrypoint: "main".into(),
                fragment_handle: pools.shader_modules.get_or_create(
                    device,
                    resolver,
                    &ShaderModuleDesc {
                        label: "outlines_from_distance".into(),
                        source: include_file!("../../shader/outlines/outlines_from_distance.wgsl"),
                    },
                ),
                vertex_buffers: smallvec![],
                render_targets: smallvec![Some(wgpu::ColorTargetState {
                    format: shared_data.config.output_format_color,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::all()
                })],
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
            },
            &pools.pipeline_layouts,
            &pools.shader_modules,
        );

        OutlineCompositor {
            render_pipeline,
            bind_group_layout_read_distance,
        }
    }

    fn draw<'a>(
        &self,
        pools: &'a WgpuResourcePools,
        _phase: DrawPhase,
        pass: &mut wgpu::RenderPass<'a>,
        draw_data: &'a OutlineCompositingDrawData,
    ) -> anyhow::Result<()> {
        let pipeline = pools.render_pipelines.get_resource(self.render_pipeline)?;

        pass.set_pipeline(pipeline);
        pass.set_bind_group(1, &draw_data.bind_group, &[]);
        pass.draw(0..3, 0..1);

        Ok(())
    }
}
