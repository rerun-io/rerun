use smallvec::smallvec;

use crate::renderer::screen_triangle_vertex_shader;
use crate::wgpu_resources::{
    BindGroupDesc, BindGroupEntry, BindGroupLayoutDesc, GpuBindGroup, GpuBindGroupLayoutHandle,
    GpuRenderPipelineHandle, GpuRenderPipelinePoolAccessor, GpuTexture, PipelineLayoutDesc,
    PoolError, RenderPipelineDesc, TextureDesc,
};
use crate::{RenderContext, include_shader_module};

/// Supplies single-sample reverse-Z depth in an `R32Float` color texture.
///
/// Always resolves, even for single-sample sources, so consumers can use ordinary float
/// textures without requiring depth-texture loads on WebGL.
/// MSAA already requires this pass in the expected case; single-sample conversion is inexpensive.
/// The maximum sample depth preserves the nearest occluder at partially covered pixels.
/// On WebGL, supplies a zero-depth placeholder instead of sampling depth.
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
    /// Both single-sample and multisampled sources are converted, except on WebGL.
    pub fn new(ctx: &RenderContext, source: &GpuTexture) -> Self {
        let can_resolve = ctx.device_caps().tier.support_sampling_msaa_texture();
        let size = if can_resolve {
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
        let usage = if can_resolve {
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
                format: wgpu::TextureFormat::R32Float,
                usage,
            },
        );
        Self {
            resolved_depth,
            resolve_pass: can_resolve.then(|| Self::create_resolve_pass(ctx, source)),
        }
    }

    fn create_resolve_pass(ctx: &RenderContext, source: &GpuTexture) -> DepthResolvePass {
        let multisampled = source.creation_desc.sample_count > 1;
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
                        multisampled,
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
        let pipeline = Self::create_pipeline(ctx, layout, multisampled);
        DepthResolvePass {
            source_bind_group,
            pipeline,
        }
    }

    fn create_pipeline(
        ctx: &RenderContext,
        layout: GpuBindGroupLayoutHandle,
        multisampled: bool,
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
            &if multisampled {
                include_shader_module!("../../shader/resolve_depth.wgsl")
            } else {
                include_shader_module!("../../shader/copy_depth.wgsl")
            },
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
                render_targets: smallvec![Some(wgpu::TextureFormat::R32Float.into())],
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
            },
        )
    }

    /// Records the conversion/resolve pass, or does nothing for the WebGL placeholder.
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
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.resolved_depth.default_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
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
