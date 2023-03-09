//! Outlines as postprocessing effect.
//!
//! This module provides the [`OutlineMaskProcessor`] which handles render passes around outlines
//! and [`OutlineCompositor`] which handles compositing the outlines into the final image.
//!
//! There are two channels (in shader code referred to as A and B) that are handled simultaneously.
//! For configuring the look of the outline refer to [`OutlineConfig`].
//! For setting outlines for an individual primitive from another [`Renderer`]/[`DrawData`],
//! check for [`OutlineMaskPreference`] settings on that primitive.
//!
//! How it works:
//! =============
//! The basic approach follows closely @bgolus' [blog post](https://bgolus.medium.com/the-quest-for-very-wide-outlines-ba82ed442cd9)
//! on jump-flooding based outlines.
//!
//! Quick recap & overview:
//! * Render scene into a mask texture
//! * Extract a contour from the mask texture, for each contour contour pixel write the position in the (to-be) voronoi texture.
//!     * in our case we extract all pixels at which the mask changes (details below)
//! * Jump-flooding iterations: For each pixel in the voronoi texture,
//!  sample the current pixel and an 8-neighborhood at a certain, for each pass decreasing, distance and write out the closest position seen so far.
//!     * This is repeated for `log2(outline_width)` iterations.
//! * During composition, extract an outline by checking the distance to the closest contour using the voronoi texture
//!
//! What makes our implementation (a little bit) special:
//! -----------------------------------------------------
//! In short: We have more complex outline relationships but do so without additional passes!
//!
//! * Different objects may have outlines between each other
//!     * This is achieved by making the mask texture a 2 channel texture, where each channel is a different 8bit object id.
//!         * object ids are arbitrary and only for the purpose of distinguishing between outlines
//!     * Since we now no longer can resolve anti-aliasing in a straight forward manner (can't blend object ids!),
//!         * This implies a custom resolve during contour extraction!
//!     * It seems to force our hand towards outlines that extend inwards:
//!         * For each channel A & B we only get a single voronoi texture (fused into one 4 channel texture),
//!           meaning that we only have a single unsigned distance to the closest contour.
//!           If we don't want to ignore objects drawn upon each other, we need to compute the distance to any contour (== pixel where object id changes).
//!         * It might be possible to mask out inner outlines during composition, but it's not clear what the exact masking rules are for this.
//! * We use two channels (A and B) for outlines, so that we can have two independent outlines (even for the same object if desired)
//!     * We do this in a single pass by using a 2 channel texture on the mask (object id A, object id B) and
//!       a 4 channel texture on the voronoi texture (xy coordinates for A, xy coordinates for B)
//!
//! More details can be found in the respective shader code.
//!

use super::{screen_triangle_vertex_shader, DrawData, DrawPhase, Renderer};
use crate::{
    allocator::{create_and_fill_uniform_buffer, create_and_fill_uniform_buffer_batch},
    config::HardwareTier,
    context::SharedRendererData,
    include_file,
    view_builder::ViewBuilder,
    wgpu_resources::{
        BindGroupDesc, BindGroupEntry, BindGroupLayoutDesc, GpuBindGroup, GpuBindGroupLayoutHandle,
        GpuRenderPipelineHandle, GpuTexture, GpuTextureHandle, PipelineLayoutDesc, PoolError,
        RenderPipelineDesc, SamplerDesc, ShaderModuleDesc, WgpuResourcePools,
    },
    DebugLabel, FileResolver, FileSystem, RenderContext,
};

use smallvec::smallvec;

/// What outline (if any) should be drawn.
///
/// Outlines have two channels (referred to as A and B).
/// Each channel can distinguish up 255 different objects, each getting their own outline.
///
/// Object index 0 is special: It is the default background of each outline channel, thus rendering with it
/// is a form of "active no outline", effectively subtracting from the outline channel.
pub type OutlineMaskPreference = Option<[u8; 2]>;

#[derive(Clone, Debug)]
pub struct OutlineConfig {
    /// Outline radius for both layers in pixels. Fractional pixels are valid.
    ///
    /// Could do different radius for both layers if the need arises, but for now this simplifies things.
    pub outline_radius_pixel: f32,

