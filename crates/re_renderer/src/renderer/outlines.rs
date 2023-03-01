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
        GpuRenderPipelineHandle, GpuTexture, PipelineLayoutDesc, RenderPipelineDesc,
        ShaderModuleDesc, WgpuResourcePools,
    },
    DebugLabel, FileResolver, FileSystem, RenderContext,
};

use super::{DrawData, DrawPhase, Renderer};

use smallvec::smallvec;

#[derive(Clone, Debug)]
pub struct OutlineConfig {
    pub color_layer_0: crate::Rgba,
    pub color_layer_1: crate::Rgba,
}

// TODO(andreas): Is this a sort of DrawPhase implementor? Need a system for this.
pub struct OutlineMaskProcessor {
    label: DebugLabel,

    mask_texture: GpuTexture,
    mask_depth: GpuTexture,

    bindgroup_debug: GpuBindGroup,
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

    pub fn new(
        ctx: &mut RenderContext,
        config: OutlineConfig,
        view_name: &DebugLabel,
        resolution_in_pixel: [u32; 2],
    ) -> Self {
        let mut renderers = ctx.renderers.write();
        let debug_compositor = renderers.get_or_create::<_, OutlineCompositorDebug>(
            &ctx.shared_renderer_data,
            &mut ctx.gpu_resources,
            &ctx.device,
            &mut ctx.resolver,
        );

        let label = view_name.clone().push_str(" - OutlineMaskProcessor");
        let mask_texture = ctx.gpu_resources.textures.alloc(
            &ctx.device,
            &crate::wgpu_resources::TextureDesc {
                label: label.clone().push_str(" - mask_texture"),
                size: wgpu::Extent3d {
                    width: resolution_in_pixel[0],
                    height: resolution_in_pixel[1],
                    depth_or_array_layers: 1,
                },
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
                label: label.clone().push_str(" - mask_depth"),
                size: wgpu::Extent3d {
                    width: resolution_in_pixel[0],
                    height: resolution_in_pixel[1],
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: ViewBuilder::MAIN_TARGET_SAMPLE_COUNT,
                dimension: wgpu::TextureDimension::D2,
                format: Self::MASK_DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            },
        );

        let bindgroup_debug = ctx.gpu_resources.bind_groups.alloc(
            &ctx.device,
            &BindGroupDesc {
                label: view_name.clone().push_str(" - debug"),
                entries: smallvec![BindGroupEntry::DefaultTextureView(mask_texture.handle)],
                layout: debug_compositor.bind_group_layout,
            },
            &ctx.gpu_resources.bind_group_layouts,
            &ctx.gpu_resources.textures,
            &ctx.gpu_resources.buffers,
            &ctx.gpu_resources.samplers,
        );

        Self {
            label,
            mask_texture,
            mask_depth,
            bindgroup_debug,
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

    pub fn create_composition_debug_draw_data(self) -> OutlineCompositingDebugDrawData {
        OutlineCompositingDebugDrawData {
            bind_group: self.bindgroup_debug,
        }
    }
}

pub struct OutlineCompositorDebug {
    render_pipeline: GpuRenderPipelineHandle,
    bind_group_layout: GpuBindGroupLayoutHandle,
}

#[derive(Clone)]
pub struct OutlineCompositingDebugDrawData {
    bind_group: GpuBindGroup,
}

impl DrawData for OutlineCompositingDebugDrawData {
    type Renderer = OutlineCompositorDebug;
}

impl Renderer for OutlineCompositorDebug {
    type RendererDrawData = OutlineCompositingDebugDrawData;

    fn participated_phases() -> &'static [DrawPhase] {
        &[DrawPhase::Compositing]
    }

    fn create_renderer<Fs: FileSystem>(
        shared_data: &SharedRendererData,
        pools: &mut WgpuResourcePools,
        device: &wgpu::Device,
        resolver: &mut FileResolver<Fs>,
    ) -> Self {
        let bind_group_layout = pools.bind_group_layouts.get_or_create(
            device,
            &BindGroupLayoutDesc {
                label: "OutlineCompositorDebug".into(),
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

        let render_pipeline = pools.render_pipelines.get_or_create(
            device,
            &RenderPipelineDesc {
                label: "OutlineCompositorDebug".into(),
                pipeline_layout: pools.pipeline_layouts.get_or_create(
                    device,
                    &PipelineLayoutDesc {
                        label: "OutlineCompositorDebug".into(),
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
                        label: "OutlineCompositorDebug".into(),
                        source: include_file!("../../shader/outlines/debug.wgsl"),
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

        OutlineCompositorDebug {
            render_pipeline,
            bind_group_layout,
        }
    }

    fn draw<'a>(
        &self,
        pools: &'a WgpuResourcePools,
        _phase: DrawPhase,
        pass: &mut wgpu::RenderPass<'a>,
        draw_data: &'a OutlineCompositingDebugDrawData,
    ) -> anyhow::Result<()> {
        let pipeline = pools.render_pipelines.get_resource(self.render_pipeline)?;

        pass.set_pipeline(pipeline);
        pass.set_bind_group(1, &draw_data.bind_group, &[]);
        pass.draw(0..3, 0..1);

        Ok(())
    }
}
