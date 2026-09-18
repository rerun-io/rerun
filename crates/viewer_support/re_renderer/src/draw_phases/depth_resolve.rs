use smallvec::smallvec;

use crate::renderer::screen_triangle_vertex_shader;
use crate::wgpu_resources::{
    BindGroupDesc, BindGroupEntry, BindGroupLayoutDesc, GpuBindGroup, GpuBindGroupLayoutHandle,
    GpuRenderPipelineHandle, GpuRenderPipelinePoolAccessor, GpuTexture, PipelineLayoutDesc,
    PoolError, RenderPipelineDesc, TextureDesc,
};
use crate::{RenderContext, include_shader_module};

/// Supplies single-sample reverse-Z depth, resolving multisampled sources when supported.
///
/// The maximum sample depth preserves the nearest occluder at partially covered pixels.
/// On WebGL, supplies a zero-depth placeholder because multisampled depth cannot be sampled.
pub struct DepthResolveProcessor {
    resolved_depth: GpuTexture,
    resolve_pass: Option<DepthResolvePass>,
}

struct DepthResolvePass {
    source_bind_group: GpuBindGroup,
    pipeline: GpuRenderPipelineHandle,
}

impl DepthResolveProcessor {
    /// The source must be a 2D depth texture with [`wgpu::TextureUsages::TEXTURE_BINDING`] usage.
    /// Single-sample sources are reused without a resolve pass.
    ///
    /// (except on WebGL, where we can't actually resolve multisampled depth)
    pub fn new(ctx: &RenderContext, source: &GpuTexture) -> Self {
        if source.creation_desc.sample_count == 1 {
            return Self {
                resolved_depth: source.clone(),
                resolve_pass: None,
            };
        }

        let can_resolve_msaa = ctx.device_caps().tier.support_sampling_msaa_texture();
        let size = if can_resolve_msaa {
            source.creation_desc.size
        } else {
            // WebGL cannot resolve MSAA depth without redrawing the scene
            // Use a 1x1 texture as a placeholder for the resolved depth.
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            }
        };
        let usage = if can_resolve_msaa {
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT
        } else {
            wgpu::TextureUsages::TEXTURE_BINDING
        };
        let resolved_depth = ctx.gpu_resources.textures.alloc(
            &ctx.device,
            &TextureDesc {
                label: "scene depth resolved".into(),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: source.creation_desc.format,
                usage,
            },
        );
        Self {
            resolved_depth,
            resolve_pass: can_resolve_msaa.then(|| Self::create_resolve_pass(ctx, source)),
        }
    }

    fn create_resolve_pass(ctx: &RenderContext, source: &GpuTexture) -> DepthResolvePass {
        let layout = ctx.gpu_resources.bind_group_layouts.get_or_create(
            &ctx.device,
            &BindGroupLayoutDesc {
                label: "depth resolve source".into(),
                entries: vec![wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: true,
                    },
                    count: None,
                }],
            },
        );
        let source_bind_group = ctx.gpu_resources.bind_groups.alloc(
            &ctx.device,
            &ctx.gpu_resources,
            &BindGroupDesc {
                label: "depth resolve source".into(),
                entries: smallvec![BindGroupEntry::DefaultTextureView(source.handle)],
                layout,
            },
        );
        let pipeline = Self::create_pipeline(ctx, layout, source.creation_desc.format);
        DepthResolvePass {
            source_bind_group,
            pipeline,
        }
    }

    fn create_pipeline(
        ctx: &RenderContext,
        layout: GpuBindGroupLayoutHandle,
        format: wgpu::TextureFormat,
    ) -> GpuRenderPipelineHandle {
        let pipeline_layout = ctx.gpu_resources.pipeline_layouts.get_or_create(
            ctx,
            &PipelineLayoutDesc {
                label: "depth resolve".into(),
                entries: vec![layout],
            },
        );
        let shader = ctx.gpu_resources.shader_modules.get_or_create(
            ctx,
            &include_shader_module!("../../shader/resolve_depth.wgsl"),
        );
        ctx.gpu_resources.render_pipelines.get_or_create(
            ctx,
            &RenderPipelineDesc {
                label: "depth resolve".into(),
                pipeline_layout,
                vertex_entrypoint: "main".into(),
                vertex_handle: screen_triangle_vertex_shader(ctx),
                fragment_entrypoint: "main".into(),
                fragment_handle: shader,
                vertex_buffers: smallvec![],
                render_targets: smallvec![],
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::Always),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
            },
        )
    }

    /// Records the resolve pass, or does nothing for single-sample sources and the WebGL placeholder.
    pub fn resolve(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        pipelines: &GpuRenderPipelinePoolAccessor<'_>,
    ) -> Result<GpuTexture, PoolError> {
        let Some(resolve_pass) = &self.resolve_pass else {
            return Ok(self.resolved_depth.clone());
        };

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("depth resolve"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.resolved_depth.default_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(pipelines.get(resolve_pass.pipeline)?);
        pass.set_bind_group(0, &resolve_pass.source_bind_group, &[]);
        pass.draw(0..3, 0..1);

        Ok(self.resolved_depth.clone())
    }
}