    /// Premultiplied RGBA color for the first outline layer.
    pub color_layer_a: crate::Rgba,
    /// Premultiplied RGBA color for the second outline layer.
    pub color_layer_b: crate::Rgba,
}

// TODO(andreas): Is this a sort of DrawPhase implementor? Need a system for this.
pub struct OutlineMaskProcessor {
    label: DebugLabel,

    mask_texture: GpuTexture,
    mask_depth: GpuTexture,
    voronoi_textures: [GpuTexture; 2],

    bind_group_jumpflooding_init: GpuBindGroup,
    bind_group_jumpflooding_steps: Vec<GpuBindGroup>,
    bind_group_draw_outlines: GpuBindGroup,

    render_pipeline_jumpflooding_init: GpuRenderPipelineHandle,
    render_pipeline_jumpflooding_step: GpuRenderPipelineHandle,
}

mod gpu_data {
    use crate::wgpu_buffer_types;

    /// Keep in sync with `jumpflooding_step.wgsl`
    #[repr(C, align(256))]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct JumpfloodingStepUniformBuffer {
        pub step_width: wgpu_buffer_types::U32RowPadded,
        /// All this padding hurts. `step_width` be a PushConstant but they are not widely supported enough!
        pub end_padding: [wgpu_buffer_types::PaddingRow; 16 - 1],
    }

    /// Keep in sync with `outlines_from_voronoi.wgsl`
    #[repr(C, align(256))]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct OutlineConfigUniformBuffer {
        pub color_layer_a: wgpu_buffer_types::Vec4,
        pub color_layer_b: wgpu_buffer_types::Vec4,
        pub outline_radius_pixel: wgpu_buffer_types::F32RowPadded,
        pub end_padding: [wgpu_buffer_types::PaddingRow; 16 - 3],
    }
}

impl OutlineMaskProcessor {
    /// Format of the outline mask target.
    ///
    /// Two channels with each 256 object ids.
    pub const MASK_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg8Uint;
    pub const MASK_DEPTH_FORMAT: wgpu::TextureFormat = ViewBuilder::MAIN_TARGET_DEPTH_FORMAT;
    pub const MASK_DEPTH_STATE: Option<wgpu::DepthStencilState> =
        ViewBuilder::MAIN_TARGET_DEFAULT_DEPTH_STATE;

    /// Holds two pairs of pixel coordinates (one for each layer).
    const VORONOI_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

    /// Default MSAA state for the outline mask target.
    pub fn get_mask_default_msaa_state(tier: HardwareTier) -> wgpu::MultisampleState {
        wgpu::MultisampleState {
            count: Self::get_mask_sample_count(tier),
            mask: !0,
            alpha_to_coverage_enabled: false,
        }
    }

    /// Number of MSAA samples used for the outline mask target.
    pub fn get_mask_sample_count(tier: HardwareTier) -> u32 {
        match tier {
            HardwareTier::Web => 1,
            // The MSAA shader variant deals with *exactly* 4 samples.
            // See `jumpflooding_step_msaa.wgsl`.
            HardwareTier::Native => 4,
        }
    }

    pub fn new(
        ctx: &mut RenderContext,
        config: &OutlineConfig,
        view_name: &DebugLabel,
        resolution_in_pixel: [u32; 2],
    ) -> Self {
        crate::profile_function!();
        let instance_label = view_name.clone().push_str(" - OutlineMaskProcessor");

        // ------------- Textures -------------
        let texture_pool = &ctx.gpu_resources.textures;

        let mask_sample_count =
            Self::get_mask_sample_count(ctx.shared_renderer_data.config.hardware_tier);
        let mask_texture_desc = crate::wgpu_resources::TextureDesc {
            label: instance_label.clone().push_str("::mask_texture"),
            size: wgpu::Extent3d {
                width: resolution_in_pixel[0],
                height: resolution_in_pixel[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: mask_sample_count,
            dimension: wgpu::TextureDimension::D2,
            format: Self::MASK_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
        };
        let mask_texture = texture_pool.alloc(&ctx.device, &mask_texture_desc);

        // We have a fresh depth buffer here that we need because:
        // * We want outlines visible even if there's an object in front, so don't re-use previous
        // * Overdraw IDs correctly
        // * TODO(andreas): Make overdrawn outlines more transparent by comparing depth
        let mask_depth = texture_pool.alloc(
            &ctx.device,
            &crate::wgpu_resources::TextureDesc {
                label: instance_label.clone().push_str("::mask_depth"),
                format: Self::MASK_DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                ..mask_texture_desc
            },
        );

        let voronoi_texture_desc = crate::wgpu_resources::TextureDesc {
            label: instance_label.clone().push_str("::distance_texture"),
            sample_count: 1,
            format: Self::VORONOI_FORMAT,
            ..mask_texture_desc
        };
        let voronoi_textures = [
            texture_pool.alloc(&ctx.device, &voronoi_texture_desc.with_label_push("0")),
            texture_pool.alloc(&ctx.device, &voronoi_texture_desc.with_label_push("1")),
        ];

        // ------------- Bind Groups -------------

        let (bind_group_jumpflooding_init, bind_group_layout_jumpflooding_init) =
            Self::create_bind_group_jumpflooding_init(ctx, &instance_label, &mask_texture);
        let (bind_group_jumpflooding_steps, bind_group_layout_jumpflooding_step) =
            Self::create_bind_groups_for_jumpflooding_steps(
                config,
                ctx,
                &instance_label,
                &voronoi_textures,
            );

        // Create a bind group for the final compositor pass - it will read the last voronoi texture
        let bind_group_draw_outlines = {
            let mut renderers = ctx.renderers.write();
            let compositor_renderer = renderers.get_or_create::<_, OutlineCompositor>(
                &ctx.shared_renderer_data,
                &mut ctx.gpu_resources,
                &ctx.device,
                &mut ctx.resolver,
            );

            // Point to the last written voronoi texture
            // We start writing to voronoi_textures[0] and then do `num_steps` ping-pong rendering.
            // Therefore, the last texture is voronoi_textures[num_steps % 2]
            compositor_renderer.create_bind_group(
                ctx,
                voronoi_textures[bind_group_jumpflooding_steps.len() % 2].handle,
                config,
            )
        };

        // ------------- Render Pipelines -------------

        let screen_triangle_vertex_shader =
            screen_triangle_vertex_shader(&mut ctx.gpu_resources, &ctx.device, &mut ctx.resolver);
        let jumpflooding_init_desc = RenderPipelineDesc {
            label: "OutlineMaskProcessor::jumpflooding_init".into(),
            pipeline_layout: ctx.gpu_resources.pipeline_layouts.get_or_create(
                &ctx.device,
                &PipelineLayoutDesc {
                    label: "OutlineMaskProcessor::jumpflooding_init".into(),
                    entries: vec![bind_group_layout_jumpflooding_init],
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
                    source: if mask_sample_count == 1 {
                        include_file!("../../shader/outlines/jumpflooding_init.wgsl")
                    } else {
                        include_file!("../../shader/outlines/jumpflooding_init_msaa.wgsl")
                    },
                },
            ),
            vertex_buffers: smallvec![],
            render_targets: smallvec![Some(Self::VORONOI_FORMAT.into())],
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
        };
        let render_pipeline_jumpflooding_init = ctx.gpu_resources.render_pipelines.get_or_create(
            &ctx.device,
            &jumpflooding_init_desc,
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
                        entries: vec![bind_group_layout_jumpflooding_step],
                    },
                    &ctx.gpu_resources.bind_group_layouts,
                ),
                fragment_handle: ctx.gpu_resources.shader_modules.get_or_create(
                    &ctx.device,
                    &mut ctx.resolver,
                    &ShaderModuleDesc {
                        label: "jumpflooding_step".into(),
                        source: include_file!("../../shader/outlines/jumpflooding_step.wgsl"),
                    },
                ),
                ..jumpflooding_init_desc
            },
            &ctx.gpu_resources.pipeline_layouts,
            &ctx.gpu_resources.shader_modules,
        );

        Self {
            label: instance_label,
            mask_texture,
            mask_depth,
            voronoi_textures,
            bind_group_jumpflooding_init,
            bind_group_jumpflooding_steps,
            bind_group_draw_outlines,
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

        let ops = wgpu::Operations {
            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), // Clear is the closest to "don't care"
            store: true,
        };

        // Initialize the jump flooding into voronoi texture 0 by looking at the mask texture.
        {
            let mut jumpflooding_init = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: self.label.clone().push_str(" - jumpflooding_init").get(),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.voronoi_textures[0].default_view,
                    resolve_target: None,
                    ops,
                })],
                depth_stencil_attachment: None,
            });

            let render_pipeline_init =
                pipelines.get_resource(self.render_pipeline_jumpflooding_init)?;
            jumpflooding_init.set_bind_group(0, &self.bind_group_jumpflooding_init, &[]);
            jumpflooding_init.set_pipeline(render_pipeline_init);
            jumpflooding_init.draw(0..3, 0..1);
        }

        // Perform jump flooding.
        let render_pipeline_step =
            pipelines.get_resource(self.render_pipeline_jumpflooding_step)?;
        for (i, bind_group) in self.bind_group_jumpflooding_steps.into_iter().enumerate() {
            let mut jumpflooding_step = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: self
                    .label
                    .clone()
                    .push_str(&format!(" - jumpflooding_step {i}"))
                    .get(),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    // Start with texture 1 since the init step wrote to texture 0
                    view: &self.voronoi_textures[(i + 1) % 2].default_view,
                    resolve_target: None,
                    ops,
                })],
                depth_stencil_attachment: None,
            });

            jumpflooding_step.set_pipeline(render_pipeline_step);
            jumpflooding_step.set_bind_group(0, &bind_group, &[]);
            jumpflooding_step.draw(0..3, 0..1);
        }

        Ok(OutlineCompositingDrawData {
            bind_group: self.bind_group_draw_outlines,
        })
    }

    fn create_bind_group_jumpflooding_init(
        ctx: &mut RenderContext,
        instance_label: &DebugLabel,
        mask_texture: &GpuTexture,
    ) -> (GpuBindGroup, GpuBindGroupLayoutHandle) {
        let bind_group_layout_jumpflooding_init =
            ctx.gpu_resources.bind_group_layouts.get_or_create(
                &ctx.device,
                &BindGroupLayoutDesc {
                    label: "OutlineMaskProcessor::bind_group_layout_jumpflooding_init".into(),
                    entries: vec![wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Uint,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: mask_texture.texture.sample_count() > 1,
                        },
                        count: None,
                    }],
                },
            );
        (
            ctx.gpu_resources.bind_groups.alloc(
                &ctx.device,
                &ctx.gpu_resources,
                &BindGroupDesc {
                    label: instance_label.clone().push_str("::jumpflooding_init"),
                    entries: smallvec![BindGroupEntry::DefaultTextureView(mask_texture.handle)],
                    layout: bind_group_layout_jumpflooding_init,
                },
            ),
            bind_group_layout_jumpflooding_init,
        )
    }

    fn create_bind_groups_for_jumpflooding_steps(
        config: &OutlineConfig,
        ctx: &mut RenderContext,
        instance_label: &DebugLabel,
        voronoi_textures: &[GpuTexture; 2],
    ) -> (Vec<GpuBindGroup>, GpuBindGroupLayoutHandle) {
        let bind_group_layout_jumpflooding_step =
            ctx.gpu_resources.bind_group_layouts.get_or_create(
                &ctx.device,
                &BindGroupLayoutDesc {
                    label: "OutlineMaskProcessor::bind_group_layout_jumpflooding_step".into(),
                    entries: vec![
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                // Dynamic offset would make sense here since we cycle through a bunch of these.
                                // But we need at least two bind groups anyways since we're ping-ponging between two textures,
                                // which would make this needlessly complicated.
                                has_dynamic_offset: false,
                                min_binding_size: std::num::NonZeroU64::new(std::mem::size_of::<
                                    gpu_data::JumpfloodingStepUniformBuffer,
                                >(
                                )
                                    as _),
                            },
                            count: None,
                        },
                    ],
                },
            );

        let max_step_width =
            (config.outline_radius_pixel.max(1.0).ceil() as u32).next_power_of_two();
        let num_steps = max_step_width.ilog2() + 1;
        let uniform_buffer_jumpflooding_steps_bindings = create_and_fill_uniform_buffer_batch(
            ctx,
            "jumpflooding uniformbuffer".into(),
            (0..num_steps)
                .into_iter()
                .map(|step| gpu_data::JumpfloodingStepUniformBuffer {
                    step_width: (max_step_width >> step).into(),
                    end_padding: Default::default(),
                }),
        );
        let sampler = ctx.gpu_resources.samplers.get_or_create(
            &ctx.device,
            &SamplerDesc {
                label: "nearest_clamp".into(),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                ..Default::default()
            },
        );
        let uniform_buffer_jumpflooding_steps = uniform_buffer_jumpflooding_steps_bindings
            .into_iter()
            .enumerate()
            .map(|(i, uniform_buffer_binding)| {
                ctx.gpu_resources.bind_groups.alloc(
                    &ctx.device,
                    &ctx.gpu_resources,
                    &BindGroupDesc {
                        label: instance_label
                            .clone()
                            .push_str(&format!("::jumpflooding_steps[{i}]")),
                        entries: smallvec![
                            BindGroupEntry::DefaultTextureView(voronoi_textures[i % 2].handle),
                            BindGroupEntry::Sampler(sampler),
                            uniform_buffer_binding
                        ],
                        layout: bind_group_layout_jumpflooding_step,
                    },
                )
            })
            .collect();

        (
            uniform_buffer_jumpflooding_steps,
            bind_group_layout_jumpflooding_step,
        )
    }
}

pub struct OutlineCompositor {
    render_pipeline: GpuRenderPipelineHandle,
    bind_group_layout: GpuBindGroupLayoutHandle,
}

#[derive(Clone)]
pub struct OutlineCompositingDrawData {
    bind_group: GpuBindGroup,
}

impl DrawData for OutlineCompositingDrawData {
    type Renderer = OutlineCompositor;
}

impl OutlineCompositor {
    fn create_bind_group(
        &self,
        ctx: &RenderContext,
        final_voronoi_texture: GpuTextureHandle,
        config: &OutlineConfig,
    ) -> GpuBindGroup {
        let uniform_buffer_binding = create_and_fill_uniform_buffer(
            ctx,
            "OutlineCompositingDrawData".into(),
            gpu_data::OutlineConfigUniformBuffer {
                color_layer_a: config.color_layer_a.into(),
                color_layer_b: config.color_layer_b.into(),
                outline_radius_pixel: config.outline_radius_pixel.into(),
                end_padding: Default::default(),
            },
        );

        ctx.gpu_resources.bind_groups.alloc(
            &ctx.device,
            &ctx.gpu_resources,
            &BindGroupDesc {
                label: "OutlineCompositingDrawData".into(),
                entries: smallvec![
                    BindGroupEntry::DefaultTextureView(final_voronoi_texture),
                    uniform_buffer_binding
                ],
                layout: self.bind_group_layout,
            },
        )
    }
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
        let bind_group_layout = pools.bind_group_layouts.get_or_create(
            device,
            &BindGroupLayoutDesc {
                label: "OutlineCompositor::bind_group_layout".into(),
                entries: vec![
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: std::num::NonZeroU64::new(std::mem::size_of::<
                                gpu_data::OutlineConfigUniformBuffer,
                            >(
                            )
                                as _),
                        },
                        count: None,
                    },
                ],
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
                        entries: vec![shared_data.global_bindings.layout, bind_group_layout],
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
                        label: "outlines_from_voronoi".into(),
                        source: include_file!("../../shader/outlines/outlines_from_voronoi.wgsl"),
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
            bind_group_layout,
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
